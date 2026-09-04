// Types partagés avec le backend (miroir de src-tauri/src/storage/settings.rs
// et src-tauri/src/asr/types.rs). À garder synchronisé au fil du projet.
//
// Le déclenchement de la dictée n'est plus un réglage : c'est le maintien de
// Ctrl + Win, câblé en dur (`hotkey::modifier_combo`, demande utilisateur
// 2026-09-02).

export interface Settings {
  asr_provider: string;
  cloud_enabled: boolean;
  /** Fournisseur ASR cloud ("groq" par défaut). UI dans Spec_Frontend.md §2.3. */
  cloud_provider: string;
  /** Clé API "BYO" du fournisseur cloud (jamais loguée côté backend). */
  cloud_api_key: string;
  /** Nettoyage avancé par LLM local (§2.3). Opt-in ; effet au redémarrage. */
  llm_cleanup_enabled: boolean;
  /** Langue de l'interface ("en" par défaut, ou "fr"). Demande utilisateur
   *  2026-09-04 — voir src/lib/i18n.ts. N'affecte que le frontend ; les
   *  messages d'erreur générés côté Rust restent en français. */
  ui_language: "en" | "fr";
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
