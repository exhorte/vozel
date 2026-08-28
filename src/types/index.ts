// Types partagés avec le backend (miroir de src-tauri/src/storage/settings.rs
// et src-tauri/src/asr/types.rs). À garder synchronisé au fil du projet.

export type HotkeyMode = "push_to_talk" | "toggle";

export interface Settings {
  asr_provider: string;
  hotkey: string;
  hotkey_mode: HotkeyMode;
  cloud_enabled: boolean;
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
