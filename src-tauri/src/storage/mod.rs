//! Persistance locale : dictionnaire personnalisé et réglages utilisateur.
//!
//! SQLite embarqué (plugin SQL de Tauri) plutôt qu'un fichier plat, pour
//! permettre recherche/fuzzy-matching efficace sur le vocabulaire
//! personnalisé (voir analyse section 4.6).

pub mod db;
pub mod dictionary;
pub mod history;
pub mod settings;
