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

use asr::local::LocalAsrEngine;
use audio::capture::{CaptureCommand, PcmFrame};
use hotkey::HotkeyManager;
use storage::settings::Settings;
use tauri::Manager;

/// État managé Tauri pour le moteur ASR local — `None` si le modèle n'est
/// pas installé (voir `asr::local::LocalAsrEngine::load`), auquel cas la
/// dictée reste indisponible mais l'app démarre normalement. Consommé par
/// `commands::run_pipeline` (Spec_Backend_Desktop.md §1.6).
pub struct AsrState(pub Option<LocalAsrEngine>);

/// État managé Tauri pour le pipeline de dictée : le `Sender` pour piloter
/// `audio::capture` (partagé entre `hotkey` et les commandes IPC, pour que
/// les deux déclencheurs utilisent le même mécanisme) et le `Receiver` des
/// frames PCM accumulées, vidé par `commands::run_pipeline` à l'arrêt d'une
/// session de dictée. `Mutex` sur le `Receiver` seul (pas sur tout l'état) :
/// un canal `crossbeam_channel::Receiver` n'est pas `Sync`, mais plusieurs
/// déclencheurs (hotkey, futur bouton UI) doivent pouvoir y accéder depuis
/// des threads différents sans jamais le lire concurremment pour de vrai
/// (une seule dictée à la fois).
pub struct PipelineState {
    pub capture_tx: crossbeam_channel::Sender<CaptureCommand>,
    pub pcm_rx: std::sync::Mutex<crossbeam_channel::Receiver<PcmFrame>>,
}

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
            // Réglages persistés (fichier JSON, Spec_Backend_Desktop.md
            // §1.6) — valeurs par défaut au tout premier lancement.
            let settings = Settings::load(app.handle());

            // La capture audio doit exister (paused) avant l'enregistrement
            // du hotkey, qui la pilote via `CaptureCommand::Start/Stop` —
            // voir Spec_Backend_Desktop.md §1.2. `pcm_rx` est vidé par
            // `commands::run_pipeline` (§1.6) à l'arrêt d'une dictée ; tant
            // qu'aucune dictée n'est active la capture est en pause (aucune
            // frame produite), donc le canal borné n'accumule rien à vide.
            match audio::capture::spawn(app.handle().clone()) {
                Ok((capture_tx, pcm_rx)) => {
                    println!("[audio] capture micro initialisée (paused, device par défaut)");
                    app.manage(PipelineState {
                        capture_tx: capture_tx.clone(),
                        pcm_rx: std::sync::Mutex::new(pcm_rx),
                    });
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

            // Moteur ASR local (Parakeet-TDT ONNX INT8, Spec_Backend_Desktop.md
            // §1.3) : chargé une seule fois ici, pas à chaque transcription
            // (même principe que hotkey/audio). Le modèle (~670 Mo) n'est pas
            // commité dans le dépôt — son absence au premier lancement est un
            // cas attendu et recouvrable (voir LocalAsrEngine::load), pas une
            // erreur fatale.
            let asr_state = match LocalAsrEngine::load(app.handle()) {
                Ok(engine) => {
                    println!("[asr] moteur local Parakeet-TDT chargé");
                    AsrState(Some(engine))
                }
                Err(e) => {
                    eprintln!("[asr] moteur local indisponible : {e}");
                    AsrState(None)
                }
            };
            app.manage(asr_state);

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
