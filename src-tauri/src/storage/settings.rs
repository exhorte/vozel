//! Réglages utilisateur : choix du modèle ASR (taille/local vs cloud),
//! raccourci clavier, fournisseur cloud + clé API, préférences de
//! confidentialité.
//!
//! Persistance : SQLite depuis la Phase 2 (Spec_Backend_Desktop.md §2.1),
//! ligne unique `settings.id = 1` (voir `migrations/0001_initial.sql`). La
//! Phase 1 (§1.6) persistait un fichier JSON dans `app_config_dir()` — la
//! bascule est faite une seule fois au démarrage par `ensure_migrated`, qui
//! importe l'ancien `settings.json` puis le renomme en `.migrated`.

use std::fmt;

use crate::hotkey::HotkeyMode;
use sqlx::{Row, SqlitePool};
use tauri::{AppHandle, Manager};

const LEGACY_SETTINGS_FILE_NAME: &str = "settings.json";

#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Settings {
    pub asr_provider: String,
    /// Syntaxe `global_hotkey::hotkey::HotKey` (ex. `"control+alt+Space"`).
    pub hotkey: String,
    pub hotkey_mode: HotkeyMode,
    pub cloud_enabled: bool,
    /// Fournisseur ASR cloud sélectionné (`"groq"` par défaut, voir
    /// `asr::cloud::Provider`). Utilisé seulement si `cloud_enabled`.
    pub cloud_provider: String,
    /// Clé API "BYO" du fournisseur cloud. **Jamais loguée en clair** — le
    /// `Debug` manuel ci-dessous la masque (Spec_Backend_Desktop.md §2.4).
    pub cloud_api_key: String,
    /// Nettoyage avancé par LLM local (§2.3). Opt-in, désactivé par défaut.
    /// Le modèle n'est chargé au démarrage que si ce drapeau est vrai ; un
    /// changement prend effet au redémarrage de l'app.
    pub llm_cleanup_enabled: bool,
    /// Push-to-talk sur le maintien de Ctrl+Win seul (§2.5). Opt-in,
    /// désactivé par défaut : installe un hook clavier bas niveau
    /// (`hotkey::modifier_combo`) au démarrage seulement si vrai ; un
    /// changement prend effet au redémarrage de l'app.
    pub ctrl_win_ptt_enabled: bool,
    /// Command Mode (Spec_Frontend.md §2.2) : palette de reformulation d'une
    /// sélection, déclenchée par `command_mode_hotkey`. Opt-in, désactivé par
    /// défaut : le raccourci dédié n'est enregistré et le modèle LLM n'est
    /// chargé pour cet usage qu'au démarrage si vrai ; effet au redémarrage.
    pub command_mode_enabled: bool,
    /// Raccourci global dédié au Command Mode (syntaxe
    /// `global_hotkey::hotkey::HotKey`, comme `hotkey`). Distinct du raccourci
    /// de dictée pour un déclenchement explicite et non ambigu.
    pub command_mode_hotkey: String,
}

// `Debug` manuel : la clé API ne doit jamais apparaître dans un log, un
// message d'erreur ou un rapport de panic, même via un `{:?}` involontaire.
impl fmt::Debug for Settings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Settings")
            .field("asr_provider", &self.asr_provider)
            .field("hotkey", &self.hotkey)
            .field("hotkey_mode", &self.hotkey_mode)
            .field("cloud_enabled", &self.cloud_enabled)
            .field("cloud_provider", &self.cloud_provider)
            .field(
                "cloud_api_key",
                &if self.cloud_api_key.is_empty() {
                    "<vide>"
                } else {
                    "<défini>"
                },
            )
            .field("llm_cleanup_enabled", &self.llm_cleanup_enabled)
            .field("ctrl_win_ptt_enabled", &self.ctrl_win_ptt_enabled)
            .field("command_mode_enabled", &self.command_mode_enabled)
            .field("command_mode_hotkey", &self.command_mode_hotkey)
            .finish()
    }
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
            cloud_provider: "groq".into(),
            cloud_api_key: String::new(),
            llm_cleanup_enabled: false,
            ctrl_win_ptt_enabled: false,
            command_mode_enabled: false,
            // Raccourci dédié au Command Mode, distinct du raccourci de dictée
            // (`control+shift+Space`). `alt+shift+KeyC` retenu par défaut :
            // `control+shift+KeyK` (essayé d'abord) est déjà pris globalement
            // sur la machine de dev — même situation que `control+alt+Space`
            // pour la dictée en Session 2. `alt+shift+*` est moins contesté et
            // évite Ctrl+Alt (= AltGr sur beaucoup de claviers). Modifiable
            // dans les réglages — effet au redémarrage.
            command_mode_hotkey: "alt+shift+KeyC".into(),
        }
    }
}

// `hotkey_mode` est stocké en TEXT ("toggle" / "push_to_talk"). On réutilise
// la représentation serde de l'enum (rename_all = "snake_case", voir
// `hotkey::HotkeyMode`) plutôt qu'un `match` manuel qui pourrait dériver.
fn hotkey_mode_to_str(mode: HotkeyMode) -> String {
    serde_json::to_value(mode)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_else(|| "toggle".to_owned())
}

fn hotkey_mode_from_str(raw: &str) -> HotkeyMode {
    serde_json::from_value(serde_json::Value::String(raw.to_owned())).unwrap_or_else(|_| {
        eprintln!("[storage::settings] hotkey_mode inconnu '{raw}', 'toggle' par défaut");
        HotkeyMode::Toggle
    })
}

impl Settings {
    /// Charge la ligne unique `settings.id = 1`. Retourne les valeurs par
    /// défaut (sans erreur) si la table est vide — `ensure_migrated`
    /// garantit normalement une ligne au démarrage, ce cas ne survient donc
    /// qu'en test ou si la migration ponctuelle a échoué.
    pub async fn load_db(pool: &SqlitePool) -> Result<Self, String> {
        let row = sqlx::query(
            "SELECT asr_provider, hotkey, hotkey_mode, cloud_enabled, cloud_provider, cloud_api_key,
                    llm_cleanup_enabled, ctrl_win_ptt_enabled, command_mode_enabled, command_mode_hotkey
             FROM settings WHERE id = 1",
        )
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("lecture des réglages : {e}"))?;

        let Some(row) = row else {
            return Ok(Self::default());
        };

        let cloud_enabled: i64 = row.try_get("cloud_enabled").map_err(|e| e.to_string())?;
        let llm_cleanup_enabled: i64 =
            row.try_get("llm_cleanup_enabled").map_err(|e| e.to_string())?;
        let ctrl_win_ptt_enabled: i64 = row
            .try_get("ctrl_win_ptt_enabled")
            .map_err(|e| e.to_string())?;
        let command_mode_enabled: i64 = row
            .try_get("command_mode_enabled")
            .map_err(|e| e.to_string())?;
        let hotkey_mode: String = row.try_get("hotkey_mode").map_err(|e| e.to_string())?;

        Ok(Self {
            asr_provider: row.try_get("asr_provider").map_err(|e| e.to_string())?,
            hotkey: row.try_get("hotkey").map_err(|e| e.to_string())?,
            hotkey_mode: hotkey_mode_from_str(&hotkey_mode),
            cloud_enabled: cloud_enabled != 0,
            cloud_provider: row.try_get("cloud_provider").map_err(|e| e.to_string())?,
            cloud_api_key: row.try_get("cloud_api_key").map_err(|e| e.to_string())?,
            llm_cleanup_enabled: llm_cleanup_enabled != 0,
            ctrl_win_ptt_enabled: ctrl_win_ptt_enabled != 0,
            command_mode_enabled: command_mode_enabled != 0,
            command_mode_hotkey: row
                .try_get("command_mode_hotkey")
                .map_err(|e| e.to_string())?,
        })
    }

    /// Écrit (upsert) la ligne unique `settings.id = 1`.
    pub async fn save_db(&self, pool: &SqlitePool) -> Result<(), String> {
        sqlx::query(
            "INSERT INTO settings
                 (id, asr_provider, hotkey, hotkey_mode, cloud_enabled, cloud_provider, cloud_api_key,
                  llm_cleanup_enabled, ctrl_win_ptt_enabled, command_mode_enabled, command_mode_hotkey)
             VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(id) DO UPDATE SET
                 asr_provider         = excluded.asr_provider,
                 hotkey               = excluded.hotkey,
                 hotkey_mode          = excluded.hotkey_mode,
                 cloud_enabled        = excluded.cloud_enabled,
                 cloud_provider       = excluded.cloud_provider,
                 cloud_api_key        = excluded.cloud_api_key,
                 llm_cleanup_enabled  = excluded.llm_cleanup_enabled,
                 ctrl_win_ptt_enabled = excluded.ctrl_win_ptt_enabled,
                 command_mode_enabled = excluded.command_mode_enabled,
                 command_mode_hotkey  = excluded.command_mode_hotkey",
        )
        .bind(&self.asr_provider)
        .bind(&self.hotkey)
        .bind(hotkey_mode_to_str(self.hotkey_mode))
        .bind(self.cloud_enabled as i64)
        .bind(&self.cloud_provider)
        .bind(&self.cloud_api_key)
        .bind(self.llm_cleanup_enabled as i64)
        .bind(self.ctrl_win_ptt_enabled as i64)
        .bind(self.command_mode_enabled as i64)
        .bind(&self.command_mode_hotkey)
        .execute(pool)
        .await
        .map_err(|e| format!("écriture des réglages : {e}"))?;
        Ok(())
    }

    /// Migration ponctuelle Phase 1 → Phase 2 : si la table `settings` est
    /// vide, y insère soit les réglages lus depuis l'ancien `settings.json`,
    /// soit les valeurs par défaut. L'ancien fichier est ensuite renommé en
    /// `settings.json.migrated` pour ne pas rejouer la migration ni laisser
    /// deux sources de vérité.
    pub async fn ensure_migrated(pool: &SqlitePool, app: &AppHandle) -> Result<(), String> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM settings")
            .fetch_one(pool)
            .await
            .map_err(|e| format!("vérification de la table settings : {e}"))?;
        if count > 0 {
            return Ok(());
        }

        let legacy = Self::read_legacy_json(app);
        let settings = legacy.clone().unwrap_or_default();
        settings.save_db(pool).await?;

        if legacy.is_some() {
            if let Ok(path) = Self::legacy_json_path(app) {
                let target = path.with_file_name("settings.json.migrated");
                let _ = std::fs::rename(&path, &target);
            }
            println!("[storage::settings] réglages importés depuis settings.json vers SQLite");
        }
        Ok(())
    }

    fn legacy_json_path(app: &AppHandle) -> Result<std::path::PathBuf, String> {
        let dir = app
            .path()
            .app_config_dir()
            .map_err(|e| format!("répertoire de config introuvable : {e}"))?;
        Ok(dir.join(LEGACY_SETTINGS_FILE_NAME))
    }

    /// Lit l'ancien fichier JSON de la Phase 1 s'il existe et est valide.
    /// `None` si absent (rien à migrer) ; un fichier corrompu est signalé
    /// dans les logs et traité comme absent (on ne bloque pas le démarrage).
    fn read_legacy_json(app: &AppHandle) -> Option<Self> {
        let path = Self::legacy_json_path(app).ok()?;
        let raw = std::fs::read_to_string(&path).ok()?;
        match serde_json::from_str(&raw) {
            Ok(s) => Some(s),
            Err(e) => {
                eprintln!(
                    "[storage::settings] ancien settings.json illisible ({e}), migration ignorée"
                );
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

    async fn memory_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(SqliteConnectOptions::new().filename(":memory:"))
            .await
            .expect("pool mémoire");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("migrations");
        pool
    }

    #[test]
    fn defaults_when_table_empty() {
        tauri::async_runtime::block_on(async {
            let pool = memory_pool().await;
            let loaded = Settings::load_db(&pool).await.expect("load");
            assert_eq!(loaded, Settings::default());
        });
    }

    #[test]
    fn round_trips_through_sqlite() {
        tauri::async_runtime::block_on(async {
            let pool = memory_pool().await;
            let written = Settings {
                asr_provider: "whisper-cpp".into(),
                hotkey: "alt+shift+KeyD".into(),
                hotkey_mode: HotkeyMode::PushToTalk,
                cloud_enabled: true,
                cloud_provider: "openai".into(),
                cloud_api_key: "sk-test-secret".into(),
                llm_cleanup_enabled: true,
                ctrl_win_ptt_enabled: true,
                command_mode_enabled: true,
                command_mode_hotkey: "control+alt+KeyR".into(),
            };
            written.save_db(&pool).await.expect("save");
            let reloaded = Settings::load_db(&pool).await.expect("load");
            assert_eq!(reloaded, written);

            // Deuxième save : upsert, pas d'insertion en double.
            let updated = Settings {
                cloud_enabled: false,
                ..written
            };
            updated.save_db(&pool).await.expect("save 2");
            let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM settings")
                .fetch_one(&pool)
                .await
                .expect("count");
            assert_eq!(count, 1);
            assert_eq!(Settings::load_db(&pool).await.expect("load 3"), updated);
        });
    }

    #[test]
    fn debug_never_prints_api_key() {
        let s = Settings {
            cloud_api_key: "sk-super-secret-value".into(),
            ..Settings::default()
        };
        let dbg = format!("{s:?}");
        assert!(!dbg.contains("sk-super-secret-value"), "clé API fuitée dans Debug : {dbg}");
        assert!(dbg.contains("<défini>"));
        assert!(format!("{:?}", Settings::default()).contains("<vide>"));
    }

    #[test]
    fn hotkey_mode_strings_match_serde() {
        assert_eq!(hotkey_mode_to_str(HotkeyMode::Toggle), "toggle");
        assert_eq!(hotkey_mode_to_str(HotkeyMode::PushToTalk), "push_to_talk");
        assert_eq!(hotkey_mode_from_str("toggle"), HotkeyMode::Toggle);
        assert_eq!(hotkey_mode_from_str("push_to_talk"), HotkeyMode::PushToTalk);
        assert_eq!(hotkey_mode_from_str("bogus"), HotkeyMode::Toggle);
    }
}
