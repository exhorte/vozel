// Widget flottant affiché pendant la dictée : indicateur d'écoute +
// visualiseur audio (retour visuel immédiat, cf. analyse section 4.2 —
// rassure sur la latence perçue). Phase 1.

import { AudioVisualizer } from "./AudioVisualizer";
import type { DictationStatus } from "../../types";

interface FloatingWidgetProps {
  status: DictationStatus;
}

export function FloatingWidget({ status }: FloatingWidgetProps) {
  return (
    <div className="floating-widget" data-status={status}>
      <AudioVisualizer active={status === "listening"} />
    </div>
  );
}
