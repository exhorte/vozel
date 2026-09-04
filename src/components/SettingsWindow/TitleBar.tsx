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
import { useTranslation } from "@/lib/i18n";

interface TitleBarProps {
  collapsed: boolean;
  onToggleSidebar: () => void;
}

export function TitleBar({ collapsed, onToggleSidebar }: TitleBarProps) {
  const { t } = useTranslation();
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
        aria-label={collapsed ? t("titlebar.expand") : t("titlebar.collapse")}
        title={collapsed ? t("titlebar.expand") : t("titlebar.collapse")}
        onClick={onToggleSidebar}
      >
        <PanelLeft size={17} />
      </button>
      <button
        type="button"
        className="vz-iconbtn"
        aria-label={t("titlebar.account")}
        title={t("titlebar.account_short")}
        disabled
      >
        <CircleUser size={17} />
      </button>

      <div className="vz-titlebar__drag" data-tauri-drag-region />

      <button
        type="button"
        className="vz-iconbtn"
        aria-label={t("titlebar.notifications")}
        title={t("titlebar.notifications_short")}
        disabled
      >
        <Bell size={16} />
      </button>
      <button
        type="button"
        className="vz-iconbtn vz-iconbtn--win"
        aria-label={t("titlebar.minimize")}
        title={t("titlebar.minimize")}
        onClick={() => void win.minimize()}
      >
        <Minus size={16} />
      </button>
      <button
        type="button"
        className="vz-iconbtn vz-iconbtn--win"
        aria-label={maximized ? t("titlebar.restore") : t("titlebar.maximize")}
        title={maximized ? t("titlebar.restore") : t("titlebar.maximize")}
        onClick={() => void win.toggleMaximize()}
      >
        {maximized ? <Copy size={13} /> : <Square size={13} />}
      </button>
      <button
        type="button"
        className="vz-iconbtn vz-iconbtn--win vz-iconbtn--close"
        aria-label={t("titlebar.close")}
        title={t("titlebar.close")}
        onClick={() => void win.close()}
      >
        <X size={16} />
      </button>
    </div>
  );
}
