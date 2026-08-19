// Types partagés avec le backend (miroir de src-tauri/src/storage/settings.rs
// et src-tauri/src/asr/types.rs). À garder synchronisé au fil du projet.

export interface Settings {
  asr_provider: string;
  hotkey: string;
  cloud_enabled: boolean;
}

export interface TranscriptionResult {
  text: string;
  language?: string;
  confidence?: number;
}

export type DictationStatus = "idle" | "listening" | "processing" | "error";
