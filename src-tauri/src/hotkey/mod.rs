//! Raccourci clavier global d'activation de la dictée (Phase 1).
//!
//! Basé sur la crate `global-hotkey` (écosystème Tauri). Deux modes
//! configurables par l'utilisateur :
//! - `PushToTalk` : dictée active tant que la touche est maintenue.
//! - `Toggle` : un appui démarre, un second appui arrête.
//!
//! `GlobalHotKeyManager` doit être créé sur le thread qui exécute la boucle
//! d'événements Win32 (le thread principal Tauri, via `.setup()`) — voir
//! doc `global-hotkey`. Les événements (press/release) sont ensuite reçus
//! sur un thread dédié, traduits en événements Tauri `listening_started` /
//! `listening_stopped` vers le frontend, et pilotent aussi le démarrage/
//! arrêt réel de la capture micro (`audio::capture`, Spec_Backend_Desktop.md
//! §1.2) via `CaptureCommand`.

/// Push-to-talk Ctrl+Win seul (§2.5) — mécanisme *additionnel* à
/// `HotkeyManager`, pour le cas « modificateurs seuls » que `global-hotkey`
/// ne sait pas représenter. Windows uniquement (hook `WH_KEYBOARD_LL`).
#[cfg(target_os = "windows")]
pub mod modifier_combo;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::audio::capture::CaptureCommand;
use crossbeam_channel::Sender;
use global_hotkey::{
    hotkey::HotKey, GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState,
};
use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HotkeyMode {
    PushToTalk,
    Toggle,
}

pub struct HotkeyManager;

impl HotkeyManager {
    /// Enregistre `hotkey_str` (ex. `"control+alt+Space"`, syntaxe
    /// `global_hotkey::hotkey::HotKey`) comme raccourci global et démarre
    /// l'écoute des événements press/release sur un thread dédié.
    ///
    /// Doit être appelé sur le thread principal (boucle d'événements Win32,
    /// typiquement depuis `.setup()`) — `GlobalHotKeyManager` n'est ni
    /// `Send` ni `Sync` sur Windows (handle natif), il ne peut donc pas être
    /// stocké dans le state managé de Tauri ni déplacé vers un thread : on
    /// le `Box::leak` volontairement pour qu'il vive aussi longtemps que le
    /// processus (l'OS le nettoie à la fermeture de l'app, comme n'importe
    /// quel handle de fenêtre/thread).
    pub fn register(
        app: AppHandle,
        hotkey_str: &str,
        mode: HotkeyMode,
        capture_tx: Sender<CaptureCommand>,
        listening: Arc<AtomicBool>,
        command_mode_hotkey: Option<&str>,
    ) -> Result<(), String> {
        let manager = GlobalHotKeyManager::new().map_err(|e| e.to_string())?;
        let hotkey: HotKey = hotkey_str
            .parse()
            .map_err(|e| format!("raccourci invalide '{hotkey_str}' : {e}"))?;
        manager.register(hotkey).map_err(|e| e.to_string())?;

        // Command Mode (Spec_Frontend.md §2.2) : un second raccourci global,
        // enregistré sur le **même** `GlobalHotKeyManager` et démultiplexé par
        // `event.id()` dans la boucle ci-dessous. `GlobalHotKeyEvent::receiver()`
        // est un singleton global — un second thread consommateur volerait des
        // événements au premier ; tout doit passer par une seule boucle.
        //
        // **Non fatal** : un raccourci Command Mode invalide ou déjà pris par
        // une autre app ne doit pas empêcher l'enregistrement du raccourci de
        // dictée (cas recouvrable, à surfacer dans les réglages — même
        // philosophie que l'échec du raccourci principal, géré dans `lib.rs`).
        let command_hotkey_id: Option<u32> = command_mode_hotkey.and_then(|s| {
            match s.parse::<HotKey>() {
                Ok(hk) => match manager.register(hk) {
                    Ok(()) => {
                        println!("[hotkey] Command Mode : raccourci '{s}' enregistré");
                        Some(hk.id())
                    }
                    Err(e) => {
                        eprintln!("[hotkey] raccourci Command Mode '{s}' non enregistré ({e}) — Command Mode inactif");
                        None
                    }
                },
                Err(e) => {
                    eprintln!("[hotkey] raccourci Command Mode '{s}' invalide ({e}) — Command Mode inactif");
                    None
                }
            }
        });

        Box::leak(Box::new(manager));

        let dictation_hotkey_id = hotkey.id();
        let receiver = GlobalHotKeyEvent::receiver();
        std::thread::spawn(move || {
            while let Ok(event) = receiver.recv() {
                // Command Mode : sur appui du raccourci dédié, on capture la
                // sélection courante et on ouvre la palette (fenêtre `command`).
                // Le relâchement n'a pas de sémantique ici (pas de push-to-talk).
                if Some(event.id()) == command_hotkey_id {
                    if event.state() == HotKeyState::Pressed {
                        crate::commands::open_command_palette(&app);
                    }
                    continue;
                }
                if event.id() != dictation_hotkey_id {
                    continue;
                }
                // État courant lu depuis le drapeau partagé, pas depuis un
                // compteur local : si la dictée a été démarrée/arrêtée depuis
                // l'UI, le mode `Toggle` doit en tenir compte (sinon il faut
                // un appui « fantôme » pour se recaler).
                let currently_listening = listening.load(Ordering::SeqCst);
                let should_be_listening = match (mode, event.state()) {
                    (HotkeyMode::PushToTalk, HotKeyState::Pressed) => true,
                    (HotkeyMode::PushToTalk, HotKeyState::Released) => false,
                    (HotkeyMode::Toggle, HotKeyState::Pressed) => !currently_listening,
                    (HotkeyMode::Toggle, HotKeyState::Released) => continue,
                };
                if should_be_listening == currently_listening {
                    continue;
                }
                listening.store(should_be_listening, Ordering::SeqCst);
                if should_be_listening {
                    println!("[hotkey] listening_started ({mode:?})");
                    let _ = capture_tx.send(CaptureCommand::Start);
                    let _ = app.emit("listening_started", ());
                } else {
                    println!("[hotkey] listening_stopped ({mode:?})");
                    let _ = capture_tx.send(CaptureCommand::Stop);
                    let _ = app.emit("listening_stopped", ());
                    // Pipeline complet (Spec_Backend_Desktop.md §1.6) :
                    // transcription -> nettoyage -> injection. Bloquant
                    // (~1-2s), volontairement synchrone sur ce thread dédié
                    // — un appui pendant le traitement met simplement en
                    // attente l'événement suivant dans le canal `receiver`,
                    // rien n'est perdu.
                    crate::commands::run_pipeline(&app);
                }
            }
        });

        Ok(())
    }
}
