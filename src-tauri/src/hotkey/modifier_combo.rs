//! Push-to-talk sur le **maintien simultané de Ctrl + Win seuls** (sans 3ᵉ
//! touche) — **seul déclencheur de la dictée** (demande utilisateur,
//! 2026-09-02 ; à l'origine Spec_Backend_Desktop.md §2.5, alors optionnel et
//! additionnel au raccourci configurable — celui-ci a depuis été retiré).
//!
//! « Ctrl+Win » à vide n'était pas représentable par la crate `global-hotkey`
//! (`RegisterHotKey` Win32 exige une touche non modificatrice), d'où ce hook
//! bas niveau dédié — devenu l'unique chemin.
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
//! Règle d'activation : démarrage dès que Ctrl **et** Win sont enfoncées
//! (peu importe l'ordre) ; arrêt dès que Ctrl **ou** Win est relâchée ; une
//! 3ᵉ touche pendant le maintien n'annule rien (on ne suit que les VK
//! Ctrl/Win).
//!
//! **Isolation de la touche Windows pendant le combo** (2026-09-02, l'utilisateur
//! a constaté que sans ça le menu Démarrer / des raccourcis Win se déclenchaient) :
//! tant que Ctrl **et** Win sont maintenues (et jusqu'au relâchement des deux),
//! les événements de la touche Win sont **avalés** (pas de `CallNextHookEx`) —
//! Windows se comporte comme si Win n'avait pas été touchée : ni menu Démarrer,
//! ni Win+X, ni Ctrl+Win+D/flèches. Deux cas :
//!  - Win enfoncée alors que Ctrl l'est déjà → on avale la frappe *et* son
//!    relâchement (Windows n'a jamais vu Win baissée, aucun état à recaler) ;
//!  - Win enfoncée **avant** Ctrl → sa frappe initiale a fui vers Windows ;
//!    on avale les répétitions, et au relâchement on injecte une touche
//!    neutre (`SendInput` vk 0x07, non assignée) juste avant de laisser passer
//!    le `key-up`, ce qui annule le menu Démarrer tout en gardant l'état Win
//!    cohérent côté OS.
//! La touche Ctrl n'est jamais avalée (inoffensive, et l'avaler risquerait de
//! casser un Ctrl+C concurrent).

#![cfg(target_os = "windows")]

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Arc;
use std::sync::OnceLock;
use std::thread::Thread;

use crossbeam_channel::{bounded, Sender};
use tauri::{AppHandle, Emitter};
use windows::Win32::Foundation::{HINSTANCE, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
    VIRTUAL_KEY, VK_LCONTROL, VK_LWIN, VK_RCONTROL, VK_RWIN,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, SetWindowsHookExW, HC_ACTION, HHOOK, KBDLLHOOKSTRUCT, WH_KEYBOARD_LL,
    WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
};

use crate::audio::capture::CaptureCommand;

/// Bit `LLKHF_INJECTED` de `KBDLLHOOKSTRUCT.flags` : l'événement vient d'un
/// `SendInput` (le nôtre, ci-dessous) — à laisser passer sans le traiter.
const LLKHF_INJECTED: u32 = 0x10;
/// VK non assignée (comme le `vk07` d'AutoHotkey) : une frappe injectée avec
/// ce code n'a aucun effet applicatif mais compte comme « une touche a été
/// pressée pendant le maintien de Win » → pas de menu Démarrer.
const VK_INERT: u16 = 0x07;

/// État des modificateurs, mis à jour **uniquement** par le callback du hook.
static CTRL_DOWN: AtomicBool = AtomicBool::new(false);
static WIN_DOWN: AtomicBool = AtomicBool::new(false);
/// Dernier « Ctrl && Win » signalé au worker — sert à ne le réveiller que
/// sur un vrai front (montée ou descente du combo), pas à chaque touche.
static COMBO_WANT: AtomicBool = AtomicBool::new(false);
/// [`WinGate`] empaqueté (bit 0 = `suppress`, bit 1 = `down_swallowed`).
/// Écrit/lu uniquement par le hook, qui est appelé en série sur un seul thread.
static WIN_GATE: AtomicU8 = AtomicU8::new(0);
/// Handle du thread worker, pour que le callback puisse le `unpark`.
static WORKER: OnceLock<Thread> = OnceLock::new();
/// Garde-fou : un seul hook installé par process.
static INSTALLED: AtomicBool = AtomicBool::new(false);

/// Injecte une frappe neutre (down+up) pour « salir » un maintien de Win qui a
/// fui vers Windows, afin que le relâchement n'ouvre pas le menu Démarrer.
fn poke_inert_key() {
    let mk = |up: bool| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(VK_INERT),
                wScan: 0,
                dwFlags: if up { KEYEVENTF_KEYUP } else { KEYBD_EVENT_FLAGS(0) },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    let inputs = [mk(false), mk(true)];
    unsafe {
        SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
    }
}

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

/// État d'isolation de la touche Win, muté seulement par le hook (mono-thread).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WinGate {
    /// On isole actuellement Win pour un combo en cours.
    suppress: bool,
    /// On a avalé la frappe initiale de Win (Windows ne l'a pas vue).
    down_swallowed: bool,
}

impl WinGate {
    fn pack(self) -> u8 {
        (self.suppress as u8) | ((self.down_swallowed as u8) << 1)
    }
    fn unpack(bits: u8) -> Self {
        Self {
            suppress: bits & 1 != 0,
            down_swallowed: bits & 2 != 0,
        }
    }
}

/// Ce que le hook doit faire d'un événement de la touche Win.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(crate) enum WinHandling {
    /// Relayer normalement (`CallNextHookEx`).
    Pass,
    /// Avaler (`LRESULT(1)`, pas de `CallNextHookEx`).
    Swallow,
    /// Injecter une frappe neutre puis relayer le `key-up`.
    PokeThenPass,
}

/// Décision pure d'isolation de Win. `down` : keydown vs keyup. `was_win_down`
/// : état de Win *avant* cet événement. `ctrl_down` : Ctrl est enfoncée.
pub(crate) fn gate_win_event(
    gate: &mut WinGate,
    down: bool,
    was_win_down: bool,
    ctrl_down: bool,
) -> WinHandling {
    if down {
        let initial = !was_win_down;
        if ctrl_down || gate.suppress {
            gate.suppress = true;
            if initial {
                gate.down_swallowed = true;
            }
            WinHandling::Swallow
        } else {
            if initial {
                gate.down_swallowed = false;
            }
            WinHandling::Pass
        }
    } else {
        let was_suppressing = gate.suppress;
        if !ctrl_down {
            gate.suppress = false;
        }
        if gate.down_swallowed {
            gate.down_swallowed = false;
            WinHandling::Swallow
        } else if was_suppressing {
            WinHandling::PokeThenPass
        } else {
            WinHandling::Pass
        }
    }
}

/// `true` si l'état de `flag` a réellement changé (ignore les `WM_KEYDOWN`
/// en rafale de l'auto-répétition : `swap(true)` sur un flag déjà `true`
/// renvoie `true`, donc `!= true` est faux).
fn note_state_change(flag: &AtomicBool, down: bool) -> bool {
    flag.swap(down, Ordering::SeqCst) != down
}

/// Callback du hook — chemin ultra-court obligatoire (voir en-tête). Renvoie
/// `LRESULT(1)` (sans `CallNextHookEx`) pour **avaler** une frappe Win faisant
/// partie du combo, sinon relaie normalement.
unsafe extern "system" fn keyboard_hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        let msg = wparam.0 as u32;
        let down = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;
        let up = msg == WM_KEYUP || msg == WM_SYSKEYUP;
        if down || up {
            // SAFETY : pour `HC_ACTION`, `lparam` pointe un `KBDLLHOOKSTRUCT`
            // valide fourni par l'OS pour la durée de l'appel.
            let info = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
            // Nos propres frappes injectées (`poke_inert_key`) : ne rien en
            // faire, juste relayer — sinon boucle / comptage faussé.
            if info.flags.0 & LLKHF_INJECTED != 0 {
                return CallNextHookEx(None, code, wparam, lparam);
            }
            let vk = info.vkCode;
            let is_ctrl = vk == VK_LCONTROL.0 as u32 || vk == VK_RCONTROL.0 as u32;
            let is_win = vk == VK_LWIN.0 as u32 || vk == VK_RWIN.0 as u32;

            let mut swallow = false;

            if is_ctrl {
                note_state_change(&CTRL_DOWN, down);
                // Ctrl relâchée alors que Win l'est aussi → plus aucun combo
                // en cours, on remet l'isolation Win à zéro (sinon `suppress`
                // resterait armé et avalerait un futur appui Win solo).
                if up && !WIN_DOWN.load(Ordering::SeqCst) {
                    WIN_GATE.store(0, Ordering::SeqCst);
                }
            } else if is_win {
                let was_win_down = WIN_DOWN.swap(down, Ordering::SeqCst);
                let mut gate = WinGate::unpack(WIN_GATE.load(Ordering::SeqCst));
                match gate_win_event(
                    &mut gate,
                    down,
                    was_win_down,
                    CTRL_DOWN.load(Ordering::SeqCst),
                ) {
                    WinHandling::Pass => {}
                    WinHandling::Swallow => swallow = true,
                    WinHandling::PokeThenPass => poke_inert_key(),
                }
                WIN_GATE.store(gate.pack(), Ordering::SeqCst);
            }

            // Le worker (`decide`) suit l'état réel des deux modificateurs,
            // que la frappe soit avalée ou non.
            if is_ctrl || is_win {
                let want = CTRL_DOWN.load(Ordering::SeqCst) && WIN_DOWN.load(Ordering::SeqCst);
                if COMBO_WANT.swap(want, Ordering::SeqCst) != want {
                    if let Some(worker) = WORKER.get() {
                        worker.unpark();
                    }
                }
            }

            if swallow {
                return LRESULT(1);
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

    // Raccourcis pour les tests de `gate_win_event` : simule une frappe et
    // renvoie la décision, en tenant `was_win_down` à jour.
    fn win_down(g: &mut WinGate, was_down: bool, ctrl: bool) -> WinHandling {
        gate_win_event(g, true, was_down, ctrl)
    }
    fn win_up(g: &mut WinGate, ctrl: bool) -> WinHandling {
        gate_win_event(g, false, true, ctrl)
    }

    #[test]
    fn win_alone_passes_through() {
        let mut g = WinGate::default();
        assert_eq!(win_down(&mut g, false, false), WinHandling::Pass);
        assert_eq!(win_down(&mut g, true, false), WinHandling::Pass); // répétition
        assert_eq!(win_up(&mut g, false), WinHandling::Pass);
        assert_eq!(g, WinGate::default(), "gate revenu à zéro");
    }

    #[test]
    fn ctrl_then_win_swallows_down_and_up() {
        let mut g = WinGate::default();
        // Ctrl est déjà baissée quand Win descend.
        assert_eq!(win_down(&mut g, false, true), WinHandling::Swallow);
        assert_eq!(win_down(&mut g, true, true), WinHandling::Swallow); // répétition
        // Win relâchée en premier (Ctrl encore baissée).
        assert_eq!(win_up(&mut g, true), WinHandling::Swallow);
        assert!(!g.down_swallowed, "équilibré : down_swallowed consommé");
    }

    #[test]
    fn win_then_ctrl_pokes_on_release() {
        let mut g = WinGate::default();
        // Win descend seule → fuit vers Windows.
        assert_eq!(win_down(&mut g, false, false), WinHandling::Pass);
        // Ctrl s'ajoute : la répétition suivante de Win est avalée.
        assert_eq!(win_down(&mut g, true, true), WinHandling::Swallow);
        // Relâchement : Windows a vu la frappe initiale → on injecte une
        // touche neutre puis on laisse passer le key-up.
        assert_eq!(win_up(&mut g, false), WinHandling::PokeThenPass);
        assert_eq!(g, WinGate::default());
    }

    #[test]
    fn win_solo_after_a_combo_is_not_swallowed() {
        // Reproduit le bug potentiel : après un combo, `suppress` doit être
        // désarmé (le hook remet `WIN_GATE` à zéro sur Ctrl-up ; ici on
        // vérifie qu'un gate à zéro laisse bien passer Win solo).
        let mut g = WinGate::default();
        assert_eq!(win_down(&mut g, false, true), WinHandling::Swallow);
        assert_eq!(win_up(&mut g, false), WinHandling::Swallow);
        // gate désarmé → appui Win solo suivant : Pass.
        assert_eq!(g, WinGate::default());
        assert_eq!(win_down(&mut g, false, false), WinHandling::Pass);
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
