//! Repli commun à tous les OS : copier le texte dans le presse-papiers,
//! simuler Ctrl+V/Cmd+V, puis restaurer le contenu original du
//! presse-papiers. À utiliser quand l'injection clavier directe échoue
//! (apps sandboxées, terminaux, IDE avec autocomplétion agressive) ou pour
//! de gros volumes de texte (voir `injection::windows::DIRECT_INJECTION_MAX_CHARS`).

use std::{thread, time::Duration};

use arboard::Clipboard;
use enigo::{Direction, Enigo, Key, Keyboard, Settings};

/// Délai avant restauration du presse-papiers original — laisse le temps à
/// l'app cible de traiter le Ctrl+V avant qu'on change le contenu sous elle
/// (sans ce délai, on risque de restaurer l'ancien contenu avant même que
/// le collage n'ait eu lieu côté app cible).
const RESTORE_DELAY: Duration = Duration::from_millis(300);

/// Sauvegarde le presse-papiers courant (texte), pose `text`, simule
/// Ctrl+V, puis restaure le contenu original après un court délai —
/// Spec_Backend_Desktop.md §1.5 point 2. Un presse-papiers initialement
/// vide ou contenant une image/du contenu non-texte n'empêche pas
/// l'opération : il n'y a simplement rien à restaurer ensuite (le
/// presse-papiers est vidé plutôt que laissé au texte transcrit).
pub fn paste_and_restore(text: &str) -> Result<(), String> {
    if text.is_empty() {
        return Ok(());
    }

    let mut clipboard = Clipboard::new().map_err(|e| format!("accès presse-papiers : {e}"))?;
    let previous = clipboard.get_text().ok();

    clipboard.set_text(text).map_err(|e| format!("écriture presse-papiers : {e}"))?;

    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| format!("init enigo (Ctrl+V) : {e}"))?;
    enigo.key(Key::Control, Direction::Press).map_err(|e| format!("Ctrl (press) : {e}"))?;
    enigo.key(Key::V, Direction::Click).map_err(|e| format!("V (click) : {e}"))?;
    enigo.key(Key::Control, Direction::Release).map_err(|e| format!("Ctrl (release) : {e}"))?;

    thread::sleep(RESTORE_DELAY);

    // Le collage a déjà eu lieu à ce stade : une erreur ici signale un échec
    // de la *restauration*, pas du collage lui-même — remontée quand même
    // (ne jamais laisser le presse-papiers de l'utilisateur silencieusement
    // altéré, critère d'acceptation explicite du §1.5).
    match previous {
        Some(prev) => clipboard.set_text(prev).map_err(|e| format!("restauration du presse-papiers : {e}")),
        None => clipboard.clear().map_err(|e| format!("effacement du presse-papiers : {e}")),
    }
}
