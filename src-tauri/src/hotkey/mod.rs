//! Déclenchement de la dictée.
//!
//! **Un seul mécanisme** (demande utilisateur, 2026-09-02) : le **maintien
//! simultané de Ctrl + Win** (sans 3ᵉ touche), via un hook clavier bas niveau
//! `WH_KEYBOARD_LL` — voir `modifier_combo`. Maintenir = enregistrer,
//! relâcher l'une des deux = arrêter puis transcrire + insérer dans le champ
//! actif (`commands::run_pipeline`).
//!
//! L'ancien raccourci global configurable (crate `global-hotkey`,
//! `Toggle`/`PushToTalk`) a été retiré — écart assumé vs
//! `Spec_Backend_Desktop.md` §1.1, documenté dans `PROGRESS.md`.

/// Push-to-talk sur le maintien de Ctrl + Win seuls. Windows uniquement
/// (hook `WH_KEYBOARD_LL`). Seul déclencheur de la dictée.
#[cfg(target_os = "windows")]
pub mod modifier_combo;
