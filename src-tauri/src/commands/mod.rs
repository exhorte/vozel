//! Commandes IPC exposées au frontend (invoke depuis React/TS).
//! Point de couture entre l'UI et les modules `audio`/`asr`/`postprocess`/
//! `injection`/`storage`. À étoffer au fil des phases de la Roadmap.
//!
//! Pipeline complet (Spec_Backend_Desktop.md §1.6) : `hotkey` (ou ces
//! commandes, pour un futur bouton UI) pilotent `audio::capture` via
//! `PipelineState::capture_tx` ; à l'arrêt, `run_pipeline` vide les frames
//! PCM accumulées (`PipelineState::pcm_rx`) et enchaîne
//! `asr::local` → `postprocess::cleanup` → `injection::windows`.

use std::sync::atomic::Ordering;
use std::{thread, time::Duration};

use tauri::{AppHandle, Emitter, Manager};

use crate::audio::capture::CaptureCommand;
use crate::injection::TextInjector;
use crate::postprocess::command_mode::{self, Reformulation};
use crate::postprocess::llm::GenParams;
use crate::storage::db::Db;
use crate::storage::dictionary::{self, DictionaryEntry};
use crate::storage::settings::Settings;
use crate::{AsrState, CleanerState, CommandModeState, PendingCommand, PipelineState};

/// Démarre une session de dictée. Déclenchée par le hotkey global ou par
/// l'UI (clic sur `FloatingWidget`) — les deux passent par cette même
/// commande, sans chemin d'état parallèle. Le drapeau partagé
/// `PipelineState::listening` est la source de vérité commune : si une
/// dictée est déjà en cours (démarrée par l'autre déclencheur), l'appel est
/// idempotent et ne renvoie pas d'erreur.
#[tauri::command]
pub fn start_dictation(app: AppHandle) -> Result<(), String> {
    let pipeline = app.state::<PipelineState>();
    if pipeline.listening.swap(true, Ordering::SeqCst) {
        return Ok(());
    }
    pipeline
        .capture_tx
        .send(CaptureCommand::Start)
        .map_err(|e| {
            pipeline.listening.store(false, Ordering::SeqCst);
            format!("démarrage de la capture : {e}")
        })?;
    let _ = app.emit("listening_started", ());
    Ok(())
}

/// Arrête la session de dictée en cours et déclenche le pipeline complet
/// (transcription → nettoyage → injection) sur l'audio capté. Idempotente :
/// si aucune dictée n'est en cours (drapeau `listening` déjà à `false`,
/// p.ex. déjà arrêtée par le hotkey), l'appel ne fait rien.
#[tauri::command]
pub fn stop_dictation(app: AppHandle) -> Result<(), String> {
    let pipeline = app.state::<PipelineState>();
    if !pipeline.listening.swap(false, Ordering::SeqCst) {
        return Ok(());
    }
    pipeline
        .capture_tx
        .send(CaptureCommand::Stop)
        .map_err(|e| format!("arrêt de la capture : {e}"))?;
    let _ = app.emit("listening_stopped", ());
    run_pipeline(&app);
    Ok(())
}

/// Retourne les réglages courants pour la fenêtre de réglages (lus depuis
/// SQLite, `settings.id = 1` — voir `storage::settings::Settings::load_db`).
/// Ne peut pas échouer côté UI : une erreur de lecture est journalisée et
/// les valeurs par défaut sont renvoyées.
#[tauri::command]
pub async fn get_settings(app: AppHandle) -> Settings {
    let pool = app.state::<Db>().0.clone();
    Settings::load_db(&pool).await.unwrap_or_else(|e| {
        eprintln!("[commands::get_settings] {e}, valeurs par défaut");
        Settings::default()
    })
}

/// Indique si le modèle LLM local (~1,9 Go, non commité) est présent dans
/// `models/llm/`. Lecture seule — la fenêtre de réglages (Spec_Frontend.md
/// §2.4) s'en sert pour signaler un switch « nettoyage IA » actif alors
/// qu'aucun modèle n'est installé (sinon repli silencieux sur les règles).
/// Ne charge pas le modèle : simple test d'existence des fichiers.
#[tauri::command]
pub fn llm_model_available(app: AppHandle) -> bool {
    crate::postprocess::llm::model_present(&app)
}

/// Sauvegarde les réglages modifiés par l'utilisateur dans SQLite
/// (Spec_Backend_Desktop.md §2.1).
/// Note : ne réapplique pas à chaud un raccourci clavier modifié (le hotkey
/// global est enregistré une seule fois au démarrage, voir `hotkey::mod` —
/// un redémarrage de l'app est nécessaire pour l'instant, à lever quand
/// `HotkeyManager` gagnera une méthode de ré-enregistrement).
#[tauri::command]
pub async fn save_settings(app: AppHandle, settings: Settings) -> Result<(), String> {
    let pool = app.state::<Db>().0.clone();
    settings.save_db(&pool).await
}

// --- Dictionnaire personnalisé (Spec_Backend_Desktop.md §2.2) ---
// CRUD exposé au frontend (`Spec_Frontend.md` §2.1, `DictionaryPanel`). Les
// entrées sont appliquées dans `run_pipeline` → `cleanup::clean`.

/// Toutes les entrées du dictionnaire, plus récentes d'abord.
#[tauri::command]
pub async fn dict_list(app: AppHandle) -> Result<Vec<DictionaryEntry>, String> {
    let pool = app.state::<Db>().0.clone();
    dictionary::list(&pool).await
}

/// Ajoute une entrée (`from` → `to`). Erreur si `from` existe déjà.
#[tauri::command]
pub async fn dict_create(
    app: AppHandle,
    from: String,
    to: String,
) -> Result<DictionaryEntry, String> {
    let pool = app.state::<Db>().0.clone();
    dictionary::create(&pool, &from, &to).await
}

/// Modifie `from`/`to` de l'entrée `id`.
#[tauri::command]
pub async fn dict_update(
    app: AppHandle,
    id: i64,
    from: String,
    to: String,
) -> Result<(), String> {
    let pool = app.state::<Db>().0.clone();
    dictionary::update(&pool, id, &from, &to).await
}

/// Supprime l'entrée `id`.
#[tauri::command]
pub async fn dict_delete(app: AppHandle, id: i64) -> Result<(), String> {
    let pool = app.state::<Db>().0.clone();
    dictionary::delete(&pool, id).await
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

    // Dictionnaire personnalisé (§2.2) : rechargé à chaque dictée (petit,
    // lookup < 10 ms même à 1000 entrées — voir storage::db::tests) pour que
    // toute entrée ajoutée par l'utilisateur s'applique dès la dictée
    // suivante. Une erreur de lecture ne bloque pas la dictée : on nettoie
    // sans dictionnaire.
    let replacements = {
        let pool = app.state::<Db>().0.clone();
        tauri::async_runtime::block_on(crate::storage::dictionary::active_replacements(&pool))
            .unwrap_or_else(|e| {
                eprintln!("[pipeline] dictionnaire indisponible ({e}), nettoyage sans");
                Vec::new()
            })
    };
    // Nettoyage : règles ou LLM local selon les réglages / la présence du
    // modèle (§2.3). `run_pipeline` ne connaît que l'interface `TextCleaner`
    // — le repli sur les règles en cas de défaillance du LLM est interne à
    // `LlmCleaner`.
    let cleaned = app.state::<CleanerState>().0.clean(&raw_text, &replacements);
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

// --- Command Mode (Spec_Frontend.md §2.2 / Spec_Backend_Desktop.md §2.3.3) ---
//
// Déclenchement explicite par un raccourci global dédié
// (`Settings::command_mode_hotkey`, géré dans `hotkey::HotkeyManager`) :
//   1. `open_command_palette` capture la sélection courante (Ctrl+C synthétique
//      + presse-papiers, restauré derrière) et la fenêtre au premier plan, puis
//      affiche la fenêtre `command` (palette shadcn).
//   2. L'utilisateur choisit une reformulation → `run_command_mode(id)` : le
//      LLM local reformule, la fenêtre cible est ramenée au premier plan, et le
//      résultat est collé par-dessus la sélection (`injection::windows`).
//
// 100 % local (critère §2.3.4) : même `LlmEngine` que le nettoyage §2.3. Si le
// modèle n'est pas installé, `CommandModeState::engine` est `None` et la palette
// l'affiche (pas de repli « règles » — reformuler n'est pas nettoyer).

/// La fenêtre au premier plan (HWND brut en `isize`, `0` si aucune). Sert à
/// re-cibler l'app d'origine avant de coller le texte reformulé.
#[cfg(target_os = "windows")]
fn foreground_window() -> isize {
    use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
    unsafe { GetForegroundWindow() }.0 as isize
}

/// Ramène `hwnd` au premier plan (best-effort — `SetForegroundWindow` peut
/// être refusé par Windows selon le contexte de focus ; suffisant ici car
/// l'appel suit de peu une interaction clavier de l'utilisateur).
#[cfg(target_os = "windows")]
fn restore_foreground(hwnd: isize) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::SetForegroundWindow;
    if hwnd == 0 {
        return;
    }
    let hwnd = HWND(hwnd as *mut core::ffi::c_void);
    unsafe {
        let _ = SetForegroundWindow(hwnd);
    }
}

#[cfg(not(target_os = "windows"))]
fn foreground_window() -> isize {
    0
}
#[cfg(not(target_os = "windows"))]
fn restore_foreground(_hwnd: isize) {}

/// Capture la sélection courante de l'app au premier plan : sauvegarde le
/// presse-papiers, le vide, simule Ctrl+C, relit, puis restaure le contenu
/// d'origine (même précaution que `injection::clipboard::paste_and_restore`).
/// `None` si rien n'a été copié (aucune sélection) ou en cas d'échec.
fn capture_selection() -> Option<String> {
    use arboard::Clipboard;
    use enigo::{Direction, Enigo, Key, Keyboard, Settings as EnigoSettings};

    let mut clipboard = Clipboard::new().ok()?;
    let previous = clipboard.get_text().ok();
    // Vider d'abord : sinon une sélection vide laisserait l'ancien contenu et
    // on le prendrait à tort pour la sélection.
    let _ = clipboard.set_text(String::new());

    let mut enigo = Enigo::new(&EnigoSettings::default()).ok()?;
    let _ = enigo.key(Key::Control, Direction::Press);
    let _ = enigo.key(Key::C, Direction::Click);
    let _ = enigo.key(Key::Control, Direction::Release);
    thread::sleep(Duration::from_millis(120));

    let copied = clipboard
        .get_text()
        .ok()
        .filter(|s| !s.trim().is_empty());

    match previous {
        Some(prev) => {
            let _ = clipboard.set_text(prev);
        }
        None => {
            let _ = clipboard.clear();
        }
    }
    copied
}

/// Appelé depuis le thread du raccourci global (`hotkey::HotkeyManager`) sur
/// appui du raccourci Command Mode. Non `#[tauri::command]` : déclenché côté
/// Rust, pas depuis l'UI.
pub fn open_command_palette(app: &AppHandle) {
    let target_hwnd = foreground_window();
    let selected_text = capture_selection().unwrap_or_default();

    app.state::<CommandModeState>().store_pending(PendingCommand {
        selected_text: selected_text.clone(),
        target_hwnd,
    });

    println!(
        "[command-mode] palette ouverte (sélection : {} caractères)",
        selected_text.chars().count()
    );
    if let Some(win) = app.get_webview_window("command") {
        let _ = win.show();
        let _ = win.set_focus();
    } else {
        eprintln!("[command-mode] fenêtre 'command' introuvable");
    }
    // La palette (montée une fois, la fenêtre est cachée/réaffichée) réagit à
    // cet événement pour rafraîchir le texte affiché à chaque ouverture.
    let _ = app.emit("command_palette_opened", selected_text);
}

/// Contexte affiché par la palette à l'ouverture : la sélection capturée et si
/// le modèle LLM local est disponible pour ce mode.
#[derive(serde::Serialize)]
pub struct CommandModeContext {
    pub selected_text: String,
    pub model_available: bool,
}

/// État courant du Command Mode pour la palette (au montage et à chaque
/// `command_palette_opened`).
#[tauri::command]
pub fn command_mode_context(app: AppHandle) -> CommandModeContext {
    let cm = app.state::<CommandModeState>();
    let selected_text = cm
        .peek_pending()
        .map(|pc| pc.selected_text)
        .unwrap_or_default();
    CommandModeContext {
        selected_text,
        model_available: cm.engine.is_some(),
    }
}

/// Catalogue fixe de reformulations proposées dans la palette (§2.2 point 1).
#[tauri::command]
pub fn command_mode_reformulations() -> Vec<Reformulation> {
    command_mode::REFORMULATIONS.to_vec()
}

/// Applique la reformulation `reformulation_id` à la sélection en attente, via
/// le LLM local, puis colle le résultat par-dessus la sélection dans l'app
/// d'origine. Retourne le texte reformulé (la palette l'affiche brièvement
/// avant de se fermer). Bloquant (~10-40 s CPU sur le modèle int4) — exécuté
/// sur le pool de threads des commandes Tauri, pas le thread d'événements.
#[tauri::command]
pub fn run_command_mode(app: AppHandle, reformulation_id: String) -> Result<String, String> {
    let cm = app.state::<CommandModeState>();
    let engine = cm
        .engine
        .clone()
        .ok_or("Le modèle LLM local n'est pas installé — Command Mode indisponible (voir models\\llm\\).")?;

    let pending = cm
        .peek_pending()
        .ok_or("Aucune sélection en attente — relancez le raccourci Command Mode.")?;
    if pending.selected_text.trim().is_empty() {
        return Err("Aucun texte sélectionné au moment du déclenchement.".into());
    }

    let refm = command_mode::REFORMULATIONS
        .iter()
        .find(|r| r.id == reformulation_id)
        .ok_or_else(|| format!("reformulation inconnue : {reformulation_id}"))?;

    let out = command_mode::handle_command(
        &engine,
        refm.instruction,
        &pending.selected_text,
        &GenParams::default(),
    )?;

    // Cacher la palette et rendre l'app cible au premier plan avant de coller.
    if let Some(win) = app.get_webview_window("command") {
        let _ = win.hide();
    }
    restore_foreground(pending.target_hwnd);
    thread::sleep(Duration::from_millis(120));

    crate::injection::windows::WindowsInjector.inject(&out)?;

    cm.take_pending();
    Ok(out)
}

/// Ferme la palette sans rien appliquer (Échap / clic hors liste). Restaure le
/// premier plan de l'app d'origine et jette la sélection en attente.
#[tauri::command]
pub fn close_command_palette(app: AppHandle) {
    let cm = app.state::<CommandModeState>();
    let target_hwnd = cm.take_pending().map(|pc| pc.target_hwnd);
    if let Some(win) = app.get_webview_window("command") {
        let _ = win.hide();
    }
    if let Some(hwnd) = target_hwnd {
        restore_foreground(hwnd);
    }
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
        let cleaned = cleanup::clean(&raw, &[]);
        eprintln!("[test] nettoyé (postprocess)   : {cleaned:?}");
        assert!(!cleaned.is_empty(), "le nettoyage ne doit pas vider un texte transcrit valide");

        WindowsInjector.inject(&cleaned).expect("injection");
        eprintln!("[test] injecté (injection::windows) — vérifier visuellement dans '{title}'");
    }
}
