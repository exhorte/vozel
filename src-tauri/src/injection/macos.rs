//! Adaptateur d'injection macOS (Phase 3).
//!
//! TODO : `CGEventPost` / Accessibility API. Nécessite l'autorisation
//! Accessibilité de l'utilisateur au premier lancement. Repli sur
//! `injection::clipboard` identique à Windows.

use super::TextInjector;

#[allow(dead_code)]
pub struct MacOsInjector;

impl TextInjector for MacOsInjector {
    fn inject(&self, _text: &str) -> Result<(), String> {
        // TODO: CGEventPost, avec repli clipboard::paste_and_restore.
        Err("not implemented".into())
    }
}
