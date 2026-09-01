// Barre de titre custom de la fenêtre `main` (Réglages).
//
// La fenêtre `main` est passée en `decorations: false` (tauri.conf.json) pour
// reproduire la chrome de Wispr Flow : à gauche l'icône de repli de la
// sidebar + l'icône de compte ; à droite la cloche de notifications puis les
// contrôles de fenêtre (réduire / agrandir-restaurer / fermer), tous
// dessinés ici. La zone centrale est une région de glissement Tauri
// (`data-tauri-drag-region`) pour déplacer la fenêtre.
//
// Compte + cloche : placeholders visuels (Vozel n'a ni comptes ni
// notifications) — désactivés, présents pour la fidélité au modèle.

import { useEffect, useState } from "react";
import {
  Bell,
  CircleUser,
  Copy,
  Minus,
  PanelLeft,
  Square,
  X,
} from "lucide-react";
import { getCurrentWindow } from "@tauri-apps/api/window";

interface TitleBarProps {
  collapsed: boolean;
  onToggleSidebar: () => void;
}

export function TitleBar({ collapsed, onToggleSidebar }: TitleBarProps) {
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    const win = getCurrentWindow();
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    win.isMaximized().then((m) => {
      if (!cancelled) setMaximized(m);
    });
    win
      .onResized(() => {
        win.isMaximized().then((m) => {
          if (!cancelled) setMaximized(m);
        });
      })
      .then((fn) => {
        if (cancelled) fn();
        else unlisten = fn;
      });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  const win = getCurrentWindow();

  return (
    <div className="vz-titlebar">
      <button
        type="button"
        className="vz-iconbtn"
        aria-label={collapsed ? "Déplier la barre latérale" : "Replier la barre latérale"}
        title={collapsed ? "Déplier la barre latérale" : "Replier la barre latérale"}
        onClick={onToggleSidebar}
      >
        <PanelLeft size={17} />
      </button>
      <button
        type="button"
        className="vz-iconbtn"
        aria-label="Compte (bientôt)"
        title="Compte — bientôt"
        disabled
      >
        <CircleUser size={17} />
      </button>

      <div className="vz-titlebar__drag" data-tauri-drag-region />

      <button
        type="button"
        className="vz-iconbtn"
        aria-label="Notifications (bientôt)"
        title="Notifications — bientôt"
        disabled
      >
        <Bell size={16} />
      </button>
      <button
        type="button"
        className="vz-iconbtn vz-iconbtn--win"
        aria-label="Réduire"
        title="Réduire"
        onClick={() => void win.minimize()}
      >
        <Minus size={16} />
      </button>
      <button
        type="button"
        className="vz-iconbtn vz-iconbtn--win"
        aria-label={maximized ? "Restaurer" : "Agrandir"}
        title={maximized ? "Restaurer" : "Agrandir"}
        onClick={() => void win.toggleMaximize()}
      >
        {maximized ? <Copy size={13} /> : <Square size={13} />}
      </button>
      <button
        type="button"
        className="vz-iconbtn vz-iconbtn--win vz-iconbtn--close"
        aria-label="Fermer"
        title="Fermer"
        onClick={() => void win.close()}
      >
        <X size={16} />
      </button>
    </div>
  );
}
