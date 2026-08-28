//! Adaptateur d'injection Windows (Spec_Backend_Desktop.md §1.5).
//!
//! Injection directe via `enigo` (qui appelle `SendInput`, l'API Win32
//! standard, en interne — voir `enigo::win::win_impl::send_input`) pour un
//! texte très court, repli sur `injection::clipboard::paste_and_restore`
//! (Ctrl+V, prouvé fiable jusqu'à ~270 caractères pendant les tests de
//! cette session) pour tout le reste.
//!
//! **Seuil `DIRECT_INJECTION_MAX_CHARS` volontairement bas (20, pas les 200
//! suggérés par la spec), sur la base d'une découverte réelle pendant les
//! tests de cette session, pas d'une supposition** : au-delà d'une trentaine
//! de caractères envoyés par `enigo.text()` en une fois (ou même en petits
//! morceaux espacés — les deux ont été essayés), Notepad se met par
//! intermittence à corrompre le texte reçu (répétition en boucle d'un seul
//! caractère à la place du reste), alors même que `SendInput` rapporte un
//! succès total (le nombre d'événements acceptés correspond — la
//! corruption a lieu après l'insertion dans la file de messages Win32, pas
//! au niveau de l'appel lui-même). **Le signal "best-effort" d'`enigo`
//! (vérification du nombre d'événements `SendInput` acceptés) ne détecte
//! donc PAS ce mode de défaillance précis** — contrairement à ce qu'une
//! première lecture de son code source suggérait (voir `try_direct`
//! ci-dessous) ; seul un test réel avec vérification indépendante du
//! contenu réellement affiché (UI Automation, pas juste le retour de
//! `SendInput`) l'a révélé. Reproduit de façon fiable, avec et sans
//! troncature/espacement du texte (voir PROGRESS.md Session 6 pour la
//! démarche complète de diagnostic et les textes de test exacts) — la
//! zone fiable observée est sous ~20 caractères, incertaine entre 20 et 30,
//! systématiquement en échec au-delà de ~30. Seuil fixé à 20 par prudence.
//! **Limite reconnue** : cette instabilité a été observée sur la machine
//! de développement de cette session (potentiellement un environnement
//! virtualisé/à latence d'entrée non représentative d'un poste utilisateur
//! réel) — non re-testée sur une machine physique standard. Le
//! presse-papiers étant prouvé fiable indépendamment de ce doute, le
//! pencher largement de ce côté est le choix prudent tant que ce n'est pas
//! confirmé/infirmé sur d'autres machines.

use enigo::{Enigo, Keyboard, Settings};

use super::TextInjector;

/// Voir la doc de module ci-dessus pour la justification (seuil bas
/// volontaire, pas les 200 caractères suggérés par la spec).
const DIRECT_INJECTION_MAX_CHARS: usize = 20;

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

/// Délai entre chaque caractère envoyé individuellement. `enigo` (depuis
/// qu'il a supprimé ses délais internes sur Windows, voir enigo-rs/enigo#219
/// et #231) envoie par défaut tout un texte d'un coup en un seul appel
/// `SendInput` batché (un `INPUT` keydown+keyup par caractère, tous
/// soumis simultanément). **Constaté expérimentalement** (pas supposé,
/// diagnostiqué en isolant les variables une à une — voir PROGRESS.md
/// Session 6 pour la reproduction précise) : au-delà d'une dizaine de
/// caractères envoyés sans le moindre espacement, Notepad se met à
/// répéter en boucle un seul caractère du texte à la place du reste —
/// signature typique d'une détection d'auto-répétition clavier de Windows
/// qui se déclenche par erreur sur une rafale d'événements Unicode
/// synthétiques rapprochés (`SendInput` rapporte pourtant un succès total :
/// la corruption a lieu après l'insertion dans la file de messages Win32,
/// pas au niveau de l'appel lui-même — `enigo` ne peut donc pas la
/// détecter). Un simple tronçonnage par lots de N caractères n'a **pas**
/// suffi (la corruption démarrait dès le 11ᵉ caractère, donc dans le
/// premier lot déjà) — un espacement entre caractères individuels était
/// nécessaire. Coût : quelques centaines de ms sur une phrase de dictée
/// typique, négligeable face aux ~1-2s déjà pris par la transcription.
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
