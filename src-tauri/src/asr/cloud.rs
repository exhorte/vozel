//! Moteur ASR cloud multi-fournisseurs (Spec_Backend_Desktop.md §2.4), à la
//! manière de Freestyle : l'utilisateur choisit son fournisseur et fournit
//! sa propre clé API (`storage::settings`, jamais loguée en clair).
//!
//! Phase 2 §2.4 : **Groq** implémenté (endpoint transcription compatible
//! OpenAI, le plus rapide d'après l'analyse). OpenAI et Deepgram sont des
//! branches `match` explicites à compléter « une fois le premier validé »
//! (§2.4.3). Le pipeline ne voit jamais ce choix : c'est `RoutingAsrEngine`
//! (`asr::mod`) qui présente l'interface `AsrEngine` commune (critère §2.4 :
//! « aucun changement dans `commands/` »).

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::types::TranscriptionResult;

const SAMPLE_RATE: u32 = 16_000;
const GROQ_URL: &str = "https://api.groq.com/openai/v1/audio/transcriptions";
// Modèle Groq le plus rapide (« turbo »), cohérent avec le positionnement
// « cloud = plus rapide » ; ne supporte que `response_format=json`.
const GROQ_MODEL: &str = "whisper-large-v3-turbo";

#[derive(Debug, PartialEq, Eq)]
pub enum Provider {
    Groq,
    OpenAi,
    Deepgram,
    Unknown(String),
}

impl Provider {
    pub fn from_setting(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "groq" => Self::Groq,
            "openai" | "open-ai" => Self::OpenAi,
            "deepgram" => Self::Deepgram,
            other => Self::Unknown(other.to_string()),
        }
    }
}

pub struct CloudAsrEngine {
    agent: ureq::Agent,
}

impl Default for CloudAsrEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl CloudAsrEngine {
    pub fn new() -> Self {
        // `http_status_as_error(false)` : on veut lire le corps d'erreur du
        // fournisseur (message explicite) plutôt qu'une `Err(StatusCode)`
        // opaque. Timeout global généreux — une transcription cloud d'une
        // phrase de dictée est courte, mais le réseau peut traîner.
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(60)))
            .build()
            .into();
        Self { agent }
    }

    /// Transcrit `pcm` (mono 16 kHz, `f32` dans [-1, 1]) via le fournisseur
    /// `provider` avec la clé `api_key`. `provider`/`api_key` viennent des
    /// réglages (relus à chaque dictée par `RoutingAsrEngine`).
    pub fn transcribe(
        &self,
        pcm: &[f32],
        provider: &str,
        api_key: &str,
    ) -> Result<TranscriptionResult, String> {
        let key = api_key.trim();
        if key.is_empty() {
            return Err("clé API cloud manquante (à renseigner dans les réglages)".into());
        }
        if pcm.is_empty() {
            return Err("audio vide, rien à transcrire".into());
        }
        match Provider::from_setting(provider) {
            Provider::Groq => self.transcribe_groq(pcm, key),
            Provider::OpenAi => {
                Err("fournisseur « OpenAI » pas encore implémenté (Spec_Backend_Desktop.md §2.4.3)".into())
            }
            Provider::Deepgram => {
                Err("fournisseur « Deepgram » pas encore implémenté (Spec_Backend_Desktop.md §2.4.3)".into())
            }
            Provider::Unknown(name) => Err(format!("fournisseur cloud inconnu : « {name} »")),
        }
    }

    fn transcribe_groq(&self, pcm: &[f32], key: &str) -> Result<TranscriptionResult, String> {
        let wav = pcm_f32_to_wav16(pcm, SAMPLE_RATE);
        let boundary = multipart_boundary();
        let body = multipart_body(&boundary, GROQ_MODEL, &wav);

        let mut response = self
            .agent
            .post(GROQ_URL)
            .header("Authorization", format!("Bearer {key}"))
            .header(
                "Content-Type",
                format!("multipart/form-data; boundary={boundary}"),
            )
            .send(body.as_slice())
            .map_err(|e| format!("requête cloud (Groq) échouée : {e}"))?;

        let status = response.status();
        let raw = response
            .body_mut()
            .read_to_string()
            .map_err(|e| format!("lecture de la réponse Groq : {e}"))?;

        if !status.is_success() {
            return Err(format!(
                "Groq a répondu {} : {}",
                status.as_u16(),
                groq_error_message(&raw)
            ));
        }

        let parsed: GroqTranscription = serde_json::from_str(&raw)
            .map_err(|e| format!("réponse Groq inattendue ({e})"))?;
        Ok(TranscriptionResult {
            text: parsed.text.trim().to_string(),
            language: None,
            confidence: None,
        })
    }
}

#[derive(serde::Deserialize)]
struct GroqTranscription {
    text: String,
}

#[derive(serde::Deserialize)]
struct GroqErrorEnvelope {
    error: GroqErrorBody,
}

#[derive(serde::Deserialize)]
struct GroqErrorBody {
    message: String,
}

/// Extrait le message d'erreur d'une réponse Groq (`{"error":{"message":…}}`),
/// ou renvoie le corps brut tronqué. Ne contient jamais la clé API (Groq ne
/// la renvoie pas ; elle n'est que dans l'en-tête de la requête).
fn groq_error_message(raw: &str) -> String {
    match serde_json::from_str::<GroqErrorEnvelope>(raw) {
        Ok(env) => env.error.message,
        Err(_) => {
            let trimmed = raw.trim();
            if trimmed.len() > 300 {
                format!("{}…", &trimmed[..300])
            } else {
                trimmed.to_string()
            }
        }
    }
}

fn multipart_boundary() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("----vozel{nanos:x}")
}

/// Corps `multipart/form-data` : un champ `model` + le fichier `file`
/// (WAV). Construit à la main — deux parties, pas besoin d'une crate.
fn multipart_body(boundary: &str, model: &str, wav: &[u8]) -> Vec<u8> {
    let mut body = Vec::with_capacity(wav.len() + 256);
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(b"Content-Disposition: form-data; name=\"model\"\r\n\r\n");
    body.extend_from_slice(model.as_bytes());
    body.extend_from_slice(b"\r\n");
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(
        b"Content-Disposition: form-data; name=\"file\"; filename=\"audio.wav\"\r\n",
    );
    body.extend_from_slice(b"Content-Type: audio/wav\r\n\r\n");
    body.extend_from_slice(wav);
    body.extend_from_slice(b"\r\n");
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    body
}

/// Encode un flux `f32` mono en WAV PCM 16 bits little-endian (en-tête de
/// 44 octets + échantillons). Pas de dépendance : le format est trivial et
/// `hound` n'est qu'une `dev-dependency`.
fn pcm_f32_to_wav16(pcm: &[f32], sample_rate: u32) -> Vec<u8> {
    let data_len = (pcm.len() * 2) as u32;
    let mut w = Vec::with_capacity(44 + pcm.len() * 2);
    w.extend_from_slice(b"RIFF");
    w.extend_from_slice(&(36 + data_len).to_le_bytes());
    w.extend_from_slice(b"WAVE");
    w.extend_from_slice(b"fmt ");
    w.extend_from_slice(&16u32.to_le_bytes()); // taille du sous-chunk fmt
    w.extend_from_slice(&1u16.to_le_bytes()); // format PCM
    w.extend_from_slice(&1u16.to_le_bytes()); // 1 canal (mono)
    w.extend_from_slice(&sample_rate.to_le_bytes());
    w.extend_from_slice(&(sample_rate * 2).to_le_bytes()); // octets/s = sr * canaux * (bits/8)
    w.extend_from_slice(&2u16.to_le_bytes()); // alignement de bloc
    w.extend_from_slice(&16u16.to_le_bytes()); // bits par échantillon
    w.extend_from_slice(b"data");
    w.extend_from_slice(&data_len.to_le_bytes());
    for &s in pcm {
        let v = (s.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16;
        w.extend_from_slice(&v.to_le_bytes());
    }
    w
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_parsing() {
        assert_eq!(Provider::from_setting(" Groq "), Provider::Groq);
        assert_eq!(Provider::from_setting("OpenAI"), Provider::OpenAi);
        assert_eq!(Provider::from_setting("deepgram"), Provider::Deepgram);
        assert_eq!(
            Provider::from_setting("mistral"),
            Provider::Unknown("mistral".to_string())
        );
    }

    #[test]
    fn missing_api_key_fails_before_any_network_call() {
        let engine = CloudAsrEngine::new();
        let err = engine.transcribe(&[0.1, 0.2], "groq", "   ").unwrap_err();
        assert!(err.contains("clé API"), "message inattendu : {err}");
    }

    #[test]
    fn unimplemented_providers_report_clearly() {
        let engine = CloudAsrEngine::new();
        let err = engine.transcribe(&[0.1], "openai", "sk-x").unwrap_err();
        assert!(err.contains("OpenAI") && err.contains("pas encore"));
    }

    #[test]
    fn wav_header_is_well_formed() {
        let wav = pcm_f32_to_wav16(&[0.0, 1.0, -1.0, 0.5], 16_000);
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(&wav[36..40], b"data");
        // 44 octets d'en-tête + 4 échantillons * 2 octets
        assert_eq!(wav.len(), 44 + 8);
        // data_len annoncé
        assert_eq!(u32::from_le_bytes(wav[40..44].try_into().unwrap()), 8);
        // saturation : 1.0 -> i16::MAX, -1.0 -> -i16::MAX
        let s1 = i16::from_le_bytes(wav[46..48].try_into().unwrap());
        assert_eq!(s1, i16::MAX);
    }

    #[test]
    fn multipart_body_has_both_parts() {
        let body = multipart_body("BND", "some-model", b"WAVDATA");
        let s = String::from_utf8_lossy(&body);
        assert!(s.starts_with("--BND\r\n"));
        assert!(s.contains("name=\"model\""));
        assert!(s.contains("some-model"));
        assert!(s.contains("name=\"file\"; filename=\"audio.wav\""));
        assert!(s.contains("Content-Type: audio/wav"));
        assert!(s.contains("WAVDATA"));
        assert!(s.ends_with("--BND--\r\n"));
    }

    #[test]
    fn groq_error_message_extracts_or_falls_back() {
        assert_eq!(
            groq_error_message(r#"{"error":{"message":"Invalid API Key","type":"invalid_request_error"}}"#),
            "Invalid API Key"
        );
        assert_eq!(groq_error_message("  Bad Gateway  "), "Bad Gateway");
    }
}
