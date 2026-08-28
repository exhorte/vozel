//! Adaptateur d'injection Windows (Spec_Backend_Desktop.md §1.5).
//!
//! Injection directe via `enigo` (qui appelle `SendInput`, l'API Win32
//! standard, en interne — voir `enigo::win::win_impl::send_input`). Choix
//! documenté dans `PROGRESS.md` Session 6 : `enigo` plutôt que la crate
//! `windows` + `SendInput` à la main, car `enigo` vérifie déjà en interne
//! le nombre d'événements réellement acceptés par `SendInput` contre le
//! nombre attendu et retourne une erreur en cas d'écart — exactement le
//! signal "best-effort" de succès/échec demandé par la spec, sans avoir à
//! réimplémenter cette vérification ni à inventer un timeout arbitraire
//! (`SendInput` répond de façon synchrone, un timeout n'aurait rien détecté
//! de plus fiable). Migration vers `SendInput` natif (crate `windows`)
//! envisageable plus tard si `enigo` s'avère insuffisant sur des apps
//! spécifiques — pas rencontré lors des tests de cette session (voir
//! tableau de résultats dans `PROGRESS.md`).
//!
//! Repli sur `injection::clipboard::paste_and_restore` dans deux cas :
//! texte long (au-delà d'un seuil, une frappe synthétique caractère par
//! caractère devient lente/visible et certaines apps réagissent mal à une
//! frappe prolongée), ou échec de l'injection directe (signal ci-dessus).

use enigo::{Enigo, Keyboard, Settings};

use super::TextInjector;

/// Au-delà de cette longueur, bascule directement sur le presse-papiers
/// (Spec_Backend_Desktop.md §1.5 point 3, seuil suggéré par la spec).
const DIRECT_INJECTION_MAX_CHARS: usize = 200;

pub struct WindowsInjector;

impl TextInjector for WindowsInjector {
    fn inject(&self, text: &str) -> Result<(), String> {
        if text.is_empty() {
            return Ok(());
        }

        if text.chars().count() > DIRECT_INJECTION_MAX_CHARS {
            return super::clipboard::paste_and_restore(text);
        }

        match try_direct(text) {
            Ok(()) => Ok(()),
            Err(e) => {
                eprintln!("[injection] injection directe échouée ({e}), repli presse-papiers");
                super::clipboard::paste_and_restore(text)
            }
        }
    }
}

fn try_direct(text: &str) -> Result<(), String> {
    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| format!("init enigo : {e}"))?;
    enigo.text(text).map_err(|e| format!("SendInput (via enigo) : {e}"))
}

#[cfg(test)]
mod tests {
    //! Tests manuels réels (pas des mocks) — `#[ignore]` par défaut, à
    //! lancer un par un pendant que l'app cible a le focus (voir
    //! PROGRESS.md Session 6 pour la procédure et le tableau de résultats
    //! par app, Spec_Backend_Desktop.md §1.5 critère d'acceptation).
    //!
    //! Méthode de vérification indépendante de l'app cible : après
    //! l'injection, on simule Ctrl+A puis Ctrl+C dans l'app elle-même (donc
    //! avec son propre mécanisme de sélection/copie, pas une lecture de
    //! fenêtre bas niveau spécifique à un type de contrôle), puis on relit
    //! le presse-papiers pour comparer au texte attendu.
    use super::*;
    use arboard::Clipboard;
    use enigo::{Direction, Key};
    use std::{thread, time::Duration};
    use windows::Win32::Foundation::{HWND, LPARAM};
    use windows::core::BOOL;
    use windows::Win32::UI::WindowsAndMessaging::{EnumWindows, GetWindowTextW, IsWindowVisible, SetForegroundWindow};

    /// Cherche une fenêtre top-level visible dont le titre contient `needle`
    /// (insensible à la casse) et la met au premier plan. Nécessaire car le
    /// terminal exécutant `cargo test` reprend le focus OS au démarrage du
    /// process de test — constaté lors du premier essai de cette session
    /// (voir PROGRESS.md Session 6) : sans ce re-focus explicite, le texte
    /// atterrit dans le terminal plutôt que dans l'app cible.
    fn focus_window_containing(needle: &str) -> bool {
        struct SearchState {
            needle_lower: String,
            found: Option<isize>,
        }

        unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
            unsafe {
                let state = &mut *(lparam.0 as *mut SearchState);
                if IsWindowVisible(hwnd).as_bool() {
                    let mut buf = [0u16; 512];
                    let len = GetWindowTextW(hwnd, &mut buf);
                    if len > 0 {
                        let title = String::from_utf16_lossy(&buf[..len as usize]);
                        if title.to_lowercase().contains(&state.needle_lower) {
                            state.found = Some(hwnd.0 as isize);
                            return BOOL(0);
                        }
                    }
                }
                BOOL(1)
            }
        }

        let mut state = SearchState { needle_lower: needle.to_lowercase(), found: None };
        unsafe {
            let _ = EnumWindows(Some(enum_proc), LPARAM(&mut state as *mut SearchState as isize));
        }
        match state.found {
            Some(raw) => {
                let hwnd = HWND(raw as *mut core::ffi::c_void);
                unsafe {
                    let _ = SetForegroundWindow(hwnd);
                }
                thread::sleep(Duration::from_millis(300));
                true
            }
            None => false,
        }
    }

    fn select_all_and_copy() -> String {
        let mut enigo = Enigo::new(&Settings::default()).expect("init enigo (vérification)");
        enigo.key(Key::Control, Direction::Press).unwrap();
        enigo.key(Key::A, Direction::Click).unwrap();
        enigo.key(Key::Control, Direction::Release).unwrap();
        thread::sleep(Duration::from_millis(150));
        enigo.key(Key::Control, Direction::Press).unwrap();
        enigo.key(Key::C, Direction::Click).unwrap();
        enigo.key(Key::Control, Direction::Release).unwrap();
        thread::sleep(Duration::from_millis(250));
        Clipboard::new().and_then(|mut c| c.get_text()).unwrap_or_default()
    }

    /// Titre (ou sous-chaîne) de la fenêtre cible, passé via variable
    /// d'environnement `VOZEL_TEST_WINDOW_TITLE` (ex. `notepad`, `Visual
    /// Studio Code`, `Discord`) — évite de coder en dur une app par test.
    fn target_window_title() -> String {
        std::env::var("VOZEL_TEST_WINDOW_TITLE")
            .expect("VOZEL_TEST_WINDOW_TITLE non défini (sous-chaîne du titre de la fenêtre cible)")
    }

    /// Injection courte (< 200 caractères -> chemin direct `SendInput`).
    #[test]
    #[ignore]
    fn manual_direct_injection() {
        let title = target_window_title();
        assert!(focus_window_containing(&title), "fenêtre contenant '{title}' introuvable");
        let text = std::env::var("VOZEL_TEST_TEXT")
            .unwrap_or_else(|_| "Bonjour, ceci est un test Vozel avec des accents éàçùê et un 42 !".to_string());
        WindowsInjector.inject(&text).expect("injection directe");
        eprintln!("[test] injecté : {text:?}");
        thread::sleep(Duration::from_millis(300));
        let got = select_all_and_copy();
        eprintln!("[test] relu depuis le presse-papiers (Ctrl+A puis Ctrl+C dans l'app) : {got:?}");
        eprintln!("[test] attendu                                                         : {text:?}");
    }

    /// Injection longue (> 200 caractères -> chemin presse-papiers direct,
    /// pas de fallback). Vérifie aussi que le presse-papiers original est
    /// bien restauré après coup.
    #[test]
    #[ignore]
    fn manual_clipboard_fallback_injection() {
        let title = target_window_title();
        assert!(focus_window_containing(&title), "fenêtre contenant '{title}' introuvable");

        let sentinel = "SENTINEL-AVANT-TEST-VOZEL";
        Clipboard::new().unwrap().set_text(sentinel).unwrap();

        let text = "Ceci est un texte long pour forcer le repli sur le presse-papiers. "
            .repeat(4);
        assert!(text.chars().count() > 200, "le texte de test doit dépasser le seuil de 200 caractères");
        WindowsInjector.inject(&text).expect("injection via presse-papiers");
        eprintln!("[test] injecté ({} caractères)", text.chars().count());
        thread::sleep(Duration::from_millis(300));

        // Vérifier la restauration du presse-papiers AVANT tout Ctrl+C de
        // vérification ci-dessous, qui écraserait sinon le résultat qu'on
        // essaie de lire (constaté lors du premier essai de cette session).
        let restored = Clipboard::new().unwrap().get_text().unwrap_or_default();
        eprintln!(
            "[test] presse-papiers après injection : {restored:?} (doit valoir le sentinel '{sentinel}' si la restauration a fonctionné)"
        );

        let got = select_all_and_copy();
        eprintln!("[test] relu depuis l'app : {got:?}");
        eprintln!("[test] attendu            : {text:?}");
    }
}
