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

    /// Boucle de génération autorégressive glouton (greedy) avec cache KV
    /// threadé d'un appel à l'autre — la brique que `llama.cpp` fournit clé
    /// en main et qu'il faut écrire à la main au-dessus d'`ort`.
    ///
    /// `prompt_ids` : tokens du prompt déjà mis en forme (gabarit de chat
    /// inclus). Retourne les tokens **générés** (hors prompt), s'arrête sur
    /// un id de `eos_ids` ou à `max_new_tokens`. Pas d'échantillonnage
    /// (température/top-p) pour ce spike — décodage déterministe.
    pub fn generate_greedy(
        &self,
        prompt_ids: &[i64],
        max_new_tokens: usize,
        eos_ids: &[i64],
    ) -> Result<Vec<i64>, String> {
        if prompt_ids.is_empty() {
            return Err("prompt vide".into());
        }
        let n_layers = self.shape.num_layers;
        let kvh = self.shape.num_kv_heads;
        let hd = self.shape.head_dim;

        // Cache KV possédé entre les itérations : chaque couche stocke
        // key/value aplaties, de forme [1, kvh, past_len, hd].
        let mut past_k: Vec<Vec<f32>> = vec![Vec::new(); n_layers];
        let mut past_v: Vec<Vec<f32>> = vec![Vec::new(); n_layers];
        let mut past_len = 0usize;

        let mut cur: Vec<i64> = prompt_ids.to_vec();
        let mut generated: Vec<i64> = Vec::new();

        let mut session = self
            .session
            .lock()
            .map_err(|_| "mutex session LLM empoisonné".to_string())?;

        // +1 : la première itération est le préfill (ne compte pas comme un
        // token généré tant qu'on n'a pas lu son argmax).
        for _ in 0..=max_new_tokens {
            let seq = cur.len();
            let total = past_len + seq;

            let mut inputs: Vec<(String, DynValue)> = Vec::with_capacity(3 + n_layers * 2);
            inputs.push((
                "input_ids".to_string(),
                Tensor::from_array(([1usize, seq], cur.clone()))
                    .map_err(|e| format!("tensor input_ids : {e}"))?
                    .into_dyn(),
            ));
            inputs.push((
                "attention_mask".to_string(),
                Tensor::from_array(([1usize, total], vec![1i64; total]))
                    .map_err(|e| format!("tensor attention_mask : {e}"))?
                    .into_dyn(),
            ));
            inputs.push((
                "position_ids".to_string(),
                Tensor::from_array((
                    [1usize, seq],
                    (past_len as i64..total as i64).collect::<Vec<_>>(),
                ))
                .map_err(|e| format!("tensor position_ids : {e}"))?
                .into_dyn(),
            ));
            for l in 0..n_layers {
                inputs.push((
                    format!("past_key_values.{l}.key"),
                    Tensor::from_array(([1usize, kvh, past_len, hd], past_k[l].clone()))
                        .map_err(|e| format!("tensor past.{l}.key : {e}"))?
                        .into_dyn(),
                ));
                inputs.push((
                    format!("past_key_values.{l}.value"),
                    Tensor::from_array(([1usize, kvh, past_len, hd], past_v[l].clone()))
                        .map_err(|e| format!("tensor past.{l}.value : {e}"))?
                        .into_dyn(),
                ));
            }

            let outputs = session
                .run(inputs)
                .map_err(|e| format!("inférence LLM (génération) : {e}"))?;

            let (lshape, ldata) = outputs["logits"]
                .try_extract_tensor::<f32>()
                .map_err(|e| format!("extraction logits : {e}"))?;
            let vocab = lshape[2] as usize;
            let last = (seq - 1) * vocab;
            let next = argmax(&ldata[last..last + vocab]) as i64;

            for l in 0..n_layers {
                let (_, k) = outputs[format!("present.{l}.key").as_str()]
                    .try_extract_tensor::<f32>()
                    .map_err(|e| format!("extraction present.{l}.key : {e}"))?;
                let (_, v) = outputs[format!("present.{l}.value").as_str()]
                    .try_extract_tensor::<f32>()
                    .map_err(|e| format!("extraction present.{l}.value : {e}"))?;
                past_k[l] = k.to_vec();
                past_v[l] = v.to_vec();
            }
            past_len = total;

            generated.push(next);
            if eos_ids.contains(&next) || generated.len() >= max_new_tokens {
                break;
            }
            cur = vec![next];
        }

        Ok(generated)
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

    fn env_usize(key: &str, default: usize) -> usize {
        std::env::var(key)
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(default)
    }

    // Géométrie via l'env (défauts = export gpt2). Pour Qwen2.5-1.5B :
    // VOZEL_LLM_LAYERS=28 VOZEL_LLM_KV_HEADS=2 VOZEL_LLM_HEAD_DIM=128.
    fn shape_from_env() -> ModelShape {
        ModelShape {
            num_layers: env_usize("VOZEL_LLM_LAYERS", 12),
            num_kv_heads: env_usize("VOZEL_LLM_KV_HEADS", 12),
            head_dim: env_usize("VOZEL_LLM_HEAD_DIM", 64),
            eos_token_id: 50256,
        }
    }

    /// Jalon principal du spike : un forward pass d'un export `optimum`
    /// standard s'exécute-t-il sous la version de `ort` du projet, sans
    /// erreur d'opérateur non supporté ?
    #[test]
    #[ignore]
    fn forward_pass_runs_under_ort() {
        let engine =
            LlmEngine::load_from_dir(&model_dir(), shape_from_env()).expect("chargement du modèle");
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

    /// Boucle de génération complète : prompt (tokens fournis via
    /// `VOZEL_LLM_PROMPT_IDS`, générés par `_llm_spike/prompt_fixture.py`) →
    /// tokens générés. On imprime les ids : à comparer au décodage de
    /// référence Python (`prompt_fixture.py ... refgen`).
    #[test]
    #[ignore]
    fn greedy_generation_produces_tokens() {
        let prompt_ids: Vec<i64> = std::env::var("VOZEL_LLM_PROMPT_IDS")
            .expect("VOZEL_LLM_PROMPT_IDS non défini (ids séparés par des virgules)")
            .split(',')
            .map(|s| s.trim().parse().expect("id non entier"))
            .collect();
        let eos: Vec<i64> = std::env::var("VOZEL_LLM_EOS")
            .unwrap_or_else(|_| "151645,151643".to_string())
            .split(',')
            .map(|s| s.trim().parse().unwrap())
            .collect();
        let max_new = env_usize("VOZEL_LLM_MAX_NEW", 60);

        let engine =
            LlmEngine::load_from_dir(&model_dir(), shape_from_env()).expect("chargement du modèle");
        let t0 = std::time::Instant::now();
        let out = engine
            .generate_greedy(&prompt_ids, max_new, &eos)
            .expect("génération");
        let dt = t0.elapsed();
        eprintln!(
            "[llm-spike] {} tokens générés en {:.1}s ({:.1} tok/s)\nids: {:?}",
            out.len(),
            dt.as_secs_f32(),
            out.len() as f32 / dt.as_secs_f32(),
            out
        );
        assert!(!out.is_empty(), "aucun token généré");
    }
}
