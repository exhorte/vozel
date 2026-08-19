// État global minimal de l'app (statut de dictée courant, réglages en
// mémoire). Placeholder simple pour l'instant (Phase 1) — à remplacer par
// une solution de state management (ex. Zustand) si la complexité le
// justifie une fois l'UI réelle en place.

import { useState } from "react";
import type { DictationStatus, Settings } from "../types";

export function useDictationStatus() {
  return useState<DictationStatus>("idle");
}

export function useSettingsState(initial?: Settings) {
  return useState<Settings | undefined>(initial);
}
