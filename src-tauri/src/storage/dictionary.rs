//! Dictionnaire personnalisé : remplacements de mots/expressions et
//! vocabulaire spécialisé (noms propres, jargon métier) — voir analyse
//! section 2, facteur de rétention identifié chez Wispr Flow et Freestyle.
//!
//! Phase 2 (Spec_Backend_Desktop.md §2.2) : CRUD complet sur la table
//! `dictionary` (créée par `migrations/0001_initial.sql`), exposé au
//! frontend par les commandes `dict_*` (`commands/mod.rs`) et appliqué dans
//! `postprocess::cleanup::clean` avant les règles de nettoyage.

use sqlx::{Row, SqlitePool};

/// Une entrée de remplacement : `from` (tel que reconnu par l'ASR) →
/// `to` (forme voulue). `from` est unique dans la table.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DictionaryEntry {
    pub id: i64,
    pub from: String,
    pub to: String,
}

fn row_to_entry(row: &sqlx::sqlite::SqliteRow) -> Result<DictionaryEntry, String> {
    Ok(DictionaryEntry {
        id: row.try_get("id").map_err(|e| e.to_string())?,
        from: row.try_get("from_text").map_err(|e| e.to_string())?,
        to: row.try_get("to_text").map_err(|e| e.to_string())?,
    })
}

/// `from`/`to` nettoyés (trim + espaces internes normalisés). `from` vide
/// après nettoyage est refusé — une source vide n'a pas de sens et
/// produirait un remplacement qui matche partout.
fn normalize_pair(from: &str, to: &str) -> Result<(String, String), String> {
    let from = from.split_whitespace().collect::<Vec<_>>().join(" ");
    let to = to.trim().to_string();
    if from.is_empty() {
        return Err("le terme source ne peut pas être vide".into());
    }
    Ok((from, to))
}

/// Toutes les entrées, plus récentes d'abord.
pub async fn list(pool: &SqlitePool) -> Result<Vec<DictionaryEntry>, String> {
    let rows = sqlx::query(
        "SELECT id, from_text, to_text FROM dictionary ORDER BY created_at DESC, id DESC",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("lecture du dictionnaire : {e}"))?;
    rows.iter().map(row_to_entry).collect()
}

/// Ajoute une entrée. Erreur si `from` (après normalisation) existe déjà.
pub async fn create(pool: &SqlitePool, from: &str, to: &str) -> Result<DictionaryEntry, String> {
    let (from, to) = normalize_pair(from, to)?;
    let row = sqlx::query(
        "INSERT INTO dictionary (from_text, to_text) VALUES (?1, ?2)
         RETURNING id, from_text, to_text",
    )
    .bind(&from)
    .bind(&to)
    .fetch_one(pool)
    .await
    .map_err(|e| match &e {
        sqlx::Error::Database(db) if db.is_unique_violation() => {
            format!("une entrée existe déjà pour « {from} »")
        }
        _ => format!("ajout au dictionnaire : {e}"),
    })?;
    row_to_entry(&row)
}

/// Modifie `from`/`to` d'une entrée existante.
pub async fn update(pool: &SqlitePool, id: i64, from: &str, to: &str) -> Result<(), String> {
    let (from, to) = normalize_pair(from, to)?;
    let affected = sqlx::query("UPDATE dictionary SET from_text = ?1, to_text = ?2 WHERE id = ?3")
        .bind(&from)
        .bind(&to)
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| match &e {
            sqlx::Error::Database(db) if db.is_unique_violation() => {
                format!("une entrée existe déjà pour « {from} »")
            }
            _ => format!("modification du dictionnaire : {e}"),
        })?
        .rows_affected();
    if affected == 0 {
        return Err(format!("aucune entrée de dictionnaire avec l'id {id}"));
    }
    Ok(())
}

/// Supprime une entrée.
pub async fn delete(pool: &SqlitePool, id: i64) -> Result<(), String> {
    let affected = sqlx::query("DELETE FROM dictionary WHERE id = ?1")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| format!("suppression du dictionnaire : {e}"))?
        .rows_affected();
    if affected == 0 {
        return Err(format!("aucune entrée de dictionnaire avec l'id {id}"));
    }
    Ok(())
}

/// Paires `(from, to)` à appliquer par `postprocess::cleanup::clean`, triées
/// par longueur de `from` décroissante : une entrée plus spécifique
/// ("type script pro") l'emporte sur une plus courte ("type script") quand
/// les deux pourraient matcher.
pub async fn active_replacements(pool: &SqlitePool) -> Result<Vec<(String, String)>, String> {
    let mut pairs: Vec<(String, String)> = sqlx::query("SELECT from_text, to_text FROM dictionary")
        .fetch_all(pool)
        .await
        .map_err(|e| format!("lecture du dictionnaire : {e}"))?
        .iter()
        .map(|row| {
            Ok((
                row.try_get::<String, _>("from_text").map_err(|e| e.to_string())?,
                row.try_get::<String, _>("to_text").map_err(|e| e.to_string())?,
            ))
        })
        .collect::<Result<_, String>>()?;
    pairs.sort_by(|a, b| b.0.chars().count().cmp(&a.0.chars().count()));
    Ok(pairs)
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
    fn crud_round_trip() {
        tauri::async_runtime::block_on(async {
            let pool = memory_pool().await;

            let created = create(&pool, "  type   script ", "TypeScript").await.expect("create");
            assert_eq!(created.from, "type script"); // trim + espaces normalisés
            assert_eq!(created.to, "TypeScript");

            let all = list(&pool).await.expect("list");
            assert_eq!(all.len(), 1);
            assert_eq!(all[0], created);

            update(&pool, created.id, "type script", "Typescript")
                .await
                .expect("update");
            assert_eq!(list(&pool).await.unwrap()[0].to, "Typescript");

            delete(&pool, created.id).await.expect("delete");
            assert!(list(&pool).await.unwrap().is_empty());
        });
    }

    #[test]
    fn create_rejects_empty_source_and_duplicates() {
        tauri::async_runtime::block_on(async {
            let pool = memory_pool().await;
            assert!(create(&pool, "   ", "x").await.is_err());
            create(&pool, "vozel", "Vozel").await.expect("first");
            assert!(create(&pool, "vozel", "VOZEL").await.is_err()); // from unique
        });
    }

    #[test]
    fn update_and_delete_report_missing_id() {
        tauri::async_runtime::block_on(async {
            let pool = memory_pool().await;
            assert!(update(&pool, 999, "a", "b").await.is_err());
            assert!(delete(&pool, 999).await.is_err());
        });
    }

    #[test]
    fn active_replacements_sorted_by_source_length_desc() {
        tauri::async_runtime::block_on(async {
            let pool = memory_pool().await;
            create(&pool, "js", "JavaScript").await.unwrap();
            create(&pool, "type script", "TypeScript").await.unwrap();
            let pairs = active_replacements(&pool).await.unwrap();
            assert_eq!(pairs[0].0, "type script");
            assert_eq!(pairs[1].0, "js");
        });
    }
}
