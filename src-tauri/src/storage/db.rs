//! Connexion SQLite embarquée et migrations versionnées
//! (Spec_Backend_Desktop.md §2.1).
//!
//! Choix d'implémentation (déviation mineure signalée dans PROGRESS.md) :
//! `sqlx` en direct plutôt que `tauri-plugin-sql`. Tout l'accès à la base
//! est côté Rust — les réglages passent déjà par des commandes IPC
//! (`get_settings`/`save_settings`) et le dictionnaire (§2.2) est appliqué
//! dans `postprocess::cleanup`, qui tourne dans `run_pipeline`. Un
//! `SqlitePool` possédé dans l'état managé + `sqlx::migrate!` (explicitement
//! cité comme alternative acceptable par la spec) est plus simple et plus
//! robuste ici que l'accès au pool interne du plugin.

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use tauri::{AppHandle, Manager};

const DB_FILE_NAME: &str = "vozel.db";

/// Pool SQLite partagé, stocké dans l'état managé Tauri. `SqlitePool` est un
/// handle `Arc` clonable, `Send + Sync` — accédé depuis les commandes IPC
/// (réglages, CRUD dictionnaire) et depuis le pipeline (`postprocess`, §2.2).
pub struct Db(pub SqlitePool);

/// Ouvre (ou crée au premier lancement) la base dans le répertoire de config
/// de l'app — le **même** dossier que `settings.json` de la Phase 1, pour que
/// la migration ponctuelle JSON → SQLite (`settings::Settings::ensure_migrated`)
/// retrouve l'ancien fichier — puis applique les migrations versionnées
/// embarquées dans le binaire (`migrations/`).
pub async fn init(app: &AppHandle) -> Result<SqlitePool, String> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("répertoire de config introuvable : {e}"))?;
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("création du répertoire de config impossible : {e}"))?;
    let path = dir.join(DB_FILE_NAME);

    // `.filename(path)` plutôt qu'une URL `sqlite://...` : évite tout souci
    // d'échappement du chemin Windows (backslashes, `C:`, espaces).
    let options = SqliteConnectOptions::new()
        .filename(&path)
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(4)
        .connect_with(options)
        .await
        .map_err(|e| format!("ouverture de la base '{}' impossible : {e}", path.display()))?;

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .map_err(|e| format!("échec des migrations SQLite : {e}"))?;

    Ok(pool)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pool sur une base en mémoire, migrations appliquées. `max_connections(1)`
    /// : chaque connexion `:memory:` est une base distincte, il faut donc une
    /// seule connexion partagée pour que migrations et requêtes voient le même
    /// schéma.
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
    fn migrations_create_expected_schema() {
        tauri::async_runtime::block_on(async {
            let pool = memory_pool().await;
            // Tables applicatives uniquement (hors `_sqlx_migrations` et la
            // table interne `sqlite_sequence` créée par AUTOINCREMENT).
            let tables: Vec<String> = sqlx::query_scalar(
                "SELECT name FROM sqlite_master
                 WHERE type = 'table' AND name NOT LIKE '\\_%' ESCAPE '\\' AND name <> 'sqlite_sequence'
                 ORDER BY name",
            )
            .fetch_all(&pool)
            .await
            .expect("liste des tables");
            assert_eq!(tables, vec!["dictionary".to_string(), "settings".to_string()]);

            // Colonnes de `settings` conformes au struct Rust (0001 + 0002).
            let cols: Vec<String> =
                sqlx::query_scalar("SELECT name FROM pragma_table_info('settings') ORDER BY name")
                    .fetch_all(&pool)
                    .await
                    .expect("colonnes settings");
            assert_eq!(
                cols,
                vec![
                    "asr_provider".to_string(),
                    "cloud_api_key".to_string(),
                    "cloud_enabled".to_string(),
                    "cloud_provider".to_string(),
                    "hotkey".to_string(),
                    "hotkey_mode".to_string(),
                    "id".to_string(),
                ]
            );
        });
    }

    #[test]
    fn dictionary_lookup_stays_under_10ms() {
        tauri::async_runtime::block_on(async {
            let pool = memory_pool().await;
            for i in 0..1000 {
                sqlx::query("INSERT INTO dictionary (from_text, to_text) VALUES (?1, ?2)")
                    .bind(format!("source-{i}"))
                    .bind(format!("cible-{i}"))
                    .execute(&pool)
                    .await
                    .expect("insertion");
            }
            let start = std::time::Instant::now();
            let hit: String =
                sqlx::query_scalar("SELECT to_text FROM dictionary WHERE from_text = ?1")
                    .bind("source-777")
                    .fetch_one(&pool)
                    .await
                    .expect("lookup");
            let elapsed = start.elapsed();
            assert_eq!(hit, "cible-777");
            assert!(
                elapsed < std::time::Duration::from_millis(10),
                "lookup dictionnaire trop lent : {elapsed:?} (critère §2.1 : < 10 ms)"
            );
        });
    }
}
