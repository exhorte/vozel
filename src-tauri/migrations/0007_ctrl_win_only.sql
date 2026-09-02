-- Session 16 (suite) — demande utilisateur : la dictée n'a plus qu'UN seul
-- déclencheur, le **maintien de Ctrl + Win** (hook bas niveau
-- `WH_KEYBOARD_LL`, `hotkey::modifier_combo`), toujours actif. Relâcher l'une
-- des deux touches arrête l'enregistrement et lance transcription + insertion
-- dans le champ actif.
--
-- Sont donc supprimés :
--   * le raccourci global configurable (`hotkey`) et son mode
--     `Toggle`/`PushToTalk` (`hotkey_mode`) — crate `global-hotkey` /
--     `HotkeyManager`, retirée ;
--   * l'ancien opt-in `ctrl_win_ptt_enabled` — le combo Ctrl+Win devient le
--     comportement unique, plus une option ;
--   * le Command Mode et son raccourci dédié (`command_mode_enabled`,
--     `command_mode_hotkey`) — « pas d'autre raccourci que Ctrl+Win ».
--
-- Écart assumé vs `Spec_Backend_Desktop.md` §1.1 (« les deux modes
-- configurables dans les réglages ») et §2.3.3 (Command Mode) — décision
-- utilisateur (2026-09-02), documentée dans PROGRESS.md.
ALTER TABLE settings DROP COLUMN hotkey;
ALTER TABLE settings DROP COLUMN hotkey_mode;
ALTER TABLE settings DROP COLUMN ctrl_win_ptt_enabled;
ALTER TABLE settings DROP COLUMN command_mode_enabled;
ALTER TABLE settings DROP COLUMN command_mode_hotkey;
