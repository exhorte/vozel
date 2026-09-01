// Widget flottant (fenêtre Tauri `overlay`) — « flow bar » : 4 points dans
// une pastille gris clair translucide à bordure noire (voir
// `screenchot/flow bar.png`), positionnée en bas-centre de l'écran.
//
// États :
//  - repos (`idle`) / `error` : points immobiles (anneau rouge si erreur) ;
//  - `listening` : vibration en vague, amplitude modulée par le niveau audio
//    réel (`audio_level` du backend, Spec_Backend_Desktop.md §1.2) ;
//  - `processing` : pulsation lente (retour visuel « pipeline en cours »).
//
// Déclencheur UI manuel (hors numérotation Spec_Frontend.md — voir
// « Déviations signalées » dans PROGRESS.md) : la pastille est cliquable et
// appelle `startDictation`/`stopDictation` (`lib/tauri.ts`), les mêmes
// commandes IPC que le hotkey. Une seule source de vérité pour l'état : le
// clic déclenche le backend, qui émet les mêmes événements — voir
// `state/store.ts::useDictationStatus`.

import { useEffect, useState, type CSSProperties } from "react";
import { listenAudioLevel, startDictation, stopDictation } from "@/lib/tauri";
import type { DictationStatus } from "../../types";

const DOT_COUNT = 4;

interface FloatingWidgetProps {
  status: DictationStatus;
}

// "processing" : une dictée est déjà en cours de traitement, un nouveau clic
// serait ambigu — ignoré. "error" : un clic relance une tentative (= `idle`).
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
  const recording = status === "listening";
  const [level, setLevel] = useState(0);

  // Fond de document transparent + fenêtre repositionnée en bas-centre de
  // l'écran (uniquement cette fenêtre — `FloatingWidget` n'est monté que pour
  // le label `overlay`, voir `App.tsx`).
  useEffect(() => {
    document.documentElement.style.background = "transparent";
    document.body.style.background = "transparent";

    let cancelled = false;
    (async () => {
      try {
        const { getCurrentWindow, currentMonitor, PhysicalPosition } = await import(
          "@tauri-apps/api/window"
        );
        const win = getCurrentWindow();
        const mon = await currentMonitor();
        if (cancelled || !mon) return;
        const size = await win.outerSize();
        const marginBottom = Math.round(12 * mon.scaleFactor);
        const x = mon.position.x + Math.round((mon.size.width - size.width) / 2);
        const y = mon.position.y + mon.size.height - size.height - marginBottom;
        await win.setPosition(new PhysicalPosition(x, y));
      } catch (e) {
        console.error("[overlay] positionnement bas-centre échoué :", e);
      }
    })();

    return () => {
      cancelled = true;
    };
  }, []);

  // Niveau audio réel -> intensité de la vibration (variable CSS `--level`).
  useEffect(() => {
    if (!recording) {
      setLevel(0);
      return;
    }
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    listenAudioLevel((l) => setLevel(Math.min(1, Math.max(0, l)))).then((fn) => {
      if (cancelled) fn();
      else unlisten = fn;
    });
    return () => {
      cancelled = true;
      unlisten?.();
      setLevel(0);
    };
  }, [recording]);

  return (
    <div className="flow-bar-root">
      <div
        className="flow-bar"
        data-status={status}
        data-recording={recording}
        role="button"
        tabIndex={clickable ? 0 : -1}
        aria-disabled={!clickable}
        aria-label={recording ? "Arrêter la dictée" : "Démarrer la dictée"}
        style={{ "--level": level } as CSSProperties}
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
        {Array.from({ length: DOT_COUNT }, (_, i) => (
          <span
            key={i}
            className="flow-dot"
            style={{ animationDelay: `${i * 0.1}s` }}
          />
        ))}
      </div>
    </div>
  );
}
