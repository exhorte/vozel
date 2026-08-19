//! Nettoyage et reformulation IA du texte transcrit — couche découplée de
//! l'ASR (voir analyse section 4.4).
//!
//! Phase 1 : ponctuation/grammaire basique + suppression des hésitations.
//! Phase 2 : "Command Mode" (reformulation vocale d'une sélection, ex.
//! "rends ça plus pro") via un petit LLM local (GGUF/llama.cpp) et,
//! en option, un LLM cloud pour les reformulations plus exigeantes.

pub mod cleanup;
pub mod command_mode;
