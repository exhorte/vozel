// Widget flottant affiché pendant la dictée : indicateur d'état +
// visualiseur audio (retour visuel immédiat, cf. analyse section 4.2 —
// rassure sur la latence perçue). Phase 1.
//
// Déclencheur UI manuel (ajout demandé par l'utilisateur, hors numérotation
// Spec_Frontend.md — voir "Déviations signalées" dans PROGRESS.md) : le
// widget entier est cliquable et appelle `startDictation`/`stopDictation`
// (`lib/tauri.ts`), les mêmes commandes IPC que celles pilotées par le
// hotkey côté Rust (`commands::start_dictation`/`stop_dictation`,
// Spec_Backend_Desktop.md §1.6). Une seule source de vérité pour l'état :
// le clic ne met pas à jour `status` directement, il déclenche le backend,
// qui émet les mêmes événements que pour un appui clavier — voir
// `state/store.ts::useDictationStatus`.

import { Badge } from "@/components/ui/badge";
import { AudioVisualizer } from "./AudioVisualizer";
import { startDictation, stopDictation } from "@/lib/tauri";
import type { DictationStatus } from "../../types";

const STATUS_LABEL: Record<DictationStatus, string> = {
  idle: "Prêt",
  listening: "Écoute…",
  processing: "Traitement…",
  error: "Erreur",
};

const STATUS_VARIANT: Record<DictationStatus, "secondary" | "default" | "outline" | "destructive"> = {
  idle: "secondary",
  listening: "default",
  processing: "outline",
  error: "destructive",
};

interface FloatingWidgetProps {
  status: DictationStatus;
}

// "processing" : une dictée est déjà en cours de traitement, un nouveau
// clic serait ambigu (redémarrer ? attendre ?) — ignoré plutôt que de
// risquer un état incohérent. "error" : un clic relance une tentative
// (équivaut à `idle`).
async function handleToggle(status: DictationStatus) {
  try {
    if (status === "listening") {
      await stopDictation();
    } else if (status === "idle" || status === "error") {
      await startDictation();
    }
  } catch (e) {
    console.error("[dictation] échec du déclenchement manuel :", e);
  }
}

export function FloatingWidget({ status }: FloatingWidgetProps) {
  const clickable = status !== "processing";

  return (
    <div
      className="floating-widget flex items-center gap-2 rounded-full border bg-background/80 px-3 py-1.5 backdrop-blur select-none"
      data-status={status}
      role="button"
      tabIndex={clickable ? 0 : -1}
      aria-disabled={!clickable}
      aria-label={status === "listening" ? "Arrêter la dictée" : "Démarrer la dictée"}
      style={{ cursor: clickable ? "pointer" : "default" }}
      onClick={() => {
        if (clickable) void handleToggle(status);
      }}
      onKeyDown={(e) => {
        if (clickable && (e.key === "Enter" || e.key === " ")) {
          e.preventDefault();
          void handleToggle(status);
        }
      }}
    >
      <AudioVisualizer active={status === "listening"} />
      <Badge variant={STATUS_VARIANT[status]}>{STATUS_LABEL[status]}</Badge>
    </div>
  );
}
