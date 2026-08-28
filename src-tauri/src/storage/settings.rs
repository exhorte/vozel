//! Réglages utilisateur : choix du modèle ASR (taille/local vs cloud),
//! raccourci clavier, fournisseur cloud + clé API, préférences de
//! confidentialité.
//!
//! Persistance Phase 1 (Spec_Backend_Desktop.md §1.6) : fichier JSON dans le
//! répertoire de config de l'app (`Settings::PATH` relatif à
//! `app.path().app_config_dir()`) — le SQLite complet reste Phase 2
//! (`storage::db`, migration prévue depuis ce même fichier JSON).

use crate::hotkey::HotkeyMode;
use tauri::{AppHandle, Manager};

const SETTINGS_FILE_NAME: &str = "settings.json";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Settings {
    pub asr_provider: String,
    /// Syntaxe `global_hotkey::hotkey::HotKey` (ex. `"control+alt+Space"`).
    pub hotkey: String,
    pub hotkey_mode: HotkeyMode,
    pub cloud_enabled: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            // Moteur ASR local par défaut : Parakeet-TDT (ONNX INT8),
            // tranché par le benchmark français Session 4 (voir
            // PROGRESS.md, ~4-5x plus rapide et WER plus bas que
            // whisper.cpp sur l'échantillon testé). whisper.cpp reste un
            // stub non branché (Spec_Backend_Desktop.md §1.3).
            asr_provider: "parakeet-tdt".into(),
            // "Fn" seul n'est pas utilisable : sur la plupart des claviers
            // laptop, la touche Fn est interceptée par le contrôleur clavier
            // (firmware) et n'atteint jamais l'OS comme un événement clavier
            // normal — `RegisterHotKey` (Win32) ne peut donc pas s'y
            // enregistrer. `control+alt+Space` testé et rejeté aussi (déjà
            // pris par une autre app/le système sur la machine de dev —
            // Ctrl+Alt est par ailleurs l'équivalent d'AltGr sur beaucoup de
            // claviers non-US, donc à éviter pour un raccourci global).
            // Défaut retenu : `control+shift+Space`, modifiable dans les
            // réglages (`Spec_Frontend.md` Phase 1 §1.2).
            hotkey: "control+shift+Space".into(),
            hotkey_mode: HotkeyMode::Toggle,
            cloud_enabled: false,
        }
    }
}

impl Settings {
    fn file_path(app: &AppHandle) -> Result<std::path::PathBuf, String> {
        let dir = app
            .path()
            .app_config_dir()
            .map_err(|e| format!("répertoire de config introuvable : {e}"))?;
        Ok(dir.join(SETTINGS_FILE_NAME))
    }

    /// Charge les réglages depuis le fichier JSON persisté. Retourne les
    /// valeurs par défaut (sans erreur) si le fichier n'existe pas encore
    /// (premier lancement) — un fichier illisible ou corrompu est signalé
    /// dans les logs mais ne doit jamais empêcher le démarrage de l'app.
    pub fn load(app: &AppHandle) -> Self {
        let path = match Self::file_path(app) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("[storage::settings] {e}, utilisation des valeurs par défaut");
                return Self::default();
            }
        };
        match std::fs::read_to_string(&path) {
            Ok(raw) => serde_json::from_str(&raw).unwrap_or_else(|e| {
                eprintln!(
                    "[storage::settings] fichier de config illisible ({e}), utilisation des valeurs par défaut"
                );
                Self::default()
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Self::default(),
            Err(e) => {
                eprintln!(
                    "[storage::settings] échec de lecture de '{}' ({e}), utilisation des valeurs par défaut",
                    path.display()
                );
                Self::default()
            }
        }
    }

    /// Persiste les réglages dans le fichier JSON, en créant le répertoire
    /// de config si besoin (absent au tout premier lancement).
    pub fn save(&self, app: &AppHandle) -> Result<(), String> {
        let path = Self::file_path(app)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("création du répertoire de config impossible : {e}"))?;
        }
        let raw = serde_json::to_string_pretty(self)
            .map_err(|e| format!("sérialisation des réglages impossible : {e}"))?;
        std::fs::write(&path, raw)
            .map_err(|e| format!("écriture de '{}' impossible : {e}", path.display()))
    }
}
