//! Nettoyage et reformulation IA du texte transcrit — couche découplée de
//! l'ASR (voir analyse section 4.4).
//!
//! Phase 1 : ponctuation/grammaire basique + suppression des hésitations.
//! Phase 2 §2.3 : "Command Mode" (reformulation vocale d'une sélection, ex.
//! "rends ça plus pro") via un petit LLM local — **ONNX/`ort`**, pas
//! GGUF/llama.cpp (voir `llm` + `01_Recherche/Approche_LLM_Local.md`).

pub mod cleanup;
pub mod command_mode;
pub mod llm;
