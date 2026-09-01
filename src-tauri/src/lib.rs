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

use asr::cloud::CloudAsrEngine;
use asr::local::LocalAsrEngine;
use asr::{AsrEngine, RoutingAsrEngine};
use audio::capture::{CaptureCommand, PcmFrame};
use hotkey::HotkeyManager;
use storage::settings::Settings;
use tauri::Manager;

/// État managé Tauri pour l'ASR : un `RoutingAsrEngine` (local ↔ cloud selon
/// les réglages, Spec_Backend_Desktop.md §2.4) derrière l'interface
/// `AsrEngine`. `Option` conservé pour la forme historique attendue par
/// `commands::run_pipeline` — en pratique toujours `Some` (l'aiguilleur
/// gère lui-même l'indisponibilité de l'un ou l'autre moteur).
pub struct AsrState(pub Option<Box<dyn AsrEngine + Send + Sync>>);

/// État managé Tauri pour le pipeline de dictée : le `Sender` pour piloter
/// `audio::capture` (partagé entre `hotkey` et les commandes IPC, pour que
/// les deux déclencheurs utilisent le même mécanisme) et le `Receiver` des
/// frames PCM accumulées, vidé par `commands::run_pipeline` à l'arrêt d'une
/// session de dictée. `Mutex` sur le `Receiver` seul (pas sur tout l'état) :
/// un canal `crossbeam_channel::Receiver` n'est pas `Sync`, mais plusieurs
/// déclencheurs (hotkey, futur bouton UI) doivent pouvoir y accéder depuis
/// des threads différents sans jamais le lire concurremment pour de vrai
/// (une seule dictée à la fois).
///
/// `listening` : drapeau partagé entre le thread du hotkey et les commandes
/// IPC (`start_dictation`/`stop_dictation`). Une seule source de vérité pour
/// « une dictée est-elle en cours ? », quel que soit le déclencheur — sans
/// lui, le mode `Toggle` du hotkey garde son propre compteur interne et se
/// désynchronise dès qu'une dictée a été démarrée/arrêtée depuis l'UI (le
/// hotkey demanderait alors un second appui pour se recaler).
pub struct PipelineState {
    pub capture_tx: crossbeam_channel::Sender<CaptureCommand>,
    pub pcm_rx: std::sync::Mutex<crossbeam_channel::Receiver<PcmFrame>>,
    pub listening: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

/// État managé Tauri pour le nettoyage du texte transcrit (§2.3) : soit les
/// règles (`postprocess::RuleCleaner`), soit le LLM local
/// (`postprocess::LlmCleaner`) selon `Settings::llm_cleanup_enabled` et la
/// présence du modèle. `commands::run_pipeline` appelle `.clean()` sans
/// savoir lequel tourne.
pub struct CleanerState(pub Box<dyn postprocess::TextCleaner>);

/// Sélection capturée au dernier déclenchement du raccourci Command Mode, en
/// attente qu'une reformulation soit choisie dans la palette (fenêtre
/// `command`). `target_hwnd` : la fenêtre au premier plan avant l'ouverture
/// de la palette — à re-cibler avant de coller le résultat par-dessus la
/// sélection.
#[derive(Clone)]
pub struct PendingCommand {
    pub selected_text: String,
    pub target_hwnd: isize,
}

/// État managé Tauri pour le Command Mode (Spec_Frontend.md §2.2). `engine` :
/// le même `LlmEngine` que `CleanerState` (partagé via `Arc` — le modèle
/// ~1,9 Go n'est chargé qu'une fois même si les deux features sont actives),
/// ou `None` si `command_mode_enabled` est faux / le modèle est absent.
pub struct CommandModeState {
    pub engine: Option<std::sync::Arc<postprocess::llm::LlmEngine>>,
    pub pending: std::sync::Mutex<Option<PendingCommand>>,
}

impl CommandModeState {
    /// Enregistre la sélection capturée par le raccourci, en attente d'une
    /// reformulation. Un `Mutex` empoisonné est ignoré (silencieusement : la
    /// palette signalera « aucune sélection »).
    pub fn store_pending(&self, cmd: PendingCommand) {
        if let Ok(mut p) = self.pending.lock() {
            *p = Some(cmd);
        }
    }

    /// Copie la sélection en attente sans la consommer.
    pub fn peek_pending(&self) -> Option<PendingCommand> {
        self.pending.lock().ok().and_then(|p| p.clone())
    }

    /// Consomme la sélection en attente (après application ou annulation).
    pub fn take_pending(&self) -> Option<PendingCommand> {
        self.pending.lock().ok().and_then(|mut p| p.take())
    }
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
            commands::llm_model_available,
            commands::dict_list,
            commands::dict_create,
            commands::dict_update,
            commands::dict_delete,
            commands::command_mode_context,
            commands::command_mode_reformulations,
            commands::run_command_mode,
            commands::close_command_palette,
        ])
        .setup(|app| {
            // Base SQLite (Spec_Backend_Desktop.md §2.1) — cœur de la
            // persistance depuis la Phase 2. Contrairement au micro / au
            // modèle ASR / à l'ancien fichier de config (absents = cas
            // recouvrables gérés sans crash), une base illisible dans le
            // dossier de données de l'app signale un environnement cassé
            // (disque plein, permissions, corruption) : on échoue le
            // démarrage avec un message clair plutôt que de tourner à moitié
            // sans réglages ni dictionnaire.
            let db_pool = tauri::async_runtime::block_on(storage::db::init(app.handle()))
                .map_err(|e| format!("initialisation de la base SQLite : {e}"))?;
            // Migration ponctuelle des réglages Phase 1 (settings.json) vers
            // SQLite, puis chargement de la ligne unique.
            tauri::async_runtime::block_on(Settings::ensure_migrated(&db_pool, app.handle()))
                .map_err(|e| format!("migration des réglages vers SQLite : {e}"))?;
            let settings = tauri::async_runtime::block_on(Settings::load_db(&db_pool))
                .unwrap_or_else(|e| {
                    eprintln!("[storage::settings] {e}, valeurs par défaut");
                    Settings::default()
                });
            app.manage(storage::db::Db(db_pool.clone()));
            println!("[storage] base SQLite prête (migrations appliquées)");

            // La capture audio doit exister (paused) avant l'enregistrement
            // du hotkey, qui la pilote via `CaptureCommand::Start/Stop` —
            // voir Spec_Backend_Desktop.md §1.2. `pcm_rx` est vidé par
            // `commands::run_pipeline` (§1.6) à l'arrêt d'une dictée ; tant
            // qu'aucune dictée n'est active la capture est en pause (aucune
            // frame produite), donc le canal borné n'accumule rien à vide.
            match audio::capture::spawn(app.handle().clone()) {
                Ok((capture_tx, pcm_rx)) => {
                    println!("[audio] capture micro initialisée (paused, device par défaut)");
                    let listening = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                    app.manage(PipelineState {
                        capture_tx: capture_tx.clone(),
                        pcm_rx: std::sync::Mutex::new(pcm_rx),
                        listening: listening.clone(),
                    });
                    // Raccourci Command Mode (Spec_Frontend.md §2.2) : passé au
                    // même `HotkeyManager` (enregistré sur le même manager,
                    // démultiplexé par id) seulement si la feature est activée.
                    let command_mode_hotkey = settings
                        .command_mode_enabled
                        .then(|| settings.command_mode_hotkey.as_str());
                    match HotkeyManager::register(
                        app.handle().clone(),
                        &settings.hotkey,
                        settings.hotkey_mode,
                        capture_tx.clone(),
                        listening.clone(),
                        command_mode_hotkey,
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

                    // Push-to-talk Ctrl+Win seul (Spec_Backend_Desktop.md §2.5) :
                    // mécanisme *additionnel* au hotkey classique ci-dessus, opt-in
                    // (`ctrl_win_ptt_enabled`, défaut faux). Installe un hook clavier
                    // bas niveau `WH_KEYBOARD_LL` — coût nul si non activé, donc pas
                    // installé du tout sinon. Doit être appelé ici (thread de la
                    // boucle de messages Win32), même contrainte que `HotkeyManager`.
                    // Un changement du réglage prend effet au redémarrage.
                    #[cfg(target_os = "windows")]
                    if settings.ctrl_win_ptt_enabled {
                        match hotkey::modifier_combo::register(
                            app.handle().clone(),
                            capture_tx,
                            listening,
                        ) {
                            Ok(()) => {}
                            Err(e) => eprintln!("[hotkey] push-to-talk Ctrl+Win indisponible : {e}"),
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
            let local_asr = match LocalAsrEngine::load(app.handle()) {
                Ok(engine) => {
                    println!("[asr] moteur local Parakeet-TDT chargé");
                    Some(engine)
                }
                Err(e) => {
                    eprintln!("[asr] moteur local indisponible : {e}");
                    None
                }
            };
            // Aiguilleur local ↔ cloud (§2.4) : présente une seule interface
            // `AsrEngine` au pipeline, choisit à chaque dictée selon les
            // réglages. Le client cloud ne fait aucun appel réseau tant que
            // `cloud_enabled` est faux.
            let router =
                RoutingAsrEngine::new(local_asr, CloudAsrEngine::new(), db_pool.clone());
            app.manage(AsrState(Some(Box::new(router))));

            // LLM local (§2.3 nettoyage + §2.2 Command Mode) : le modèle
            // (~1,9 Go) est chargé **une seule fois** ici et partagé via `Arc`
            // entre `CleanerState` et `CommandModeState` quand les deux
            // features sont actives. Chargé seulement si au moins l'une des
            // deux est activée dans les réglages ; toute défaillance (absence,
            // échec) laisse `None` — le nettoyage retombe sur les règles, le
            // Command Mode devient indisponible. Un changement d'un de ces
            // réglages prend effet au redémarrage (pas de recharge à chaud).
            let llm_engine: Option<std::sync::Arc<postprocess::llm::LlmEngine>> =
                if settings.llm_cleanup_enabled || settings.command_mode_enabled {
                    match postprocess::llm::LlmEngine::load(app.handle()) {
                        Ok(engine) => Some(std::sync::Arc::new(engine)),
                        Err(e) => {
                            eprintln!("[postprocess] LLM demandé mais indisponible ({e}) — nettoyage sur règles, Command Mode désactivé");
                            None
                        }
                    }
                } else {
                    None
                };

            // Nettoyage du texte (§2.3) : `run_pipeline` ne voit qu'un
            // `dyn TextCleaner`. Par défaut = règles (`RuleCleaner`), sauf si
            // `llm_cleanup_enabled` ET le modèle chargé ci-dessus.
            let cleaner: Box<dyn postprocess::TextCleaner> =
                match (settings.llm_cleanup_enabled, &llm_engine) {
                    (true, Some(engine)) => {
                        println!("[postprocess] nettoyage LLM local activé");
                        Box::new(postprocess::LlmCleaner::new(
                            engine.clone(),
                            postprocess::llm::GenParams::default(),
                        ))
                    }
                    _ => Box::new(postprocess::RuleCleaner),
                };
            app.manage(CleanerState(cleaner));

            // Command Mode (§2.2) : réutilise le même moteur si activé et
            // chargé. `pending` accueille la sélection capturée par le
            // raccourci dédié, le temps qu'une reformulation soit choisie.
            let command_engine = match (settings.command_mode_enabled, &llm_engine) {
                (true, Some(engine)) => {
                    println!("[postprocess] Command Mode activé");
                    Some(engine.clone())
                }
                (true, None) => {
                    eprintln!("[postprocess] Command Mode demandé mais modèle LLM indisponible — la palette signalera l'absence du modèle");
                    None
                }
                _ => None,
            };
            app.manage(CommandModeState {
                engine: command_engine,
                pending: std::sync::Mutex::new(None),
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
