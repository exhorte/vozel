//! Détection d'activité vocale (VAD) — coupe les silences en amont de l'ASR
//! pour réduire la charge de calcul et la latence perçue de démarrage/arrêt.
//!
//! TODO (Phase 1) : intégrer Silero VAD (ONNX) ou équivalent léger.

#[allow(dead_code)]
pub fn is_speech(_frame: &[f32]) -> bool {
    // TODO: inférence VAD sur la frame audio.
    true
}
