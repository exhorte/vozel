// Types partagés avec le backend (miroir de src-tauri/src/storage/settings.rs
// et src-tauri/src/asr/types.rs). À garder synchronisé au fil du projet.

export type HotkeyMode = "push_to_talk" | "toggle";

export interface Settings {
  asr_provider: string;
  hotkey: string;
  hotkey_mode: HotkeyMode;
  cloud_enabled: boolean;
  /** Fournisseur ASR cloud ("groq" par défaut). UI dans Spec_Frontend.md §2.3. */
  cloud_provider: string;
  /** Clé API "BYO" du fournisseur cloud (jamais loguée côté backend). */
  cloud_api_key: string;
  /** Nettoyage avancé par LLM local (§2.3). Opt-in ; effet au redémarrage. */
  llm_cleanup_enabled: boolean;
  /** Push-to-talk sur le maintien de Ctrl+Win seul (§2.5). Opt-in (hook
   *  clavier bas niveau) ; effet au redémarrage. */
  ctrl_win_ptt_enabled: boolean;
  /** Command Mode (§2.2) : palette de reformulation d'une sélection,
   *  déclenchée par `command_mode_hotkey`. Opt-in ; effet au redémarrage. */
  command_mode_enabled: boolean;
  /** Raccourci global dédié au Command Mode (syntaxe
   *  `global_hotkey::hotkey::HotKey`, comme `hotkey`). */
  command_mode_hotkey: string;
}

// Miroir de src-tauri/src/postprocess/command_mode.rs::Reformulation.
// `id` : stable ; `label` : affiché dans la palette ; `instruction` : consigne
// envoyée au LLM (le frontend renvoie l'`id`, pas l'instruction).
export interface Reformulation {
  id: string;
  label: string;
  instruction: string;
}

// Retour de la commande `command_mode_context`.
export interface CommandModeContext {
  selected_text: string;
  model_available: boolean;
}

export interface TranscriptionResult {
  text: string;
  language?: string;
  confidence?: number;
}

// Miroir de src-tauri/src/storage/dictionary.rs::DictionaryEntry.
// `from` : tel que reconnu par l'ASR ; `to` : forme voulue.
export interface DictionaryEntry {
  id: number;
  from: string;
  to: string;
}

export type DictationStatus = "idle" | "listening" | "processing" | "error";

// Miroir de src-tauri/src/storage/history.rs::HistoryEntry (historique local
// des dictées, Session 16). `created_at` : UTC "YYYY-MM-DD HH:MM:SS".
// `duration_ms` : `null` si la durée n'était pas disponible à
// l'enregistrement.
export interface HistoryEntry {
  id: number;
  created_at: string;
  text: string;
  word_count: number;
  duration_ms: number | null;
}

// Miroir de src-tauri/src/storage/history.rs::HistoryStats.
export interface HistoryStats {
  count: number;
  word_count: number;
}

// Retour de la commande `history_stats` (compteurs jour + semaine).
export interface HistoryStatsPair {
  today: HistoryStats;
  week: HistoryStats;
}
