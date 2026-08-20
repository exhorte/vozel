// Visualiseur audio temps réel : bandes de fréquence pilotées par le niveau
// d'amplitude émis par le backend pendant la capture (`audio_level`, Rust
// `audio::capture`, Spec_Backend_Desktop.md §1.2 — pic RMS par frame 16 kHz,
// throttlé à 20 Hz côté Rust).

import { useEffect, useState } from "react";
import { listenAudioLevel } from "../../lib/tauri";

const BAR_COUNT = 5;

interface AudioVisualizerProps {
  active: boolean;
}

export function AudioVisualizer({ active }: AudioVisualizerProps) {
  const [level, setLevel] = useState(0);

  useEffect(() => {
    if (!active) {
      setLevel(0);
      return;
    }
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    listenAudioLevel(setLevel).then((fn) => {
      if (cancelled) {
        fn();
      } else {
        unlisten = fn;
      }
    });
    return () => {
      cancelled = true;
      unlisten?.();
      setLevel(0);
    };
  }, [active]);

  return (
    <div className="audio-visualizer flex h-4 items-end gap-0.5" data-active={active}>
      {Array.from({ length: BAR_COUNT }, (_, i) => {
        // Déphasage simple par bande pour un effet moins uniforme qu'un
        // seul niveau répété — à affiner une fois un vrai flux multi-bandes
        // disponible côté backend (VAD/FFT plutôt qu'un niveau RMS unique).
        const barLevel = active ? Math.min(1, level * (0.6 + 0.4 * Math.sin(i + 1))) : 0;
        return (
          <span
            key={i}
            className="w-1 rounded-full bg-current transition-[height] duration-75"
            style={{ height: `${4 + barLevel * 12}px` }}
          />
        );
      })}
    </div>
  );
}
