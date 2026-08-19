//! Commandes IPC exposées au frontend (invoke depuis React/TS).
//! Point de couture entre l'UI et les modules `audio`/`asr`/`postprocess`/
//! `injection`/`storage`. À étoffer au fil des phases de la Roadmap.

use crate::storage::settings::Settings;

/// Démarre une session de dictée (déclenchée par le hotkey ou l'UI).
#[tauri::command]
pub fn start_dictation() -> Result<(), String> {
    // TODO: audio::capture::start() -> asr -> postprocess -> injection
    Err("not implemented".into())
}

/// Arrête la session de dictée en cours.
#[tauri::command]
pub fn stop_dictation() -> Result<(), String> {
    Err("not implemented".into())
}

/// Retourne les réglages courants pour la fenêtre de réglages.
#[tauri::command]
pub fn get_settings() -> Settings {
    Settings::default()
}

/// Sauvegarde les réglages modifiés par l'utilisateur.
#[tauri::command]
pub fn save_settings(_settings: Settings) -> Result<(), String> {
    // TODO: storage::settings persist.
    Err("not implemented".into())
}
