//! Commandes IPC exposées au frontend (invoke depuis React/TS).
//! Point de couture entre l'UI et les modules `audio`/`asr`/`postprocess`/
//! `injection`/`storage`. À étoffer au fil des phases de la Roadmap.

use crate::storage::settings::Settings;
use tauri::AppHandle;

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

/// Retourne les réglages courants pour la fenêtre de réglages (lus depuis le
/// fichier de config JSON, valeurs par défaut si absent — voir
/// `storage::settings::Settings::load`).
#[tauri::command]
pub fn get_settings(app: AppHandle) -> Settings {
    Settings::load(&app)
}

/// Sauvegarde les réglages modifiés par l'utilisateur dans le fichier de
/// config JSON (Spec_Backend_Desktop.md §1.6 — le SQLite complet est Phase 2).
/// Note : ne réapplique pas à chaud un raccourci clavier modifié (le hotkey
/// global est enregistré une seule fois au démarrage, voir `hotkey::mod` —
/// un redémarrage de l'app est nécessaire pour l'instant, à lever quand
/// `HotkeyManager` gagnera une méthode de ré-enregistrement).
#[tauri::command]
pub fn save_settings(app: AppHandle, settings: Settings) -> Result<(), String> {
    settings.save(&app)
}
