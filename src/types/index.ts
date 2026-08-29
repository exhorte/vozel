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
