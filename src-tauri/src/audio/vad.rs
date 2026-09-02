//! Détection d'activité vocale (VAD) — coupe les silences en amont de l'ASR
//! pour réduire la charge de calcul et la latence perçue de démarrage/arrêt.
//!
//! V1 (Spec_Backend_Desktop.md §1.2) : heuristique d'énergie RMS, sans
//! dépendance modèle. Un vrai VAD (ex. Silero ONNX) reste une amélioration
//! ultérieure sans bloquer la Phase 1 — la signature reste stable pour un
//! remplacement transparent.

/// Seuil RMS en-dessous duquel une frame 16 kHz mono est considérée comme du
/// silence. Calibré empiriquement pour un micro laptop en environnement
/// calme ; à exposer en réglage avancé si trop sensible en usage réel.
/// Abaissé de 0.02 → 0.012 (Session 16 suite) : à 0.02 une voix normale sur
/// micro laptop passait parfois pour du silence, ne transmettant rien à l'ASR.
const SILENCE_RMS_THRESHOLD: f32 = 0.012;

/// Valeur du seuil, pour les logs de diagnostic (`audio::capture`).
pub fn silence_threshold() -> f32 {
    SILENCE_RMS_THRESHOLD
}

pub fn is_speech(frame: &[f32]) -> bool {
    if frame.is_empty() {
        return false;
    }
    let sum_sq: f32 = frame.iter().map(|s| s * s).sum();
    let rms = (sum_sq / frame.len() as f32).sqrt();
    rms > SILENCE_RMS_THRESHOLD
}
