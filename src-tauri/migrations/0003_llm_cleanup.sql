-- Nettoyage avancé par LLM local (Spec_Backend_Desktop.md §2.3) : opt-in,
-- désactivé par défaut (comme le cloud). Chargé au démarrage seulement si
-- activé — un changement prend effet au redémarrage de l'app (même limite
-- que le raccourci clavier).
ALTER TABLE settings ADD COLUMN llm_cleanup_enabled INTEGER NOT NULL DEFAULT 0;
