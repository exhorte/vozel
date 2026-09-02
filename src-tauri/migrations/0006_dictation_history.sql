-- Historique local des dictées (prompt de reprise Session 16 +
-- 01_Recherche/Analyse_Fonctionnalites_WisprFlow_vs_Vozel.md §6 — pas de
-- section de spec, les specs ne couvrent que les Phases 1-2, closes).
--
-- Fondation partagée : la page d'accueil (compteurs jour/semaine + fil des
-- dictées récentes) la consomme dès cette session ; une future page
-- Statistiques et le Bloc-notes s'appuieront dessus aussi — d'où une seule
-- table plutôt que trois brouillons dispersés.
--
-- `text` : le texte final réellement inséré (remplacements du dictionnaire
-- + nettoyage règles/LLM déjà appliqués). Potentiellement sensible, jamais
-- filtré, jamais synchronisé (Vozel est 100 % local, sans compte) — c'est
-- pourquoi `clear()` / la commande `history_clear` ne sont pas optionnels :
-- l'utilisateur doit pouvoir tout effacer lui-même.
--
-- `duration_ms` NULLable : renseigné quand la durée d'enregistrement est
-- disponible à l'endroit où le pipeline branche l'écriture, absent sinon
-- (`word_count`, trivial à calculer depuis `text`, suffit aux statistiques
-- légères de la page d'accueil).
CREATE TABLE dictation_history (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at  TEXT    NOT NULL DEFAULT (datetime('now')),
    text        TEXT    NOT NULL,
    word_count  INTEGER NOT NULL,
    duration_ms INTEGER
);

-- Le fil récent trie par date décroissante, les compteurs jour/semaine
-- filtrent sur une borne `created_at >= ?` — index sur la colonne de tri.
CREATE INDEX idx_dictation_history_created_at ON dictation_history (created_at DESC);
