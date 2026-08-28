-- Schéma initial (Spec_Backend_Desktop.md §2.1).
-- `settings` : ligne unique (id = 1), miroir SQL de storage::settings::Settings
-- (jusque-là persisté en JSON en Phase 1 §1.6 — migration ponctuelle dans
-- storage::settings::ensure_migrated).
CREATE TABLE settings (
    id            INTEGER PRIMARY KEY CHECK (id = 1),
    asr_provider  TEXT    NOT NULL,
    hotkey        TEXT    NOT NULL,
    hotkey_mode   TEXT    NOT NULL,
    cloud_enabled INTEGER NOT NULL DEFAULT 0
);

-- `dictionary` : remplacements exacts appliqués avant le nettoyage par règles
-- (storage::dictionary, §2.2). `from_text` unique (une seule cible par
-- source) et indexé pour un lookup rapide (< 10 ms, critère §2.1).
CREATE TABLE dictionary (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    from_text  TEXT NOT NULL UNIQUE,
    to_text    TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_dictionary_from_text ON dictionary (from_text);
