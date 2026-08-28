//! Commandes IPC exposées au frontend (invoke depuis React/TS).
//! Point de couture entre l'UI et les modules `audio`/`asr`/`postprocess`/
//! `injection`/`storage`. À étoffer au fil des phases de la Roadmap.
//!
//! Pipeline complet (Spec_Backend_Desktop.md §1.6) : `hotkey` (ou ces
//! commandes, pour un futur bouton UI) pilotent `audio::capture` via
//! `PipelineState::capture_tx` ; à l'arrêt, `run_pipeline` vide les frames
//! PCM accumulées (`PipelineState::pcm_rx`) et enchaîne
//! `asr::local` → `postprocess::cleanup` → `injection::windows`.

use std::{thread, time::Duration};

use tauri::{AppHandle, Emitter, Manager};

use crate::asr::AsrEngine;
use crate::audio::capture::CaptureCommand;
use crate::injection::TextInjector;
use crate::postprocess::cleanup;
use crate::storage::settings::Settings;
use crate::{AsrState, PipelineState};

/// Démarre une session de dictée (déclenchée par le hotkey ou l'UI — un
/// bouton dans `FloatingWidget`/`ModelPanel` reste à ajouter côté frontend,
/// hors périmètre de cette session ; la commande elle-même est prête).
#[tauri::command]
pub fn start_dictation(app: AppHandle) -> Result<(), String> {
    let pipeline = app.state::<PipelineState>();
    pipeline
        .capture_tx
        .send(CaptureCommand::Start)
        .map_err(|e| format!("démarrage de la capture : {e}"))?;
    let _ = app.emit("listening_started", ());
    Ok(())
}

/// Arrête la session de dictée en cours et déclenche le pipeline complet
/// (transcription → nettoyage → injection) sur l'audio capté.
#[tauri::command]
pub fn stop_dictation(app: AppHandle) -> Result<(), String> {
    let pipeline = app.state::<PipelineState>();
    pipeline
        .capture_tx
        .send(CaptureCommand::Stop)
        .map_err(|e| format!("arrêt de la capture : {e}"))?;
    let _ = app.emit("listening_stopped", ());
    run_pipeline(&app);
    Ok(())
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

/// Vide les frames PCM accumulées depuis le dernier `Start` et, si l'audio
/// capté n'est pas vide, enchaîne le pipeline complet : transcription
/// locale (`asr::local`) → nettoyage par règles (`postprocess::cleanup`) →
/// injection dans l'app active (`injection::windows`). Appelée à la fois
/// par `hotkey` (raccourci physique) et par `stop_dictation` (future UI),
/// pour ne garder qu'une seule implémentation du pipeline.
///
/// Bloquant (transcription + injection, ~1-2s sur machine de dev) — appelé
/// depuis le thread dédié du hotkey ou depuis le pool de threads Tauri pour
/// les commandes IPC, jamais depuis le thread d'événements principal.
pub fn run_pipeline(app: &AppHandle) {
    let pipeline = app.state::<PipelineState>();

    // Laisse le temps à un dernier callback `cpal` déjà en vol de pousser sa
    // frame avant qu'on vide le canal — la capture vient d'être mise en
    // pause (`CaptureCommand::Stop`), pas coupée instantanément.
    thread::sleep(Duration::from_millis(50));

    let pcm: Vec<f32> = {
        let rx = match pipeline.pcm_rx.lock() {
            Ok(rx) => rx,
            Err(e) => {
                eprintln!("[pipeline] mutex pcm_rx empoisonné : {e}");
                return;
            }
        };
        let mut buf = Vec::new();
        while let Ok(frame) = rx.try_recv() {
            buf.extend(frame);
        }
        buf
    };

    if pcm.is_empty() {
        // Rien capté (VAD a tout jugé silencieux, ou appui trop bref) — pas
        // une erreur, simplement rien à transcrire.
        return;
    }

    let _ = app.emit("dictation_processing", ());

    let asr_state = app.state::<AsrState>();
    let raw_text = match &asr_state.0 {
        Some(engine) => match engine.transcribe(&pcm) {
            Ok(result) => result.text,
            Err(e) => {
                eprintln!("[pipeline] échec de la transcription : {e}");
                let _ = app.emit("dictation_error", e);
                return;
            }
        },
        None => {
            eprintln!("[pipeline] moteur ASR indisponible (modèle non installé, voir asr::local::LocalAsrEngine::load)");
            let _ = app.emit("dictation_error", "moteur ASR local indisponible");
            return;
        }
    };

    let cleaned = cleanup::clean(&raw_text);
    if cleaned.is_empty() {
        let _ = app.emit("dictation_idle", ());
        return;
    }

    let injector = crate::injection::windows::WindowsInjector;
    if let Err(e) = injector.inject(&cleaned) {
        eprintln!("[pipeline] échec de l'injection : {e}");
        let _ = app.emit("dictation_error", e);
        return;
    }

    let _ = app.emit("dictation_idle", ());
}

#[cfg(test)]
mod tests {
    //! Test d'intégration réel du pipeline texte complet (transcription
    //! réelle -> nettoyage -> injection réelle dans une app focalisée),
    //! `#[ignore]` par défaut (nécessite le modèle ASR + une app cible
    //! focalisée manuellement, voir `asr::local::tests` et
    //! `injection::windows::tests` pour les mêmes prérequis). Contourne
    //! volontairement `PipelineState`/`AppHandle`/`cpal` : aucun micro
    //! physique n'est disponible dans cet environnement pour produire de la
    //! parole réelle en direct (limite documentée dans PROGRESS.md Session
    //! 6, pas laissée sous silence) — ce test exerce donc exactement les
    //! mêmes étapes que `run_pipeline` (`asr::local::transcribe` ->
    //! `postprocess::cleanup::clean` -> `injection::windows::inject`), à
    //! partir d'un fichier audio réel plutôt que d'un flux micro live. Le
    //! déclenchement (`hotkey`/`cpal`) est lui testé séparément en
    //! conditions réelles (voir PROGRESS.md) : appui/relâchement physiques
    //! du raccourci sur l'app qui tourne pour de vrai, sans crash.
    use crate::asr::local::LocalAsrEngine;
    use crate::asr::AsrEngine;
    use crate::injection::windows::WindowsInjector;
    use crate::injection::TextInjector;
    use crate::postprocess::cleanup;
    use std::path::PathBuf;
    use windows::core::BOOL;
    use windows::Win32::Foundation::{HWND, LPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{EnumWindows, GetWindowTextW, IsWindowVisible, SetForegroundWindow};

    /// Même helper que `injection::windows::tests::focus_window_containing`
    /// (dupliqué plutôt que partagé — un seul appelant de chaque côté, pas
    /// assez de réutilisation pour justifier de le sortir en `pub`).
    /// Nécessaire car le terminal exécutant `cargo test` reprend le focus
    /// OS au démarrage du process (voir PROGRESS.md Session 6).
    fn focus_window_containing(needle: &str) -> bool {
        struct SearchState {
            needle_lower: String,
            found: Option<isize>,
        }
        unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
            unsafe {
                let state = &mut *(lparam.0 as *mut SearchState);
                if IsWindowVisible(hwnd).as_bool() {
                    let mut buf = [0u16; 512];
                    let len = GetWindowTextW(hwnd, &mut buf);
                    if len > 0 {
                        let title = String::from_utf16_lossy(&buf[..len as usize]);
                        if title.to_lowercase().contains(&state.needle_lower) {
                            state.found = Some(hwnd.0 as isize);
                            return BOOL(0);
                        }
                    }
                }
                BOOL(1)
            }
        }
        let mut state = SearchState { needle_lower: needle.to_lowercase(), found: None };
        unsafe {
            let _ = EnumWindows(Some(enum_proc), LPARAM(&mut state as *mut SearchState as isize));
        }
        match state.found {
            Some(raw) => {
                let hwnd = HWND(raw as *mut core::ffi::c_void);
                unsafe {
                    let _ = SetForegroundWindow(hwnd);
                }
                std::thread::sleep(std::time::Duration::from_millis(300));
                true
            }
            None => false,
        }
    }

    fn model_dir() -> PathBuf {
        let appdata = std::env::var("APPDATA").expect("APPDATA non défini (test Windows uniquement)");
        PathBuf::from(appdata).join("com.exponentvalue.vozel").join("models").join("parakeet-tdt-v3")
    }

    fn read_wav_mono_f32(path: &std::path::Path) -> Vec<f32> {
        let mut reader = hound::WavReader::open(path).expect("lecture du fichier WAV de test");
        let spec = reader.spec();
        assert_eq!(spec.sample_rate, 16_000, "fixture attendue en 16kHz");
        assert_eq!(spec.channels, 1, "fixture attendue en mono");
        match spec.sample_format {
            hound::SampleFormat::Float => reader.samples::<f32>().map(|s| s.unwrap()).collect(),
            hound::SampleFormat::Int => reader.samples::<i16>().map(|s| s.unwrap() as f32 / i16::MAX as f32).collect(),
        }
    }

    #[test]
    #[ignore]
    fn full_pipeline_transcribe_clean_inject() {
        let title = std::env::var("VOZEL_TEST_WINDOW_TITLE")
            .expect("VOZEL_TEST_WINDOW_TITLE non défini (sous-chaîne du titre de la fenêtre cible)");
        assert!(focus_window_containing(&title), "fenêtre contenant '{title}' introuvable");

        let engine = LocalAsrEngine::load_from_dir(&model_dir()).expect("chargement du moteur ASR");
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/asr/fr_00.wav");
        let pcm = read_wav_mono_f32(&fixture);

        let raw = engine.transcribe(&pcm).expect("transcription").text;
        eprintln!("[test] brut (asr::local)      : {raw:?}");
        let cleaned = cleanup::clean(&raw);
        eprintln!("[test] nettoyé (postprocess)   : {cleaned:?}");
        assert!(!cleaned.is_empty(), "le nettoyage ne doit pas vider un texte transcrit valide");

        WindowsInjector.inject(&cleaned).expect("injection");
        eprintln!("[test] injecté (injection::windows) — vérifier visuellement dans '{title}'");
    }
}
