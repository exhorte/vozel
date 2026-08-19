//! Dictionnaire personnalisé : remplacements de mots/expressions et
//! vocabulaire spécialisé appris au fil du temps (noms propres, jargon
//! métier) — voir analyse section 2, facteur de rétention identifié chez
//! Wispr Flow et Freestyle.
//!
//! TODO (Phase 2) : CRUD sur la table `dictionary`, appliqué dans
//! `postprocess::cleanup`.

#[allow(dead_code)]
pub struct DictionaryEntry {
    pub from: String,
    pub to: String,
}
