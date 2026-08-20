// Widget flottant affiché pendant la dictée : indicateur d'état +
// visualiseur audio (retour visuel immédiat, cf. analyse section 4.2 —
// rassure sur la latence perçue). Phase 1.

import { Badge } from "@/components/ui/badge";
import { AudioVisualizer } from "./AudioVisualizer";
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

export function FloatingWidget({ status }: FloatingWidgetProps) {
  return (
    <div
      className="floating-widget flex items-center gap-2 rounded-full border bg-background/80 px-3 py-1.5 backdrop-blur"
      data-status={status}
    >
      <AudioVisualizer active={status === "listening"} />
      <Badge variant={STATUS_VARIANT[status]}>{STATUS_LABEL[status]}</Badge>
    </div>
  );
}
