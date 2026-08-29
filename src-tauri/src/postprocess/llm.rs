//! LLM local pour le nettoyage/reformulation avancée et le futur Command
//! Mode (Spec_Backend_Desktop.md §2.3).
//!
//! **Spike Sessions 8-9.** Décision d'architecture (voir
//! `01_Recherche/Approche_LLM_Local.md` + PROGRESS.md) : LLM local via ONNX
//! (crate `ort`, déjà utilisée pour Parakeet-TDT), **pas** `llama.cpp`/GGUF
//! (bloqué par l'absence de LLVM/libclang sur la machine).
//!
//! Contrepartie : `ort` exécute un graphe, point — pas de boucle de
//! génération autorégressive clé en main. Le cache KV et la boucle greedy
//! sont écrits ici à la main au-dessus d'un export ONNX « standard »
//! (`optimum-cli export onnx`, schéma `input_ids` / `attention_mask` /
//! `position_ids` / `past_key_values.N.key|value` → `logits` /
//! `present.N.key|value`). La tokenisation utilise la crate `tokenizers`
//! (lecture directe de `tokenizer.json`).
//!
//! Validé (Session 9) : Qwen2.5-1.5B-Instruct exporté en ONNX fp32 se charge
//! et s'exécute sous `ort` 2.0.0-rc.13 (GQA + RoPE + RMSNorm, aucun
//! opérateur non supporté).

#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use ort::session::Session;
use ort::value::{DynValue, Tensor};
use tauri::{AppHandle, Manager};
use tokenizers::Tokenizer;

/// Emplacement du modèle LLM local dans le répertoire de données de l'app
/// (`%APPDATA%\com.exponentvalue.vozel\models\llm\` sur Windows). Non
/// commité (~1-3 Go) — placement manuel, voir PROGRESS.md.
fn model_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let base = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("répertoire de données introuvable : {e}"))?;
    Ok(base.join("models").join("llm"))
}

/// Géométrie du transformeur + tokens de fin, lues depuis `config.json` /
/// `generation_config.json` de l'export.
#[derive(Debug, Clone)]
pub struct ModelShape {
    pub num_layers: usize,
    /// Têtes K/V (== têtes d'attention si pas de GQA).
    pub num_kv_heads: usize,
    pub head_dim: usize,
    pub eos_token_ids: Vec<i64>,
}

pub struct LlmEngine {
    session: Mutex<Session>,
    tokenizer: Tokenizer,
    shape: ModelShape,
}

fn read_json(path: &Path) -> Result<serde_json::Value, String> {
    let raw = fs::read_to_string(path).map_err(|e| format!("lecture de '{}' : {e}", path.display()))?;
    serde_json::from_str(&raw).map_err(|e| format!("JSON invalide dans '{}' : {e}", path.display()))
}

fn as_usize(v: &serde_json::Value, key: &str) -> Result<usize, String> {
    v.get(key)
        .and_then(|x| x.as_u64())
        .map(|x| x as usize)
        .ok_or_else(|| format!("champ '{key}' absent ou non entier dans config.json"))
}

impl LlmEngine {
    /// Charge le modèle depuis `models/llm/` du répertoire de données de
    /// l'app. Absence = cas recouvrable (le pipeline retombe sur les
    /// règles), pas une erreur fatale — même logique que `asr::local`.
    pub fn load(app: &AppHandle) -> Result<Self, String> {
        Self::load_from_dir(&model_dir(app)?)
    }

    /// Charge un export `optimum` complet depuis `dir` : `config.json`
    /// (géométrie), `tokenizer.json` (tokeniseur), `model.onnx` (+ données
    /// externes dans le même dossier). `generation_config.json` est lu s'il
    /// existe pour les ids de fin (sinon `config.json::eos_token_id`).
    pub fn load_from_dir(dir: &Path) -> Result<Self, String> {
        let cfg = read_json(&dir.join("config.json"))?;
        let num_layers = as_usize(&cfg, "num_hidden_layers")?;
        let num_heads = as_usize(&cfg, "num_attention_heads")?;
        let num_kv_heads = cfg
            .get("num_key_value_heads")
            .and_then(|x| x.as_u64())
            .map(|x| x as usize)
            .unwrap_or(num_heads);
        let head_dim = match cfg.get("head_dim").and_then(|x| x.as_u64()) {
            Some(h) => h as usize,
            None => as_usize(&cfg, "hidden_size")? / num_heads,
        };

        // eos : generation_config.json (peut être une liste) sinon config.json.
        let eos_source = read_json(&dir.join("generation_config.json"))
            .ok()
            .and_then(|g| g.get("eos_token_id").cloned())
            .or_else(|| cfg.get("eos_token_id").cloned());
        let eos_token_ids = match eos_source {
            Some(serde_json::Value::Number(n)) => vec![n.as_i64().unwrap_or_default()],
            Some(serde_json::Value::Array(a)) => {
                a.iter().filter_map(|x| x.as_i64()).collect()
            }
            _ => return Err("aucun eos_token_id trouvé (config/generation_config)".into()),
        };

        let shape = ModelShape {
            num_layers,
            num_kv_heads,
            head_dim,
            eos_token_ids,
        };

        let tokenizer = Tokenizer::from_file(dir.join("tokenizer.json"))
            .map_err(|e| format!("chargement du tokeniseur : {e}"))?;

        let model_path = dir.join("model.onnx");
        let session = Session::builder()
            .map_err(|e| format!("ort::Session::builder : {e}"))?
            .commit_from_file(&model_path)
            .map_err(|e| format!("chargement de '{}' : {e}", model_path.display()))?;

        Ok(Self {
            session: Mutex::new(session),
            tokenizer,
            shape,
        })
    }

    pub fn shape(&self) -> &ModelShape {
        &self.shape
    }

    /// Nettoyage/correction d'un texte dicté via le LLM (gabarit de chat
    /// ChatML, décodage greedy). Le résultat est la réponse de l'assistant,
    /// détokenisée et rognée. `max_new_tokens` borne la génération.
    pub fn clean(&self, raw: &str, max_new_tokens: usize) -> Result<String, String> {
        let raw = raw.trim();
        if raw.is_empty() {
            return Ok(String::new());
        }
        let system = "Tu corriges la ponctuation, les majuscules et les accords d'un texte dicté. \
                      Ne reformule pas, n'ajoute rien, ne commente pas. \
                      Réponds uniquement avec le texte corrigé.";
        let prompt = format!(
            "<|im_start|>system\n{system}<|im_end|>\n\
             <|im_start|>user\n{raw}<|im_end|>\n\
             <|im_start|>assistant\n"
        );

        let enc = self
            .tokenizer
            .encode(prompt, false)
            .map_err(|e| format!("tokenisation : {e}"))?;
        let prompt_ids: Vec<i64> = enc.get_ids().iter().map(|&i| i64::from(i)).collect();

        let gen = self.generate_greedy(&prompt_ids, max_new_tokens, &self.shape.eos_token_ids)?;
        // Retire un éventuel eos final avant décodage.
        let gen_u32: Vec<u32> = gen
            .iter()
            .take_while(|id| !self.shape.eos_token_ids.contains(id))
            .map(|&id| id as u32)
            .collect();

        let text = self
            .tokenizer
            .decode(&gen_u32, true)
            .map_err(|e| format!("détokenisation : {e}"))?;
        Ok(text.trim().to_string())
    }

    /// Un seul forward pass (préfill, pas de passé). Retourne les `logits`
    /// de la dernière position. Témoin que le graphe s'exécute sous `ort`.
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
    /// `prompt_ids` : tokens du prompt déjà mis en forme. Retourne les
    /// tokens **générés** (hors prompt), s'arrête sur un id de `eos_ids` ou
    /// à `max_new_tokens`. Décodage déterministe (argmax).
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

        let mut past_k: Vec<Vec<f32>> = vec![Vec::new(); n_layers];
        let mut past_v: Vec<Vec<f32>> = vec![Vec::new(); n_layers];
        let mut past_len = 0usize;

        let mut cur: Vec<i64> = prompt_ids.to_vec();
        let mut generated: Vec<i64> = Vec::new();

        let mut session = self
            .session
            .lock()
            .map_err(|_| "mutex session LLM empoisonné".to_string())?;

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
    //! commité (voir PROGRESS.md Session 8/9 pour la procédure). Lancer avec
    //! `VOZEL_LLM_MODEL_DIR=<dir> cargo test --lib -- --ignored llm::`.
    use super::*;

    fn model_dir() -> std::path::PathBuf {
        std::env::var("VOZEL_LLM_MODEL_DIR")
            .expect("VOZEL_LLM_MODEL_DIR non défini (dossier de l'export optimum)")
            .into()
    }

    /// Jalon principal du spike : un forward pass d'un export `optimum`
    /// standard s'exécute-t-il sous la version de `ort` du projet, sans
    /// erreur d'opérateur non supporté ?
    #[test]
    #[ignore]
    fn forward_pass_runs_under_ort() {
        let engine = LlmEngine::load_from_dir(&model_dir()).expect("chargement du modèle");
        eprintln!("[llm-spike] shape = {:?}", engine.shape());
        // Prompt court quelconque (les ids exacts importent peu ici).
        let logits = engine
            .forward_prefill(&[9707, 11, 847, 829, 374])
            .expect("forward pass");
        eprintln!(
            "[llm-spike] vocab={} argmax(dernière position)={}",
            logits.len(),
            argmax(&logits)
        );
        assert!(logits.len() > 1000, "vocab suspicieusement petit : {}", logits.len());
    }

    /// Bout-en-bout : un vrai texte dicté (pas des ids pré-calculés) →
    /// tokenisation → boucle greedy → détokenisation → texte corrigé.
    /// Le critère qui compte : la sortie est cohérente à l'œil.
    #[test]
    #[ignore]
    fn clean_produces_coherent_text() {
        let engine = LlmEngine::load_from_dir(&model_dir()).expect("chargement du modèle");
        let raw = "alors voila le texte a corriger je suis aller au marche hier avec mon frere";
        let t0 = std::time::Instant::now();
        let cleaned = engine.clean(raw, 80).expect("clean");
        eprintln!(
            "[llm-spike] {:.1}s\n  entrée : {raw}\n  sortie : {cleaned}",
            t0.elapsed().as_secs_f32()
        );
        assert!(!cleaned.is_empty(), "sortie vide");
    }
}
