// État global minimal de l'app (statut de dictée courant, réglages en
// mémoire). Placeholder simple pour l'instant (Phase 1) — à remplacer par
// une solution de state management (ex. Zustand) si la complexité le
// justifie une fois l'UI réelle en place.

import { useEffect, useState } from "react";
import type { DictationStatus, Settings } from "../types";
import { listenDictationStatus } from "../lib/tauri";

export function useDictationStatus() {
  const [status, setStatus] = useState<DictationStatus>("idle");

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    listenDictationStatus(
      () => setStatus("listening"),
      () => setStatus("idle"),
    ).then((fn) => {
      if (cancelled) {
        fn();
      } else {
        unlisten = fn;
      }
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  return [status, setStatus] as const;
}

export function useSettingsState(initial?: Settings) {
  return useState<Settings | undefined>(initial);
}
