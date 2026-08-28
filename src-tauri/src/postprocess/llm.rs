//! LLM local pour le nettoyage/reformulation avancée et le futur Command
//! Mode (Spec_Backend_Desktop.md §2.3).
//!
//! **Spike Session 8, en cours.** Décision d'architecture (voir
//! `01_Recherche/Approche_LLM_Local.md` + PROGRESS.md Session 8) : LLM local
//! via ONNX (crate `ort`, déjà utilisée pour Parakeet-TDT), **pas**
//! `llama.cpp`/GGUF (bloqué par l'absence de LLVM/libclang sur la machine).
//!
//! Contrepartie : `ort` exécute un graphe, point — il n'y a pas de boucle de
//! génération autorégressive clé en main (contrairement à `llama.cpp`). Le
//! cache KV, l'échantillonnage et la tokenisation sont à écrire à la main
//! au-dessus d'un export ONNX « standard » (`optimum-cli export onnx`,
//! schéma `input_ids` / `attention_mask` / `position_ids` /
//! `past_key_values.N.key|value` → `logits` / `present.N.key|value`).
//!
//! Jalon en cours : prouver qu'un simple forward pass tourne sous `ort`.

#![allow(dead_code)]

use std::path::Path;
use std::sync::Mutex;

use ort::session::Session;
use ort::value::{DynValue, Tensor};

/// Géométrie du transformeur, lue depuis `config.json` de l'export.
#[derive(Debug, Clone, Copy)]
pub struct ModelShape {
    pub num_layers: usize,
    /// Têtes K/V (== têtes d'attention si pas de GQA).
    pub num_kv_heads: usize,
    pub head_dim: usize,
    pub eos_token_id: i64,
}

pub struct LlmEngine {
    session: Mutex<Session>,
    shape: ModelShape,
}

impl LlmEngine {
    /// Charge `<dir>/model.onnx`. La forme doit être fournie par l'appelant
    /// (lue depuis `config.json` — pas encore automatisé dans ce spike).
    pub fn load_from_dir(dir: &Path, shape: ModelShape) -> Result<Self, String> {
        let model_path = dir.join("model.onnx");
        let session = Session::builder()
            .map_err(|e| format!("ort::Session::builder : {e}"))?
            .commit_from_file(&model_path)
            .map_err(|e| format!("chargement de '{}' : {e}", model_path.display()))?;
        Ok(Self {
            session: Mutex::new(session),
            shape,
        })
    }

    /// Un seul forward pass. `input_ids` : tokens du prompt (préfill, pas de
    /// passé). Retourne les `logits` de la **dernière** position (taille =
    /// vocab). Sert de témoin que le graphe s'exécute sous `ort` avant
    /// d'écrire la vraie boucle de génération.
    pub fn forward_prefill(&self, input_ids: &[i64]) -> Result<Vec<f32>, String> {
        let seq = input_ids.len();
        if seq == 0 {
            return Err("input_ids vide".into());
        }

        let mut inputs: Vec<(String, DynValue)> = Vec::with_capacity(3 + self.shape.num_layers * 2);

        inputs.push((
            "input_ids".to_string(),
            Tensor::from_array(([1usize, seq], input_ids.to_vec()))
                .map_err(|e| format!("tensor input_ids : {e}"))?
                .into_dyn(),
        ));
        inputs.push((
            "attention_mask".to_string(),
            Tensor::from_array(([1usize, seq], vec![1i64; seq]))
                .map_err(|e| format!("tensor attention_mask : {e}"))?
                .into_dyn(),
        ));
        inputs.push((
            "position_ids".to_string(),
            Tensor::from_array(([1usize, seq], (0..seq as i64).collect::<Vec<_>>()))
                .map_err(|e| format!("tensor position_ids : {e}"))?
                .into_dyn(),
        ));

        // Cache KV vide : [1, num_kv_heads, 0, head_dim].
        for layer in 0..self.shape.num_layers {
            for kind in ["key", "value"] {
                let name = format!("past_key_values.{layer}.{kind}");
                let empty: Vec<f32> = Vec::new();
                let t = Tensor::from_array((
                    [1usize, self.shape.num_kv_heads, 0usize, self.shape.head_dim],
                    empty,
                ))
                .map_err(|e| format!("tensor {name} : {e}"))?;
                inputs.push((name, t.into_dyn()));
            }
        }

        let mut session = self
            .session
            .lock()
            .map_err(|_| "mutex session LLM empoisonné".to_string())?;
        let outputs = session
            .run(inputs)
            .map_err(|e| format!("inférence LLM (forward pass) : {e}"))?;

        let (shape, data) = outputs["logits"]
            .try_extract_tensor::<f32>()
            .map_err(|e| format!("extraction logits : {e}"))?;
        // [1, seq, vocab]
        if shape.len() != 3 {
            return Err(format!("logits de rang {} (attendu 3)", shape.len()));
        }
        let vocab = shape[2] as usize;
        let last = (seq - 1) * vocab;
        Ok(data[last..last + vocab].to_vec())
    }
}

fn argmax(v: &[f32]) -> usize {
    v.iter()
        .enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
        .map(|(i, _)| i)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    //! `#[ignore]` : nécessite un export ONNX présent sur disque, non
    //! commité (voir PROGRESS.md Session 8 pour la procédure). Lancer avec
    //! `VOZEL_LLM_MODEL_DIR=<dir> cargo test --lib -- --ignored llm::`.
    use super::*;

    fn model_dir() -> std::path::PathBuf {
        std::env::var("VOZEL_LLM_MODEL_DIR")
            .expect("VOZEL_LLM_MODEL_DIR non défini (dossier contenant model.onnx)")
            .into()
    }

    /// Jalon principal du spike : un forward pass d'un export `optimum`
    /// standard s'exécute-t-il sous la version de `ort` du projet, sans
    /// erreur d'opérateur non supporté ?
    #[test]
    #[ignore]
    fn forward_pass_runs_under_ort() {
        // Valeurs pour un export gpt2 (`optimum-cli export onnx --model gpt2
        // --task text-generation-with-past`) : 12 couches, 12 têtes, head_dim
        // 64, EOS 50256. Adapter via l'env si un autre modèle est testé.
        let shape = ModelShape {
            num_layers: std::env::var("VOZEL_LLM_LAYERS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(12),
            num_kv_heads: std::env::var("VOZEL_LLM_KV_HEADS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(12),
            head_dim: std::env::var("VOZEL_LLM_HEAD_DIM")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(64),
            eos_token_id: 50256,
        };
        let engine = LlmEngine::load_from_dir(&model_dir(), shape).expect("chargement du modèle");
        let logits = engine
            .forward_prefill(&[15496, 11, 616, 1438, 318])
            .expect("forward pass");
        eprintln!(
            "[llm-spike] vocab={} argmax(dernière position)={}",
            logits.len(),
            argmax(&logits)
        );
        assert!(logits.len() > 1000, "vocab suspicieusement petit : {}", logits.len());
    }
}
