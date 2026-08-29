-- Push-to-talk sur le maintien de Ctrl+Win seul (Spec_Backend_Desktop.md
-- §2.5) : opt-in, désactivé par défaut (comme le cloud et le LLM). Installe
-- un hook clavier bas niveau `WH_KEYBOARD_LL` au démarrage seulement si
-- activé — un changement prend effet au redémarrage de l'app.
ALTER TABLE settings ADD COLUMN ctrl_win_ptt_enabled INTEGER NOT NULL DEFAULT 0;
