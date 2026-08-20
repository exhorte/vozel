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
    ) -> Result<(), String> {
        let manager = GlobalHotKeyManager::new().map_err(|e| e.to_string())?;
        let hotkey: HotKey = hotkey_str
            .parse()
            .map_err(|e| format!("raccourci invalide '{hotkey_str}' : {e}"))?;
        manager.register(hotkey).map_err(|e| e.to_string())?;
        Box::leak(Box::new(manager));

        let receiver = GlobalHotKeyEvent::receiver();
        std::thread::spawn(move || {
            let mut listening = false;
            while let Ok(event) = receiver.recv() {
                let should_be_listening = match (mode, event.state()) {
                    (HotkeyMode::PushToTalk, HotKeyState::Pressed) => true,
                    (HotkeyMode::PushToTalk, HotKeyState::Released) => false,
                    (HotkeyMode::Toggle, HotKeyState::Pressed) => !listening,
                    (HotkeyMode::Toggle, HotKeyState::Released) => continue,
                };
                if should_be_listening == listening {
                    continue;
                }
                listening = should_be_listening;
                if listening {
                    println!("[hotkey] listening_started ({mode:?})");
                    let _ = capture_tx.send(CaptureCommand::Start);
                    let _ = app.emit("listening_started", ());
                } else {
                    println!("[hotkey] listening_stopped ({mode:?})");
                    let _ = capture_tx.send(CaptureCommand::Stop);
                    let _ = app.emit("listening_stopped", ());
                }
            }
        });

        Ok(())
    }
}
