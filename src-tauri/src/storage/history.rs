//! Historique local des dictées (prompt de reprise Session 16 +
//! `01_Recherche/Analyse_Fonctionnalites_WisprFlow_vs_Vozel.md` §6). Pas de
//! spec formelle : les specs ne couvrent que les Phases 1-2, closes.
//!
//! Sur le modèle exact de `storage::dictionary` : structs `Serialize`/
//! `Deserialize`, fonctions `async fn` prenant `&SqlitePool`, erreurs
//! `Result<_, String>` avec message clair, tests sur base `:memory:`.
//!
//! Écrit par `commands::run_pipeline` après une injection réussie
//! (`record`), lu par la page d'accueil (`list_recent` pour le fil,
//! `stats_since` pour les compteurs jour/semaine). `clear` vide toute la
//! table — exposé via la commande IPC `history_clear` : le texte stocké est
//! celui réellement dicté par l'utilisateur, jamais filtré ni synchronisé,
//! il doit pouvoir l'effacer lui-même.

use sqlx::{Row, SqlitePool};

/// Une dictée passée. `text` est le texte final inséré (dictionnaire +
/// nettoyage déjà appliqués). `duration_ms` est absent si la durée n'était
/// pas disponible à l'enregistrement.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct HistoryEntry {
    pub id: i64,
    /// Horodatage UTC `YYYY-MM-DD HH:MM:SS` (comme `dictionary.created_at`).
    pub created_at: String,
    pub text: String,
    pub word_count: i64,
    pub duration_ms: Option<i64>,
}

/// Agrégat pour un intervalle (compteurs de la page d'accueil).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct HistoryStats {
    /// Nombre de dictées.
    pub count: i64,
    /// Somme des `word_count`.
    pub word_count: i64,
}

/// Nombre de mots d'un texte (séparateurs Unicode). Source unique du calcul,
/// utilisée à l'enregistrement — trivial, mais centralisé pour ne pas
/// diverger d'un appelant à l'autre.
pub fn count_words(text: &str) -> i64 {
    text.split_whitespace().count() as i64
}

fn row_to_entry(row: &sqlx::sqlite::SqliteRow) -> Result<HistoryEntry, String> {
    Ok(HistoryEntry {
        id: row.try_get("id").map_err(|e| e.to_string())?,
        created_at: row.try_get("created_at").map_err(|e| e.to_string())?,
        text: row.try_get("text").map_err(|e| e.to_string())?,
        word_count: row.try_get("word_count").map_err(|e| e.to_string())?,
        duration_ms: row.try_get("duration_ms").map_err(|e| e.to_string())?,
    })
}

/// Enregistre une dictée. `duration_ms` = `None` si la durée n'était pas
/// disponible ; `word_count` calculé par l'appelant via [`count_words`].
/// Un `text` vide est refusé — il n'y a rien à historiser.
pub async fn record(
    pool: &SqlitePool,
    text: &str,
    duration_ms: Option<i64>,
    word_count: i64,
) -> Result<(), String> {
    if text.trim().is_empty() {
        return Err("texte vide : rien à enregistrer dans l'historique".into());
    }
    sqlx::query(
        "INSERT INTO dictation_history (text, word_count, duration_ms) VALUES (?1, ?2, ?3)",
    )
    .bind(text)
    .bind(word_count)
    .bind(duration_ms)
    .execute(pool)
    .await
    .map_err(|e| format!("enregistrement de l'historique : {e}"))?;
    Ok(())
}

/// Les `limit` dictées les plus récentes, plus récentes d'abord (pour le fil
/// de la page d'accueil).
pub async fn list_recent(pool: &SqlitePool, limit: i64) -> Result<Vec<HistoryEntry>, String> {
    let rows = sqlx::query(
        "SELECT id, created_at, text, word_count, duration_ms
         FROM dictation_history
         ORDER BY created_at DESC, id DESC
         LIMIT ?1",
    )
    .bind(limit.max(0))
    .fetch_all(pool)
    .await
    .map_err(|e| format!("lecture de l'historique : {e}"))?;
    rows.iter().map(row_to_entry).collect()
}

/// Compteurs (nombre de dictées + total de mots) depuis l'instant `since`,
/// en une seule requête d'agrégation. `since` est comparé à `created_at`
/// après normalisation par `datetime()` : accepte donc un ISO8601 avec `Z`
/// ou millisecondes (`2026-09-02T00:00:00.000Z`) comme le `YYYY-MM-DD
/// HH:MM:SS` déjà stocké.
pub async fn stats_since(pool: &SqlitePool, since: &str) -> Result<HistoryStats, String> {
    let row = sqlx::query(
        "SELECT COUNT(*) AS n, COALESCE(SUM(word_count), 0) AS words
         FROM dictation_history
         WHERE created_at >= datetime(?1)",
    )
    .bind(since)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("agrégation de l'historique : {e}"))?;
    Ok(HistoryStats {
        count: row.try_get("n").map_err(|e| e.to_string())?,
        word_count: row.try_get("words").map_err(|e| e.to_string())?,
    })
}

/// Vide toute la table (bouton « Effacer l'historique » de la page
/// d'accueil, commande IPC `history_clear`).
pub async fn clear(pool: &SqlitePool) -> Result<(), String> {
    sqlx::query("DELETE FROM dictation_history")
        .execute(pool)
        .await
        .map_err(|e| format!("effacement de l'historique : {e}"))?;
    Ok(())
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
    fn count_words_counts_whitespace_separated_tokens() {
        assert_eq!(count_words(""), 0);
        assert_eq!(count_words("   "), 0);
        assert_eq!(count_words("bonjour"), 1);
        assert_eq!(count_words("  la réunion  est   déplacée "), 4);
    }

    #[test]
    fn record_then_list_recent_round_trip() {
        tauri::async_runtime::block_on(async {
            let pool = memory_pool().await;

            record(&pool, "première dictée", Some(1_500), count_words("première dictée"))
                .await
                .expect("record 1");
            record(&pool, "seconde dictée un peu plus longue", None, 5)
                .await
                .expect("record 2");

            let recent = list_recent(&pool, 10).await.expect("list_recent");
            assert_eq!(recent.len(), 2);
            // Plus récente d'abord (id DESC départage l'égalité de created_at).
            assert_eq!(recent[0].text, "seconde dictée un peu plus longue");
            assert_eq!(recent[0].duration_ms, None);
            assert_eq!(recent[1].text, "première dictée");
            assert_eq!(recent[1].duration_ms, Some(1_500));
            assert_eq!(recent[1].word_count, 2);

            // `limit` respecté.
            assert_eq!(list_recent(&pool, 1).await.unwrap().len(), 1);
        });
    }

    #[test]
    fn record_rejects_empty_text() {
        tauri::async_runtime::block_on(async {
            let pool = memory_pool().await;
            assert!(record(&pool, "   ", None, 0).await.is_err());
            assert!(list_recent(&pool, 10).await.unwrap().is_empty());
        });
    }

    #[test]
    fn stats_since_counts_and_sums_within_window() {
        tauri::async_runtime::block_on(async {
            let pool = memory_pool().await;
            // Trois entrées à des dates contrôlées : deux "récentes", une vieille.
            for (created_at, text, words) in [
                ("2026-09-02 09:00:00", "dictée du jour A", 4),
                ("2026-09-02 18:00:00", "dictée du jour B plus longue", 6),
                ("2026-08-01 12:00:00", "vieille dictée", 2),
            ] {
                sqlx::query(
                    "INSERT INTO dictation_history (created_at, text, word_count) VALUES (?1, ?2, ?3)",
                )
                .bind(created_at)
                .bind(text)
                .bind(words as i64)
                .execute(&pool)
                .await
                .expect("insertion datée");
            }

            let today = stats_since(&pool, "2026-09-02T00:00:00.000Z").await.expect("stats jour");
            assert_eq!(today, HistoryStats { count: 2, word_count: 10 });

            let month = stats_since(&pool, "2026-08-01 00:00:00").await.expect("stats mois");
            assert_eq!(month, HistoryStats { count: 3, word_count: 12 });

            // Fenêtre vide → agrégat à zéro, pas d'erreur.
            let future = stats_since(&pool, "2027-01-01 00:00:00").await.expect("stats futur");
            assert_eq!(future, HistoryStats::default());
        });
    }

    #[test]
    fn clear_empties_the_table() {
        tauri::async_runtime::block_on(async {
            let pool = memory_pool().await;
            record(&pool, "à effacer", None, 2).await.expect("record");
            clear(&pool).await.expect("clear");
            assert!(list_recent(&pool, 10).await.unwrap().is_empty());
            // Idempotent sur une table déjà vide.
            clear(&pool).await.expect("clear vide");
        });
    }
}
