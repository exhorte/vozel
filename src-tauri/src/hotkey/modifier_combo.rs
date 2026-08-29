//! Push-to-talk sur le **maintien simultané de Ctrl + Win seuls** (sans 3ᵉ
//! touche) — Spec_Backend_Desktop.md §2.5.
//!
//! Pourquoi un mécanisme distinct : `HotkeyManager` (§1.1, crate
//! `global-hotkey` → `RegisterHotKey` Win32) exige une touche non
//! modificatrice dans la combinaison. « Ctrl+Win » à vide n'est pas
//! représentable ainsi (le garde `MODIFIER_CODES` de `ModelPanel.tsx` le
//! reflète côté UI). Ce module **s'ajoute** à `HotkeyManager`, ne le
//! remplace pas : n'importe quelle combinaison classique continue de passer
//! par `global-hotkey` inchangé.
//!
//! Mécanisme : hook clavier bas niveau `WH_KEYBOARD_LL`, installé sur le
//! thread qui pompe la boucle de messages Win32 (thread principal Tauri, via
//! `.setup()` — même contrainte que `GlobalHotKeyManager`). Le callback du
//! hook **ne fait que** basculer deux `AtomicBool` (`CTRL_DOWN`/`WIN_DOWN`)
//! et rendre la main : Windows désinstalle silencieusement un hook LL trop
//! lent (`LowLevelHooksTimeout`, ~300 ms). Toute la logique
//! (démarrage/arrêt de dictée, pipeline) tourne sur un thread dédié réveillé
//! par `unpark`, jamais dans le callback.
//!
//! Règle d'activation (exacte, §2.5 point 2) : démarrage dès que Ctrl **et**
//! Win sont enfoncées (peu importe l'ordre) ; arrêt dès que Ctrl **ou** Win
//! est relâchée ; une 3ᵉ touche pendant le maintien n'annule rien (on ne
//! suit que les VK Ctrl/Win). Sémantique identique au `PushToTalk` existant.
//!
//! Les touches ne sont **pas** consommées (`CallNextHookEx` toujours appelé)
//! — le reste du système voit Ctrl+Win normalement.

#![cfg(target_os = "windows")]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::OnceLock;
use std::thread::Thread;

use crossbeam_channel::{bounded, Sender};
use tauri::{AppHandle, Emitter};
use windows::Win32::Foundation::{HINSTANCE, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    VK_LCONTROL, VK_LWIN, VK_RCONTROL, VK_RWIN,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, SetWindowsHookExW, HC_ACTION, HHOOK, KBDLLHOOKSTRUCT, WH_KEYBOARD_LL,
    WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
};

use crate::audio::capture::CaptureCommand;

/// État des modificateurs, mis à jour **uniquement** par le callback du hook.
static CTRL_DOWN: AtomicBool = AtomicBool::new(false);
static WIN_DOWN: AtomicBool = AtomicBool::new(false);
/// Dernier « Ctrl && Win » signalé au worker — sert à ne le réveiller que
/// sur un vrai front (montée ou descente du combo), pas à chaque touche.
static COMBO_WANT: AtomicBool = AtomicBool::new(false);
/// Handle du thread worker, pour que le callback puisse le `unpark`.
static WORKER: OnceLock<Thread> = OnceLock::new();
/// Garde-fou : un seul hook installé par process.
static INSTALLED: AtomicBool = AtomicBool::new(false);

/// Décision pure : que faire compte tenu de l'état des deux modificateurs et
/// du fait qu'une dictée est déjà en cours ou non. Isolé pour être testable
/// sans Windows.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(crate) enum ComboAction {
    Start,
    Stop,
    Nothing,
}

pub(crate) fn decide(ctrl_down: bool, win_down: bool, currently_listening: bool) -> ComboAction {
    let want = ctrl_down && win_down;
    match (want, currently_listening) {
        (true, false) => ComboAction::Start,
        (false, true) => ComboAction::Stop,
        _ => ComboAction::Nothing,
    }
}

/// `true` si l'état de `flag` a réellement changé (ignore les `WM_KEYDOWN`
/// en rafale de l'auto-répétition : `swap(true)` sur un flag déjà `true`
/// renvoie `true`, donc `!= true` est faux).
fn note_state_change(flag: &AtomicBool, down: bool) -> bool {
    flag.swap(down, Ordering::SeqCst) != down
}

/// Callback du hook — chemin ultra-court obligatoire (voir en-tête).
unsafe extern "system" fn keyboard_hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        let msg = wparam.0 as u32;
        let down = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;
        let up = msg == WM_KEYUP || msg == WM_SYSKEYUP;
        if down || up {
            // SAFETY : pour `HC_ACTION`, `lparam` pointe un `KBDLLHOOKSTRUCT`
            // valide fourni par l'OS pour la durée de l'appel.
            let info = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
            let vk = info.vkCode;
            let is_ctrl = vk == VK_LCONTROL.0 as u32 || vk == VK_RCONTROL.0 as u32;
            let is_win = vk == VK_LWIN.0 as u32 || vk == VK_RWIN.0 as u32;

            let changed = if is_ctrl {
                note_state_change(&CTRL_DOWN, down)
            } else if is_win {
                note_state_change(&WIN_DOWN, down)
            } else {
                false
            };

            if changed {
                let want = CTRL_DOWN.load(Ordering::SeqCst) && WIN_DOWN.load(Ordering::SeqCst);
                if COMBO_WANT.swap(want, Ordering::SeqCst) != want {
                    if let Some(worker) = WORKER.get() {
                        worker.unpark();
                    }
                }
            }
        }
    }
    // `None` = passer au hook suivant de la chaîne (le paramètre `hhk` est
    // ignoré par Windows moderne de toute façon).
    CallNextHookEx(None, code, wparam, lparam)
}

/// Installe le hook Ctrl+Win et démarre le thread de traitement.
///
/// **À appeler sur le thread principal Tauri** (`.setup()`), comme
/// `HotkeyManager::register` : `SetWindowsHookExW(WH_KEYBOARD_LL, …)`
/// s'associe au thread appelant et exige que celui-ci pompe des messages.
///
/// `capture_tx` / `listening` : les mêmes que `HotkeyManager` — le combo
/// Ctrl+Win écrit dans le **même** drapeau `PipelineState::listening` (une
/// seule source de vérité, quel que soit le déclencheur).
pub fn register(
    app: AppHandle,
    capture_tx: Sender<CaptureCommand>,
    listening: Arc<AtomicBool>,
) -> Result<(), String> {
    if INSTALLED.swap(true, Ordering::SeqCst) {
        return Err("hook Ctrl+Win déjà installé".into());
    }

    // Le worker doit avoir publié son `Thread` dans `WORKER` **avant** que le
    // hook puisse déclencher un `unpark` (sinon le tout premier front serait
    // perdu). On l'attend via un canal `ready`, puis seulement on installe
    // le hook. Même schéma que `audio::capture::spawn`.
    let (ready_tx, ready_rx) = bounded::<()>(1);
    std::thread::Builder::new()
        .name("vozel-ctrl-win".into())
        .spawn(move || {
            let _ = WORKER.set(std::thread::current());
            let _ = ready_tx.send(());
            loop {
                std::thread::park();
                // `COMBO_WANT` est la vérité ; on relit les deux atomics
                // pour `decide` (état le plus frais au moment du réveil —
                // absorbe les réveils parasites de `park`).
                let action = decide(
                    CTRL_DOWN.load(Ordering::SeqCst),
                    WIN_DOWN.load(Ordering::SeqCst),
                    listening.load(Ordering::SeqCst),
                );
                match action {
                    ComboAction::Start => {
                        listening.store(true, Ordering::SeqCst);
                        println!("[hotkey] Ctrl+Win maintenu -> listening_started");
                        let _ = capture_tx.send(CaptureCommand::Start);
                        let _ = app.emit("listening_started", ());
                    }
                    ComboAction::Stop => {
                        listening.store(false, Ordering::SeqCst);
                        println!("[hotkey] Ctrl+Win relache -> listening_stopped");
                        let _ = capture_tx.send(CaptureCommand::Stop);
                        let _ = app.emit("listening_stopped", ());
                        // Pipeline complet (transcription -> nettoyage ->
                        // injection), bloquant, sur ce thread dédié — même
                        // choix que le thread d'événements de `HotkeyManager`.
                        crate::commands::run_pipeline(&app);
                    }
                    ComboAction::Nothing => {}
                }
            }
        })
        .map_err(|e| format!("thread Ctrl+Win : {e}"))?;

    ready_rx
        .recv()
        .map_err(|_| "thread Ctrl+Win arrêté avant initialisation".to_string())?;

    // hmod du hook : le module courant (recommandé pour WH_KEYBOARD_LL même
    // si l'OS ne l'injecte pas). `GetModuleHandleW(None)` = l'exécutable.
    let hmod: HINSTANCE = unsafe { GetModuleHandleW(None) }
        .map_err(|e| format!("GetModuleHandleW : {e}"))?
        .into();

    // SAFETY : appelé sur le thread de la boucle de messages Win32 ; `lpfn`
    // est une `extern "system" fn` valide pour toute la vie du process.
    let hook: HHOOK =
        unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook_proc), Some(hmod), 0) }
            .map_err(|e| format!("SetWindowsHookExW(WH_KEYBOARD_LL) : {e}"))?;
    // Jamais désinstallé (l'OS le nettoie à la fermeture), comme le
    // `Box::leak` de `GlobalHotKeyManager`. `HHOOK` est un handle `Copy`
    // sans `Drop` — le laisser sortir de portée ne le libère pas.
    let _ = hook;

    println!("[hotkey] push-to-talk Ctrl+Win seul actif (hook WH_KEYBOARD_LL)");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decide_covers_the_activation_rule() {
        // Démarrage seulement quand les deux sont enfoncés ET qu'on n'écoute
        // pas déjà.
        assert_eq!(decide(true, true, false), ComboAction::Start);
        // Déjà en écoute : ne pas redémarrer.
        assert_eq!(decide(true, true, true), ComboAction::Nothing);
        // Arrêt dès que l'un des deux est relâché, si on écoutait.
        assert_eq!(decide(false, true, true), ComboAction::Stop);
        assert_eq!(decide(true, false, true), ComboAction::Stop);
        assert_eq!(decide(false, false, true), ComboAction::Stop);
        // Rien à faire si on n'écoutait pas et que le combo n'est pas complet.
        assert_eq!(decide(true, false, false), ComboAction::Nothing);
        assert_eq!(decide(false, false, false), ComboAction::Nothing);
    }

    #[test]
    fn note_state_change_ignores_key_repeat() {
        let flag = AtomicBool::new(false);
        assert!(note_state_change(&flag, true), "faux -> vrai = changement");
        assert!(
            !note_state_change(&flag, true),
            "vrai -> vrai (auto-répétition) = pas un changement"
        );
        assert!(note_state_change(&flag, false), "vrai -> faux = changement");
        assert!(!note_state_change(&flag, false), "faux -> faux = pas un changement");
    }
}
