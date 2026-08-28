//! Moteur ASR local — Parakeet-TDT (NeMo Conformer-TDT multilingue) via
//! ONNX Runtime (`ort`). Moteur retenu par défaut suite au benchmark
//! français (voir `../01_Recherche/Benchmark_ASR_FR.md` et `PROGRESS.md`
//! Session 4 : `nvidia/parakeet-tdt-0.6b-v3`, ~4-5x plus rapide et WER plus
//! bas que whisper.cpp sur l'échantillon testé).
//!
//! Port direct du pipeline du paquet Python `onnx-asr`
//! (github.com/istupakov/onnx-asr, auteur de l'export ONNX utilisé pendant
//! le benchmark) — pas une réinvention approximative : le contrat exact a
//! été vérifié en lisant `onnx_asr/models/nemo.py` et
//! `onnx_asr/preprocessors/preprocessor.py`, et par introspection directe
//! des fichiers .onnx (`onnxruntime.InferenceSession(...).get_inputs()`),
//! plutôt que deviné. Trois graphes ONNX distincts, chargés une seule fois
//! au démarrage (comme `hotkey`/`audio`, pas de rechargement par appel) :
//!
//! 1. **Préprocesseur** (`nemo128.onnx`, bundlé dans le paquet Python
//!    `onnx_asr` — copié depuis là plutôt que le fichier du même nom
//!    présent dans le dépôt HuggingFace du modèle, qui n'est PAS celui
//!    réellement utilisé par `onnx-asr` à l'exécution) : calcule un
//!    mel-spectrogramme 128 canaux. `waveforms(f32,[1,N])` +
//!    `waveforms_lens(i64,[1])` -> `features(f32,[1,128,T])` +
//!    `features_lens(i64,[1])`.
//! 2. **Encoder** (Conformer) : `audio_signal(f32,[1,128,T])` +
//!    `length(i64,[1])` -> `outputs(f32,[1,1024,T'])` +
//!    `encoded_lengths(i64,[1])`.
//! 3. **Decoder+Joint** (réseau prédicteur TDT, LSTM 2 couches x640 +
//!    joint network) : appelé en boucle autorégressive, PAS un simple
//!    argmax comme pour un modèle CTC — TDT prédit à chaque étape un
//!    token ET une durée de saut (0 à 4 frames encoder à sauter avant le
//!    prochain appel). `encoder_outputs(f32,[1,1024,1])` +
//!    `targets(i32,[1,1])` (dernier token émis, ou blank au départ) +
//!    `target_length(i32,[1])` + `input_states_1/2(f32,[2,1,640])` (états
//!    LSTM, mis à jour seulement quand un token non-blank est émis) ->
//!    `outputs(f32,[1,1,1,8198])` (8198 = 8193 taille vocab dont blank
//!    id 8192, + 5 classes de durée TDT) + `output_states_1/2`.
//!
//! Boucle de décodage : voir `_AsrWithTransducerDecoding._decoding` +
//! `NemoConformerTdt._decode` dans `onnx_asr/asr.py` et
//! `onnx_asr/models/nemo.py` — portée ligne à ligne ci-dessous.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use ort::session::Session;
use ort::value::Tensor;
use tauri::{AppHandle, Manager};

use super::types::TranscriptionResult;
use super::AsrEngine;

const VOCAB_SIZE: usize = 8193;
const BLANK_ID: i64 = 8192;
const FEATURES_SIZE: usize = 128;
const ENCODER_DIM: usize = 1024;
const LSTM_LAYERS: usize = 2;
const LSTM_HIDDEN: usize = 640;
const MAX_TOKENS_PER_STEP: usize = 10;

const MODEL_FILES: &[&str] = &[
    "nemo128.onnx",
    "encoder-model.int8.onnx",
    "decoder_joint-model.int8.onnx",
    "vocab.txt",
];

pub struct LocalAsrEngine {
    preprocessor: Mutex<Session>,
    encoder: Mutex<Session>,
    decoder_joint: Mutex<Session>,
    /// Vocabulaire indexé par id de token (0..VOCAB_SIZE). Les `▁`
    /// SentencePiece sont déjà convertis en espace au chargement (même
    /// logique que `onnx_asr/asr.py::_AsrWithDecoding.__init__`).
    vocab: Vec<String>,
    /// Regex de détokenisation identique à `onnx_asr.asr._AsrWithDecoding.
    /// DECODE_SPACE_PATTERN` (\A\s|\s\B|(\s)\b) : retire l'espace en début
    /// de chaîne, retire les espaces qui ne sont pas à une frontière de
    /// mot (ex. avant une ponctuation), normalise les espaces à une
    /// frontière de mot en un seul espace.
    detokenize_re: regex::Regex,
}

/// Sous-répertoire du répertoire de données de l'app où placer les 4
/// fichiers du modèle (voir doc de module). Pas commité dans le dépôt Git
/// (~670 Mo, bien au-delà de ce qui doit être versionné) — l'utilisateur
/// (ou un futur mécanisme de téléchargement, hors périmètre Phase 1) doit
/// les y déposer manuellement pour l'instant.
fn model_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let base = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("répertoire de données introuvable : {e}"))?;
    Ok(base.join("models").join("parakeet-tdt-v3"))
}

impl LocalAsrEngine {
    /// Résout le répertoire modèle depuis le répertoire de données de
    /// l'app, vérifie la présence des 4 fichiers requis avec un message
    /// clair si absents (premier lancement sans modèle installé — cas
    /// attendu, ne doit jamais faire planter l'app), puis charge.
    pub fn load(app: &AppHandle) -> Result<Self, String> {
        let dir = model_dir(app)?;
        let missing: Vec<&str> = MODEL_FILES
            .iter()
            .filter(|f| !dir.join(f).is_file())
            .copied()
            .collect();
        if !missing.is_empty() {
            return Err(format!(
                "modèle Parakeet-TDT introuvable dans '{}' (fichiers manquants : {}). \
                 Copiez-y les fichiers du modèle (encoder-model.int8.onnx, \
                 decoder_joint-model.int8.onnx, nemo128.onnx, vocab.txt — voir \
                 istupakov/parakeet-tdt-0.6b-v3-onnx sur HuggingFace) pour activer \
                 la dictée locale.",
                dir.display(),
                missing.join(", ")
            ));
        }
        Self::load_from_dir(&dir)
    }

    /// Charge directement depuis un répertoire donné (contourne la
    /// résolution via `AppHandle` — utilisé pour les tests d'intégration
    /// avec un répertoire de modèle arbitraire).
    pub fn load_from_dir(dir: &Path) -> Result<Self, String> {
        let preprocessor = Session::builder()
            .map_err(|e| format!("ort::Session::builder (préprocesseur) : {e}"))?
            .commit_from_file(dir.join("nemo128.onnx"))
            .map_err(|e| format!("chargement du préprocesseur : {e}"))?;
        let encoder = Session::builder()
            .map_err(|e| format!("ort::Session::builder (encoder) : {e}"))?
            .commit_from_file(dir.join("encoder-model.int8.onnx"))
            .map_err(|e| format!("chargement de l'encoder : {e}"))?;
        let decoder_joint = Session::builder()
            .map_err(|e| format!("ort::Session::builder (decoder_joint) : {e}"))?
            .commit_from_file(dir.join("decoder_joint-model.int8.onnx"))
            .map_err(|e| format!("chargement du decoder_joint : {e}"))?;
        let vocab = load_vocab(&dir.join("vocab.txt"))?;
        let detokenize_re = regex::Regex::new(r"\A\s|\s\B|(\s)\b")
            .map_err(|e| format!("regex de détokenisation invalide : {e}"))?;

        Ok(Self {
            preprocessor: Mutex::new(preprocessor),
            encoder: Mutex::new(encoder),
            decoder_joint: Mutex::new(decoder_joint),
            vocab,
            detokenize_re,
        })
    }

    fn detokenize(&self, tokens: &[i64]) -> String {
        let joined: String = tokens
            .iter()
            .map(|&id| self.vocab.get(id as usize).map(String::as_str).unwrap_or(""))
            .collect();
        self.detokenize_re
            .replace_all(&joined, |caps: &regex::Captures| {
                if caps.get(1).is_some() {
                    " ".to_string()
                } else {
                    String::new()
                }
            })
            .into_owned()
    }
}

/// Format `vocab.txt` : une ligne par token, `"<token> <id>"`. Les
/// marqueurs SentencePiece `▁` (U+2581, début de mot) sont convertis en
/// espace littéral, comme côté Python (`onnx_asr/asr.py`).
fn load_vocab(path: &Path) -> Result<Vec<String>, String> {
    let raw = std::fs::read_to_string(path).map_err(|e| format!("lecture de '{}' : {e}", path.display()))?;
    let mut vocab = vec![String::new(); VOCAB_SIZE];
    for line in raw.lines() {
        let Some((token, id_str)) = line.rsplit_once(' ') else {
            continue;
        };
        let Ok(id) = id_str.parse::<usize>() else {
            continue;
        };
        if id < vocab.len() {
            vocab[id] = token.replace('\u{2581}', " ");
        }
    }
    Ok(vocab)
}

fn argmax(values: &[f32]) -> (usize, f32) {
    let mut best_idx = 0;
    let mut best_val = f32::NEG_INFINITY;
    for (i, &v) in values.iter().enumerate() {
        if v > best_val {
            best_val = v;
            best_idx = i;
        }
    }
    (best_idx, best_val)
}

impl AsrEngine for LocalAsrEngine {
    fn transcribe(&self, audio_pcm16k: &[f32]) -> Result<TranscriptionResult, String> {
        if audio_pcm16k.is_empty() {
            return Ok(TranscriptionResult { text: String::new(), language: None, confidence: None });
        }
        let n = audio_pcm16k.len();

        // 1. Préprocesseur : waveform brut -> mel-spectrogramme 128 canaux.
        let (features, feat_t, features_len) = {
            let mut sess = self.preprocessor.lock().map_err(|_| "mutex préprocesseur empoisonné".to_string())?;
            let waveforms = Tensor::from_array(([1usize, n], audio_pcm16k.to_vec()))
                .map_err(|e| format!("tensor waveforms : {e}"))?;
            let waveforms_lens = Tensor::from_array(([1usize], vec![n as i64]))
                .map_err(|e| format!("tensor waveforms_lens : {e}"))?;
            let outputs = sess
                .run(ort::inputs!["waveforms" => waveforms, "waveforms_lens" => waveforms_lens])
                .map_err(|e| format!("inférence préprocesseur : {e}"))?;
            let (shape, data) = outputs["features"]
                .try_extract_tensor::<f32>()
                .map_err(|e| format!("extraction features : {e}"))?;
            let t = shape[2] as usize;
            let (_, len_data) = outputs["features_lens"]
                .try_extract_tensor::<i64>()
                .map_err(|e| format!("extraction features_lens : {e}"))?;
            (data.to_vec(), t, len_data[0])
        };

        // 2. Encoder Conformer : mel-spectrogramme -> embeddings temporels.
        let (encoder_out, enc_t, encoder_out_len) = {
            let mut sess = self.encoder.lock().map_err(|_| "mutex encoder empoisonné".to_string())?;
            let audio_signal = Tensor::from_array(([1usize, FEATURES_SIZE, feat_t], features))
                .map_err(|e| format!("tensor audio_signal : {e}"))?;
            let length = Tensor::from_array(([1usize], vec![features_len]))
                .map_err(|e| format!("tensor length : {e}"))?;
            let outputs = sess
                .run(ort::inputs!["audio_signal" => audio_signal, "length" => length])
                .map_err(|e| format!("inférence encoder : {e}"))?;
            let (shape, data) = outputs["outputs"]
                .try_extract_tensor::<f32>()
                .map_err(|e| format!("extraction outputs (encoder) : {e}"))?;
            let t = shape[2] as usize;
            let (_, len_data) = outputs["encoded_lengths"]
                .try_extract_tensor::<i64>()
                .map_err(|e| format!("extraction encoded_lengths : {e}"))?;
            (data.to_vec(), t, len_data[0])
        };
        // encoder_out est en layout channel-first (1, ENCODER_DIM, enc_t)
        // aplati : index(c, t) = c * enc_t + t.
        let encoded_len = (encoder_out_len as usize).min(enc_t);

        // 3. Boucle de décodage TDT (autorégressive, PAS un argmax CTC) —
        // portage direct de `_AsrWithTransducerDecoding._decoding` +
        // `NemoConformerTdt._decode` (onnx_asr/asr.py, onnx_asr/models/nemo.py).
        let mut decoder_joint = self.decoder_joint.lock().map_err(|_| "mutex decoder_joint empoisonné".to_string())?;
        let mut state1 = vec![0f32; LSTM_LAYERS * LSTM_HIDDEN];
        let mut state2 = vec![0f32; LSTM_LAYERS * LSTM_HIDDEN];
        let mut tokens: Vec<i64> = Vec::new();
        let mut t = 0usize;
        let mut emitted_tokens = 0usize;

        while t < encoded_len {
            let mut frame = vec![0f32; ENCODER_DIM];
            for c in 0..ENCODER_DIM {
                frame[c] = encoder_out[c * enc_t + t];
            }
            let prev_token = *tokens.last().unwrap_or(&BLANK_ID);

            let encoder_outputs_t = Tensor::from_array(([1usize, ENCODER_DIM, 1usize], frame))
                .map_err(|e| format!("tensor encoder_outputs : {e}"))?;
            let targets = Tensor::from_array(([1usize, 1usize], vec![prev_token as i32]))
                .map_err(|e| format!("tensor targets : {e}"))?;
            let target_length = Tensor::from_array(([1usize], vec![1i32]))
                .map_err(|e| format!("tensor target_length : {e}"))?;
            let in_state1 = Tensor::from_array(([LSTM_LAYERS, 1usize, LSTM_HIDDEN], state1.clone()))
                .map_err(|e| format!("tensor input_states_1 : {e}"))?;
            let in_state2 = Tensor::from_array(([LSTM_LAYERS, 1usize, LSTM_HIDDEN], state2.clone()))
                .map_err(|e| format!("tensor input_states_2 : {e}"))?;

            let outputs = decoder_joint
                .run(ort::inputs![
                    "encoder_outputs" => encoder_outputs_t,
                    "targets" => targets,
                    "target_length" => target_length,
                    "input_states_1" => in_state1,
                    "input_states_2" => in_state2,
                ])
                .map_err(|e| format!("inférence decoder_joint : {e}"))?;

            let (_, out_data) = outputs["outputs"]
                .try_extract_tensor::<f32>()
                .map_err(|e| format!("extraction outputs (decoder_joint) : {e}"))?;
            let (token_id, _) = argmax(&out_data[..VOCAB_SIZE]);
            let (step, _) = argmax(&out_data[VOCAB_SIZE..]);

            if token_id as i64 != BLANK_ID {
                let (_, new_state1) = outputs["output_states_1"]
                    .try_extract_tensor::<f32>()
                    .map_err(|e| format!("extraction output_states_1 : {e}"))?;
                let (_, new_state2) = outputs["output_states_2"]
                    .try_extract_tensor::<f32>()
                    .map_err(|e| format!("extraction output_states_2 : {e}"))?;
                state1 = new_state1.to_vec();
                state2 = new_state2.to_vec();
                tokens.push(token_id as i64);
                emitted_tokens += 1;
            }

            if step > 0 {
                t += step;
                emitted_tokens = 0;
            } else if token_id as i64 == BLANK_ID || emitted_tokens == MAX_TOKENS_PER_STEP {
                t += 1;
                emitted_tokens = 0;
            }
        }

        let text = self.detokenize(&tokens);
        Ok(TranscriptionResult { text, language: None, confidence: None })
    }
}

#[cfg(test)]
mod tests {
    //! Test d'intégration réel (pas un mock) : charge le vrai modèle
    //! Parakeet-TDT et transcrit de vrais échantillons audio français et
    //! anglais (fixtures FLEURS, `tests/fixtures/asr/`, licence CC-BY-4.0).
    //! `#[ignore]` par défaut car il nécessite le modèle complet (~670 Mo,
    //! non commité, voir doc de module) placé dans le répertoire de
    //! données de l'app — lancer avec `cargo test -- --ignored` une fois
    //! le modèle en place (voir PROGRESS.md Session 5 pour la procédure
    //! utilisée pendant le développement).
    use super::*;
    use std::time::Instant;

    fn model_dir_for_test() -> PathBuf {
        let appdata = std::env::var("APPDATA").expect("APPDATA non défini (test Windows uniquement)");
        PathBuf::from(appdata).join("com.exponentvalue.vozel").join("models").join("parakeet-tdt-v3")
    }

    fn read_wav_mono_f32(path: &Path) -> Vec<f32> {
        let mut reader = hound::WavReader::open(path).expect("lecture du fichier WAV de test");
        let spec = reader.spec();
        assert_eq!(spec.sample_rate, 16_000, "fixture attendue en 16kHz");
        assert_eq!(spec.channels, 1, "fixture attendue en mono");
        match spec.sample_format {
            hound::SampleFormat::Float => reader.samples::<f32>().map(|s| s.unwrap()).collect(),
            hound::SampleFormat::Int => reader
                .samples::<i16>()
                .map(|s| s.unwrap() as f32 / i16::MAX as f32)
                .collect(),
        }
    }

    #[test]
    #[ignore]
    fn transcribes_french_and_english_samples() {
        let engine = LocalAsrEngine::load_from_dir(&model_dir_for_test()).expect("chargement du moteur ASR");

        let cases = [
            ("fr_short.wav", "fr", "l'accident"),
            ("fr_00.wav", "fr", "l'accident a eu lieu"),
            ("fr_01.wav", "fr", "nous sommes d'accord"),
            ("fr_02.wav", "fr", "il ajouté"),
            ("en_00.wav", "en", "due to the slow"),
        ];

        for (file, lang, expected_start) in cases {
            let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/asr").join(file);
            let pcm = read_wav_mono_f32(&path);

            let t0 = Instant::now();
            let result = engine.transcribe(&pcm).expect("transcription");
            let elapsed = t0.elapsed();

            let duration_s = pcm.len() as f64 / 16_000.0;
            println!(
                "[{lang}] {file} : \"{}\" ({:.2}s audio, {:.2}s transcription, RTF={:.3})",
                result.text,
                duration_s,
                elapsed.as_secs_f64(),
                elapsed.as_secs_f64() / duration_s
            );

            let normalized = result.text.to_lowercase();
            assert!(
                normalized.contains(expected_start),
                "'{file}' : transcription inattendue, obtenu '{}', attendu de contenir '{expected_start}'",
                result.text
            );
        }
    }
}
