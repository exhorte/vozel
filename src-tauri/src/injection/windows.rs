//! Adaptateur d'injection Windows (Phase 1).
//!
//! TODO : injection clavier directe via `SendInput` (API Win32) pour la
//! majorité des apps ; repli sur `injection::clipboard` (Ctrl+V puis
//! restauration du presse-papiers original) pour les apps qui bloquent
//! l'injection synthétique ou pour les gros volumes de texte. UI Automation
//! en secours pour les champs de texte protégés.

use super::TextInjector;

#[allow(dead_code)]
pub struct WindowsInjector;

impl TextInjector for WindowsInjector {
    fn inject(&self, _text: &str) -> Result<(), String> {
        // TODO: SendInput, avec repli clipboard::paste_and_restore.
        Err("not implemented".into())
    }
}
