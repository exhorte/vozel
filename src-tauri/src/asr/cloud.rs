//! Moteur ASR cloud multi-fournisseurs (Spec_Backend_Desktop.md §2.4), à la
//! manière de Freestyle : l'utilisateur choisit son fournisseur et fournit
//! sa propre clé API (`storage::settings`, jamais loguée en clair).
//!
//! Phase 2 §2.4 : **Groq**, **OpenAI** et **Deepgram** implémentés (§2.4.3,
//! « une fois le premier validé » — Groq validé en usage réel le 2026-09-01).
//! Groq et OpenAI partagent le **même** endpoint compatible OpenAI
//! (multipart + réponse `{"text": …}`) → un seul chemin
//! `transcribe_openai_compatible`. Deepgram a une API distincte (corps audio
//! brut, réponse imbriquée `results.channels[].alternatives[]`) →
//! `transcribe_deepgram`. Le pipeline ne voit jamais ce choix : c'est
//! `RoutingAsrEngine` (`asr::mod`) qui présente l'interface `AsrEngine`
//! commune (critère §2.4 : « aucun changement dans `commands/` »).

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::types::TranscriptionResult;

const SAMPLE_RATE: u32 = 16_000;

// --- Groq (endpoint compatible OpenAI) ---
const GROQ_URL: &str = "https://api.groq.com/openai/v1/audio/transcriptions";
// Modèle Groq le plus rapide (« turbo »), cohérent avec le positionnement
// « cloud = plus rapide » ; ne supporte que `response_format=json`.
const GROQ_MODEL: &str = "whisper-large-v3-turbo";

// --- OpenAI (même forme de requête/réponse que Groq) ---
const OPENAI_URL: &str = "https://api.openai.com/v1/audio/transcriptions";
// `whisper-1` : le modèle de transcription universellement disponible pour
// une clé BYO (les modèles `gpt-4o-*-transcribe` plus récents existent aussi
// et renvoient le même `{"text": …}` en `response_format=json`, mais ne sont
// pas garantis accessibles sur tous les comptes). Détection de langue
// automatique côté Whisper, comme Groq — pas de langue à passer.
const OPENAI_MODEL: &str = "whisper-1";

// --- Deepgram (API propre : POST du corps audio brut) ---
const DEEPGRAM_URL: &str = "https://api.deepgram.com/v1/listen";
// `nova-2` : modèle multilingue courant de Deepgram (bon français).
// `smart_format=true` ajoute ponctuation/majuscules/formatage des nombres.
// `detect_language=true` : pas d'hypothèse de langue codée en dur, cohérent
// avec la détection automatique de Whisper sur Groq/OpenAI.
const DEEPGRAM_MODEL: &str = "nova-2";

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
            Provider::Groq => {
                self.transcribe_openai_compatible(pcm, key, GROQ_URL, GROQ_MODEL, "Groq")
            }
            Provider::OpenAi => {
                self.transcribe_openai_compatible(pcm, key, OPENAI_URL, OPENAI_MODEL, "OpenAI")
            }
            Provider::Deepgram => self.transcribe_deepgram(pcm, key),
            Provider::Unknown(name) => Err(format!("fournisseur cloud inconnu : « {name} »")),
        }
    }

    /// Transcription via un endpoint **compatible OpenAI** (`/audio/transcriptions`) :
    /// requête `multipart/form-data` (`model` + `file` WAV), authentification
    /// `Bearer`, réponse `{"text": …}`, erreurs `{"error":{"message":…}}`.
    /// Groq et OpenAI partagent exactement ce contrat — seuls `url`, `model`
    /// et le libellé `provider` (messages d'erreur) changent.
    fn transcribe_openai_compatible(
        &self,
        pcm: &[f32],
        key: &str,
        url: &str,
        model: &str,
        provider: &str,
    ) -> Result<TranscriptionResult, String> {
        let wav = pcm_f32_to_wav16(pcm, SAMPLE_RATE);
        let boundary = multipart_boundary();
        let body = multipart_body(&boundary, model, &wav);

        let mut response = self
            .agent
            .post(url)
            .header("Authorization", format!("Bearer {key}"))
            .header(
                "Content-Type",
                format!("multipart/form-data; boundary={boundary}"),
            )
            .send(body.as_slice())
            .map_err(|e| format!("requête cloud ({provider}) échouée : {e}"))?;

        let status = response.status();
        let raw = response
            .body_mut()
            .read_to_string()
            .map_err(|e| format!("lecture de la réponse {provider} : {e}"))?;

        if !status.is_success() {
            return Err(format!(
                "{provider} a répondu {} : {}",
                status.as_u16(),
                openai_compat_error_message(&raw)
            ));
        }
        parse_openai_compat_transcription(&raw)
            .map_err(|e| format!("réponse {provider} inattendue ({e})"))
    }

    /// Transcription via l'API Deepgram (`/v1/listen`) : le corps de la
    /// requête est l'audio brut (pas de multipart), authentification `Token`
    /// (pas `Bearer`), options en paramètres d'URL, réponse imbriquée
    /// `results.channels[].alternatives[].transcript`.
    fn transcribe_deepgram(&self, pcm: &[f32], key: &str) -> Result<TranscriptionResult, String> {
        let wav = pcm_f32_to_wav16(pcm, SAMPLE_RATE);
        let url = deepgram_url();

        let mut response = self
            .agent
            .post(&url)
            .header("Authorization", format!("Token {key}"))
            .header("Content-Type", "audio/wav")
            .send(wav.as_slice())
            .map_err(|e| format!("requête cloud (Deepgram) échouée : {e}"))?;

        let status = response.status();
        let raw = response
            .body_mut()
            .read_to_string()
            .map_err(|e| format!("lecture de la réponse Deepgram : {e}"))?;

        if !status.is_success() {
            return Err(format!(
                "Deepgram a répondu {} : {}",
                status.as_u16(),
                deepgram_error_message(&raw)
            ));
        }
        parse_deepgram_transcription(&raw).map_err(|e| format!("réponse Deepgram inattendue ({e})"))
    }
}

/// URL Deepgram complète avec les options en paramètres de requête (voir les
/// constantes `DEEPGRAM_*`). Isolée pour être testable sans réseau.
fn deepgram_url() -> String {
    format!("{DEEPGRAM_URL}?model={DEEPGRAM_MODEL}&smart_format=true&detect_language=true")
}

// --- Réponse des endpoints compatibles OpenAI (Groq, OpenAI) ---

#[derive(serde::Deserialize)]
struct OpenAiCompatTranscription {
    text: String,
}

#[derive(serde::Deserialize)]
struct OpenAiCompatErrorEnvelope {
    error: OpenAiCompatErrorBody,
}

#[derive(serde::Deserialize)]
struct OpenAiCompatErrorBody {
    message: String,
}

/// Parse une réponse de transcription compatible OpenAI (`{"text": …}`).
fn parse_openai_compat_transcription(raw: &str) -> Result<TranscriptionResult, String> {
    let parsed: OpenAiCompatTranscription =
        serde_json::from_str(raw).map_err(|e| e.to_string())?;
    Ok(TranscriptionResult {
        text: parsed.text.trim().to_string(),
        language: None,
        confidence: None,
    })
}

/// Extrait le message d'erreur d'une réponse compatible OpenAI
/// (`{"error":{"message":…}}`), ou renvoie le corps brut tronqué. Ne contient
/// jamais la clé API (elle n'est que dans l'en-tête de la requête).
fn openai_compat_error_message(raw: &str) -> String {
    match serde_json::from_str::<OpenAiCompatErrorEnvelope>(raw) {
        Ok(env) => env.error.message,
        Err(_) => truncate_for_log(raw),
    }
}

// --- Réponse Deepgram ---

#[derive(serde::Deserialize)]
struct DeepgramResponse {
    results: DeepgramResults,
}

#[derive(serde::Deserialize)]
struct DeepgramResults {
    channels: Vec<DeepgramChannel>,
}

#[derive(serde::Deserialize)]
struct DeepgramChannel {
    alternatives: Vec<DeepgramAlternative>,
    #[serde(default)]
    detected_language: Option<String>,
}

#[derive(serde::Deserialize)]
struct DeepgramAlternative {
    transcript: String,
    #[serde(default)]
    confidence: Option<f32>,
}

#[derive(serde::Deserialize)]
struct DeepgramErrorBody {
    #[serde(default)]
    err_msg: Option<String>,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    error: Option<String>,
}

/// Parse une réponse Deepgram : `results.channels[0].alternatives[0]`.
fn parse_deepgram_transcription(raw: &str) -> Result<TranscriptionResult, String> {
    let parsed: DeepgramResponse = serde_json::from_str(raw).map_err(|e| e.to_string())?;
    let channel = parsed
        .results
        .channels
        .into_iter()
        .next()
        .ok_or("aucun canal dans la réponse")?;
    let detected_language = channel.detected_language;
    let alt = channel
        .alternatives
        .into_iter()
        .next()
        .ok_or("aucune alternative de transcription")?;
    Ok(TranscriptionResult {
        text: alt.transcript.trim().to_string(),
        language: detected_language,
        confidence: alt.confidence,
    })
}

/// Extrait le message d'erreur d'une réponse Deepgram (`err_msg`, sinon
/// `message`/`error`), ou renvoie le corps brut tronqué.
fn deepgram_error_message(raw: &str) -> String {
    if let Ok(body) = serde_json::from_str::<DeepgramErrorBody>(raw) {
        if let Some(m) = body.err_msg.or(body.message).or(body.error) {
            return m;
        }
    }
    truncate_for_log(raw)
}

/// Tronque un corps de réponse pour un message d'erreur, sur une frontière de
/// caractère (jamais au milieu d'un octet UTF-8).
fn truncate_for_log(raw: &str) -> String {
    let trimmed = raw.trim();
    match trimmed.char_indices().nth(300) {
        Some((idx, _)) => format!("{}…", &trimmed[..idx]),
        None => trimmed.to_string(),
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
    fn unknown_provider_reports_clearly() {
        let engine = CloudAsrEngine::new();
        let err = engine.transcribe(&[0.1], "mistral", "key-x").unwrap_err();
        assert!(err.contains("inconnu") && err.contains("mistral"), "message inattendu : {err}");
    }

    // OpenAI et Deepgram sont maintenant implémentés (§2.4.3). Le test qui
    // affirmait « pas encore implémenté » a été retiré : on ne peut pas faire
    // d'appel réseau réel ici (clé + réseau = test utilisateur, comme Groq),
    // donc on teste les briques — construction de requête, parsing d'une
    // réponse représentative, chemins d'erreur.

    #[test]
    fn openai_compatible_response_parses() {
        // Forme réelle renvoyée par Groq et par OpenAI (`response_format=json`).
        let r = parse_openai_compat_transcription(r#"{"text": "  Bonjour le monde.  "}"#).unwrap();
        assert_eq!(r.text, "Bonjour le monde.");
        assert!(r.language.is_none() && r.confidence.is_none());
        // JSON qui n'a pas le champ attendu -> Err (remonté comme « réponse
        // <provider> inattendue » par l'appelant).
        assert!(parse_openai_compat_transcription(r#"{"nope": 1}"#).is_err());
    }

    #[test]
    fn openai_compat_error_message_extracts_or_falls_back() {
        assert_eq!(
            openai_compat_error_message(
                r#"{"error":{"message":"Invalid API Key","type":"invalid_request_error"}}"#
            ),
            "Invalid API Key"
        );
        assert_eq!(openai_compat_error_message("  Bad Gateway  "), "Bad Gateway");
        // tronqué proprement, jamais au milieu d'un caractère
        let long = "é".repeat(500);
        let msg = openai_compat_error_message(&long);
        assert!(msg.ends_with('…') && msg.chars().count() == 301);
    }

    #[test]
    fn deepgram_url_has_expected_params() {
        let u = deepgram_url();
        assert!(u.starts_with("https://api.deepgram.com/v1/listen?"));
        assert!(u.contains("model=nova-2"));
        assert!(u.contains("smart_format=true"));
        assert!(u.contains("detect_language=true"));
    }

    #[test]
    fn deepgram_response_parses() {
        // Forme réelle de `POST /v1/listen` (pré-enregistré) : la transcription
        // est dans results.channels[0].alternatives[0].
        let raw = r#"{
            "metadata": {"request_id": "abc"},
            "results": {
                "channels": [
                    {
                        "detected_language": "fr",
                        "alternatives": [
                            {"transcript": "  on se voit demain  ", "confidence": 0.987, "words": []}
                        ]
                    }
                ]
            }
        }"#;
        let r = parse_deepgram_transcription(raw).unwrap();
        assert_eq!(r.text, "on se voit demain");
        assert_eq!(r.language.as_deref(), Some("fr"));
        assert!((r.confidence.unwrap() - 0.987).abs() < 1e-4);
    }

    #[test]
    fn deepgram_response_without_transcription_is_err() {
        assert!(parse_deepgram_transcription(r#"{"results":{"channels":[]}}"#).is_err());
        assert!(parse_deepgram_transcription(
            r#"{"results":{"channels":[{"alternatives":[]}]}}"#
        )
        .is_err());
        assert!(parse_deepgram_transcription("not json").is_err());
    }

    #[test]
    fn deepgram_error_message_extracts_or_falls_back() {
        assert_eq!(
            deepgram_error_message(
                r#"{"err_code":"INVALID_AUTH","err_msg":"Invalid credentials.","request_id":"x"}"#
            ),
            "Invalid credentials."
        );
        // certains 4xx Deepgram renvoient {"message": …}
        assert_eq!(
            deepgram_error_message(r#"{"message":"project balance is too low"}"#),
            "project balance is too low"
        );
        assert_eq!(deepgram_error_message("  Service Unavailable  "), "Service Unavailable");
    }

    #[test]
    fn multipart_body_is_shared_by_groq_and_openai() {
        // Groq et OpenAI passent tous deux par `multipart_body` avec leur
        // modèle respectif — vérifie que le corps porte bien le bon modèle.
        let g = String::from_utf8_lossy(&multipart_body("B", GROQ_MODEL, b"W")).into_owned();
        let o = String::from_utf8_lossy(&multipart_body("B", OPENAI_MODEL, b"W")).into_owned();
        assert!(g.contains("whisper-large-v3-turbo"));
        assert!(o.contains("whisper-1"));
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

}
