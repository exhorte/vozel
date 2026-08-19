//! Repli commun à tous les OS : copier le texte dans le presse-papiers,
//! simuler Ctrl+V/Cmd+V, puis restaurer le contenu original du
//! presse-papiers. À utiliser quand l'injection clavier directe échoue
//! (apps sandboxées, terminaux, IDE avec autocomplétion agressive) ou pour
//! de gros volumes de texte.

#[allow(dead_code)]
pub fn paste_and_restore(_text: &str) -> Result<(), String> {
    // TODO: sauvegarder le presse-papiers courant, coller, restaurer.
    Err("not implemented".into())
}
