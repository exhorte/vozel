//! Réglages utilisateur : choix du moteur ASR (local vs cloud), fournisseur
//! cloud + clé API, nettoyage IA local.
//!
//! Persistance : SQLite depuis la Phase 2 (Spec_Backend_Desktop.md §2.1),
//! ligne unique `settings.id = 1` (voir `migrations/0001_initial.sql`). La
//! Phase 1 (§1.6) persistait un fichier JSON dans `app_config_dir()` — la
//! bascule est faite une seule fois au démarrage par `ensure_migrated`, qui
//! importe l'ancien `settings.json` puis le renomme en `.migrated`.
//!
//! Le déclenchement de la dictée n'est **pas** un réglage : c'est le maintien
//! de Ctrl + Win, câblé en dur (`hotkey::modifier_combo`, demande utilisateur
//! 2026-09-02). Les anciennes colonnes `hotkey` / `hotkey_mode` /
//! `ctrl_win_ptt_enabled` / `command_mode_*` ont été retirées par
//! `migrations/0007_ctrl_win_only.sql`.

use std::fmt;

use sqlx::{Row, SqlitePool};
use tauri::{AppHandle, Manager};

const LEGACY_SETTINGS_FILE_NAME: &str = "settings.json";

#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Settings {
    pub asr_provider: String,
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
}

// `Debug` manuel : la clé API ne doit jamais apparaître dans un log, un
// message d'erreur ou un rapport de panic, même via un `{:?}` involontaire.
impl fmt::Debug for Settings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Settings")
            .field("asr_provider", &self.asr_provider)
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
            cloud_enabled: false,
            cloud_provider: "groq".into(),
            cloud_api_key: String::new(),
            llm_cleanup_enabled: false,
        }
    }
}

impl Settings {
    /// Charge la ligne unique `settings.id = 1`. Retourne les valeurs par
    /// défaut (sans erreur) si la table est vide — `ensure_migrated`
    /// garantit normalement une ligne au démarrage, ce cas ne survient donc
    /// qu'en test ou si la migration ponctuelle a échoué.
    pub async fn load_db(pool: &SqlitePool) -> Result<Self, String> {
        let row = sqlx::query(
            "SELECT asr_provider, cloud_enabled, cloud_provider, cloud_api_key, llm_cleanup_enabled
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

        Ok(Self {
            asr_provider: row.try_get("asr_provider").map_err(|e| e.to_string())?,
            cloud_enabled: cloud_enabled != 0,
            cloud_provider: row.try_get("cloud_provider").map_err(|e| e.to_string())?,
            cloud_api_key: row.try_get("cloud_api_key").map_err(|e| e.to_string())?,
            llm_cleanup_enabled: llm_cleanup_enabled != 0,
        })
    }

    /// Écrit (upsert) la ligne unique `settings.id = 1`.
    pub async fn save_db(&self, pool: &SqlitePool) -> Result<(), String> {
        sqlx::query(
            "INSERT INTO settings
                 (id, asr_provider, cloud_enabled, cloud_provider, cloud_api_key, llm_cleanup_enabled)
             VALUES (1, ?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(id) DO UPDATE SET
                 asr_provider        = excluded.asr_provider,
                 cloud_enabled       = excluded.cloud_enabled,
                 cloud_provider      = excluded.cloud_provider,
                 cloud_api_key       = excluded.cloud_api_key,
                 llm_cleanup_enabled = excluded.llm_cleanup_enabled",
        )
        .bind(&self.asr_provider)
        .bind(self.cloud_enabled as i64)
        .bind(&self.cloud_provider)
        .bind(&self.cloud_api_key)
        .bind(self.llm_cleanup_enabled as i64)
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
                cloud_enabled: true,
                cloud_provider: "openai".into(),
                cloud_api_key: "sk-test-secret".into(),
                llm_cleanup_enabled: true,
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
}
