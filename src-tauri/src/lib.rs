//! Vozel — point d'entrée de la logique applicative Rust (backend Tauri).
//!
//! Modules (voir `../../ARCHITECTURE.md` et
//! `01_Recherche/Analyse_WisprFlow_BridgeVoice_vs_Vozel.md` section 4) :
//! - `audio`       : capture micro + VAD
//! - `asr`         : reconnaissance vocale locale/cloud
//! - `postprocess` : nettoyage IA + Command Mode
//! - `injection`   : injection de texte par OS + repli presse-papiers
//! - `hotkey`      : raccourci global d'activation
//! - `storage`     : dictionnaire personnalisé + réglages (SQLite)
//! - `cloud`       : client vers le backend léger optionnel (auth/sync/proxy)
//! - `commands`    : commandes IPC exposées au frontend

mod audio;
mod asr;
mod postprocess;
mod injection;
mod hotkey;
mod storage;
mod cloud;
mod commands;

use hotkey::HotkeyManager;
use storage::settings::Settings;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            commands::start_dictation,
            commands::stop_dictation,
            commands::get_settings,
            commands::save_settings,
        ])
        .setup(|app| {
            // TODO(Spec_Backend_Desktop.md §1.6) : charger les réglages
            // persistés au lieu de `Settings::default()` une fois
            // `storage::settings` branché sur un fichier de config.
            let settings = Settings::default();

            // La capture audio doit exister (paused) avant l'enregistrement
            // du hotkey, qui la pilote via `CaptureCommand::Start/Stop` —
            // voir Spec_Backend_Desktop.md §1.2. `_pcm_rx` sera consommé par
            // `asr::local` en §1.3 (non branché pour l'instant : le canal
            // borné absorbe silencieusement les frames tant que rien ne les
            // lit, sans fuite mémoire).
            match audio::capture::spawn(app.handle().clone()) {
                Ok((capture_tx, _pcm_rx)) => {
                    println!("[audio] capture micro initialisée (paused, device par défaut)");
                    match HotkeyManager::register(
                        app.handle().clone(),
                        &settings.hotkey,
                        settings.hotkey_mode,
                        capture_tx,
                    ) {
                        Ok(()) => {
                            println!("[hotkey] raccourci '{}' enregistré ({:?})", settings.hotkey, settings.hotkey_mode);
                        }
                        Err(e) => {
                            // Ne doit jamais faire planter l'app (Spec_Backend_Desktop.md
                            // §1.1 critère d'acceptation : "sans crash") — un raccourci
                            // mal configuré ou déjà pris par une autre app reste un cas
                            // recouvrable, à surfacer dans les réglages en Phase 1 §1.2.
                            eprintln!("[hotkey] échec d'enregistrement du raccourci '{}': {e}", settings.hotkey);
                        }
                    }
                }
                Err(e) => {
                    // Même logique : pas de micro dispo (ou permission OS
                    // refusée) ne doit pas empêcher l'app de démarrer.
                    eprintln!("[audio] échec d'initialisation de la capture micro : {e}");
                }
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
