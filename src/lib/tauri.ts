// Wrappers typés autour des commandes IPC exposées par le backend Rust
// (voir src-tauri/src/commands/mod.rs). Toute la communication front/back
// passe par ce fichier pour garder un point d'entrée unique et typé.

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { Settings } from "../types";

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

// Écoute les événements de cycle de vie de la dictée émis par
// `hotkey::HotkeyManager` (`listening_started` / `listening_stopped`,
// voir `src-tauri/src/hotkey/mod.rs`). Retourne une fonction de
// désabonnement à appeler au démontage du composant appelant.
export async function listenDictationStatus(
  onStart: () => void,
  onStop: () => void,
): Promise<() => void> {
  const unlistenStart = await listen("listening_started", onStart);
  const unlistenStop = await listen("listening_stopped", onStop);
  return () => {
    unlistenStart();
    unlistenStop();
  };
}

// Écoute le niveau d'amplitude audio émis pendant la capture
// (`audio_level`, Spec_Backend_Desktop.md §1.2 — pas encore implémenté
// côté `audio::capture`, aucun événement ne sera reçu tant que ce module
// backend n'existe pas).
export async function listenAudioLevel(
  onLevel: (level: number) => void,
): Promise<() => void> {
  const unlisten = await listen<number>("audio_level", (event) => onLevel(event.payload));
  return unlisten;
}
