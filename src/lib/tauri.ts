// Wrappers typés autour des commandes IPC exposées par le backend Rust
// (voir src-tauri/src/commands/mod.rs). Toute la communication front/back
// passe par ce fichier pour garder un point d'entrée unique et typé.

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
  DictationStatus,
  DictionaryEntry,
  HistoryEntry,
  HistoryStatsPair,
  Settings,
} from "../types";

export async function startDictation(): Promise<void> {
  return invoke("start_dictation");
}

export async function stopDictation(): Promise<void> {
  return invoke("stop_dictation");
}

export async function getSettings(): Promise<Settings> {
  return invoke("get_settings");
}

export async function saveSettings(settings: Settings): Promise<void> {
  return invoke("save_settings", { settings });
}

// Le modèle LLM local (~1,9 Go, non commité) est-il présent dans
// `models/llm/` du répertoire de données de l'app ? Utilisé par ModelPanel
// (Spec_Frontend.md §2.4) pour signaler un switch « nettoyage IA » actif
// sans modèle installé — le backend retombe alors silencieusement sur les
// règles. Simple test d'existence côté Rust, ne charge pas le modèle.
export async function llmModelAvailable(): Promise<boolean> {
  return invoke("llm_model_available");
}

// --- Dictionnaire personnalisé (Spec_Backend_Desktop.md §2.2, commandes
// `dict_*`). Le backend normalise `from`/`to` (trim + espaces), refuse un
// `from` vide ou en doublon, et renvoie un message d'erreur explicite. ---

export async function dictList(): Promise<DictionaryEntry[]> {
  return invoke("dict_list");
}

export async function dictCreate(from: string, to: string): Promise<DictionaryEntry> {
  return invoke("dict_create", { from, to });
}

export async function dictUpdate(id: number, from: string, to: string): Promise<void> {
  return invoke("dict_update", { id, from, to });
}

export async function dictDelete(id: number): Promise<void> {
  return invoke("dict_delete", { id });
}

// --- Historique local des dictées (Session 16, pas de section de spec —
// prompt de reprise + Analyse_Fonctionnalites_WisprFlow_vs_Vozel.md §6). La
// table est alimentée par le pipeline après chaque dictée injectée ; ces
// appels servent la page d'accueil. ---

/** Les dernières dictées, plus récentes d'abord (`limit` borné 1..100 côté
 *  backend, défaut 10). */
export async function historyList(limit?: number): Promise<HistoryEntry[]> {
  return invoke("history_list", { limit });
}

/** Compteurs jour + semaine. Les bornes sont calculées ici (fuseau local) :
 *  `todaySince` = minuit local, `weekSince` = il y a 7 jours, en ISO8601. */
export async function historyStats(
  todaySince: string,
  weekSince: string,
): Promise<HistoryStatsPair> {
  return invoke("history_stats", { todaySince, weekSince });
}

/** Vide tout l'historique local (confirmation demandée côté UI). */
export async function historyClear(): Promise<void> {
  return invoke("history_clear");
}

// Écoute tous les événements de cycle de vie de la dictée, émis à la fois
// par `hotkey::HotkeyManager` (`listening_started`/`listening_stopped`,
// sur appui du raccourci physique) et par `commands::run_pipeline`
// (`dictation_processing`/`dictation_idle`/`dictation_error`, pendant et
// après la transcription — voir Spec_Backend_Desktop.md §1.6). Une seule
// source de vérité : que la dictée soit déclenchée par le hotkey ou par un
// bouton UI (`startDictation`/`stopDictation` ci-dessus, qui invoquent les
// mêmes commandes backend que le hotkey), les mêmes événements arrivent
// ici — pas de chemin d'état parallèle à synchroniser à la main. Retourne
// une fonction de désabonnement à appeler au démontage du composant
// appelant.
export async function listenDictationStatus(
  onChange: (status: DictationStatus) => void,
): Promise<() => void> {
  const unlistenStart = await listen("listening_started", () => onChange("listening"));
  const unlistenStop = await listen("listening_stopped", () => onChange("idle"));
  const unlistenProcessing = await listen("dictation_processing", () => onChange("processing"));
  const unlistenIdle = await listen("dictation_idle", () => onChange("idle"));
  const unlistenError = await listen<string>("dictation_error", (event) => {
    console.error("[dictation] échec du pipeline :", event.payload);
    onChange("error");
  });
  return () => {
    unlistenStart();
    unlistenStop();
    unlistenProcessing();
    unlistenIdle();
    unlistenError();
  };
}

// Écoute le niveau d'amplitude audio émis pendant la capture (`audio_level`,
// pic RMS par frame 16 kHz, throttlé à 20 Hz — voir `src-tauri/src/audio/
// capture.rs`, Spec_Backend_Desktop.md §1.2).
export async function listenAudioLevel(
  onLevel: (level: number) => void,
): Promise<() => void> {
  const unlisten = await listen<number>("audio_level", (event) => onLevel(event.payload));
  return unlisten;
}
