//! "Command Mode" (Phase 2) — reformulation vocale d'une sélection de texte
//! existante (ex. "rends ce paragraphe plus concis", "transforme en liste").
//! Fonctionnalité identifiée comme la plus différenciante côté Wispr Flow
//! (voir analyse section 2).
//!
//! TODO : détecter l'intention de commande vs. dictée normale, puis router
//! vers le LLM local ou cloud selon la config utilisateur.

#[allow(dead_code)]
pub fn handle_command(_instruction: &str, _selected_text: &str) -> Result<String, String> {
    Err("not implemented".into())
}
