//! Capture du flux micro (backend `cpal` prévu) et bufferisation en anneau.
//!
//! TODO (Phase 1) : ouvrir le device par défaut, gérer le changement de
//! device à chaud, exposer un flux de frames PCM 16kHz mono vers `asr`.

#[allow(dead_code)]
pub struct AudioCapture;

impl AudioCapture {
    pub fn start() -> Result<Self, String> {
        // TODO: initialiser cpal, démarrer le stream d'entrée.
        Err("not implemented".into())
    }

    pub fn stop(&self) {
        // TODO: arrêter proprement le stream.
    }
}
