// Wrappers typés autour des commandes IPC exposées par le backend Rust
// (voir src-tauri/src/commands/mod.rs). Toute la communication front/back
// passe par ce fichier pour garder un point d'entrée unique et typé.

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
  CommandModeContext,
  DictationStatus,
  DictionaryEntry,
  Reformulation,
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

// --- Command Mode (Spec_Frontend.md §2.2). La palette vit dans la fenêtre
// Tauri `command`, affichée par le backend sur appui du raccourci dédié
// (`Settings::command_mode_hotkey`) après capture de la sélection courante.
// Aucun de ces appels ne manipule la fenêtre directement : `run`/`close`
// laissent le backend cacher la fenêtre et rendre le focus à l'app d'origine.

/** Sélection capturée + disponibilité du modèle, lues à l'ouverture de la
 *  palette (au montage et à chaque événement `command_palette_opened`). */
export async function commandModeContext(): Promise<CommandModeContext> {
  return invoke("command_mode_context");
}

/** Catalogue fixe de reformulations proposées dans la palette. */
export async function commandModeReformulations(): Promise<Reformulation[]> {
  return invoke("command_mode_reformulations");
}

/** Applique la reformulation `id` à la sélection en attente via le LLM local,
 *  puis colle le résultat par-dessus la sélection dans l'app d'origine.
 *  Retourne le texte reformulé. Peut être long (~10-40 s sur le modèle int4). */
export async function runCommandMode(reformulationId: string): Promise<string> {
  return invoke("run_command_mode", { reformulationId });
}

/** Ferme la palette sans rien appliquer (Échap). */
export async function closeCommandPalette(): Promise<void> {
  return invoke("close_command_palette");
}

/** L'ouverture de la palette par le backend (après capture de la sélection).
 *  Le payload est le texte sélectionné. Retourne une fonction de désabonnement. */
export async function listenCommandPaletteOpened(
  onOpen: (selectedText: string) => void,
): Promise<() => void> {
  return listen<string>("command_palette_opened", (event) => onOpen(event.payload));
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
