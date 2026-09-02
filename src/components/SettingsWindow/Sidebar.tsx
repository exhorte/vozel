// Barre latérale de la fenêtre principale, façon Wispr Flow.
//
// Session 17 — sidebar finale : 4 destinations seulement.
//   Groupe du haut : Accueil, Dictionnaire, Historique (ordre observé chez
//   Wispr : Home / Dictionary / History).
//   Groupe du bas : Réglages — qui n'est plus une page mais **ouvre une
//   fenêtre modale** (`SettingsModal`). Il garde sa position en bas (choix
//   explicite de l'utilisateur, volontairement différent du regroupement en
//   haut chez Wispr).
//
// Supprimés en Session 17 (pas de placeholder « bientôt ») : Dictée (son
// contenu vit désormais dans la modale Réglages), Prise de notes,
// Statistiques, Snippets, Style, Transformations, Bloc-notes, Aide.

import { useEffect, useState } from "react";
import {
  AudioLines,
  BookText,
  History,
  Home,
  Settings,
  type LucideIcon,
} from "lucide-react";
import { getVersion } from "@tauri-apps/api/app";

export type SettingsPage = "home" | "dictionary" | "history";

interface NavItem {
  id: string;
  label: string;
  icon: LucideIcon;
  /** Page routée dans la fenêtre principale. */
  page?: SettingsPage;
  /** Superposition (modale) plutôt qu'une page — `"settings"` n'est
   *  volontairement PAS une valeur de `SettingsPage`. */
  overlay?: "settings";
}

const TOP_ITEMS: NavItem[] = [
  { id: "home", label: "Accueil", icon: Home, page: "home" },
  { id: "dictionary", label: "Dictionnaire", icon: BookText, page: "dictionary" },
  { id: "history", label: "Historique", icon: History, page: "history" },
];

const BOTTOM_ITEMS: NavItem[] = [
  { id: "settings", label: "Réglages", icon: Settings, overlay: "settings" },
];

interface SidebarProps {
  collapsed: boolean;
  active: SettingsPage;
  onNavigate: (page: SettingsPage) => void;
  onOpenSettings: () => void;
}

function NavButton({
  item,
  active,
  collapsed,
  onNavigate,
  onOpenSettings,
}: {
  item: NavItem;
  active: SettingsPage;
  collapsed: boolean;
  onNavigate: (page: SettingsPage) => void;
  onOpenSettings: () => void;
}) {
  const Icon = item.icon;
  const isActive = item.page !== undefined && item.page === active;
  return (
    <button
      type="button"
      className="vz-navitem"
      aria-current={isActive ? "page" : undefined}
      title={collapsed ? item.label : undefined}
      onClick={() => {
        if (item.overlay === "settings") onOpenSettings();
        else if (item.page) onNavigate(item.page);
      }}
    >
      <Icon />
      <span className="vz-navitem__label">{item.label}</span>
    </button>
  );
}

export function Sidebar({
  collapsed,
  active,
  onNavigate,
  onOpenSettings,
}: SidebarProps) {
  const [version, setVersion] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    getVersion()
      .then((v) => {
        if (!cancelled) setVersion(v);
      })
      .catch(() => {
        /* pas bloquant */
      });
    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <nav className="vz-sidebar" data-collapsed={collapsed}>
      <div className="vz-brand">
        <AudioLines size={19} />
        {!collapsed && <span>Vozel</span>}
      </div>

      <div className="vz-navgroup">
        {TOP_ITEMS.map((item) => (
          <NavButton
            key={item.id}
            item={item}
            active={active}
            collapsed={collapsed}
            onNavigate={onNavigate}
            onOpenSettings={onOpenSettings}
          />
        ))}
      </div>

      <div className="vz-navspacer" />

      <div className="vz-navsep" />
      <div className="vz-navgroup">
        {BOTTOM_ITEMS.map((item) => (
          <NavButton
            key={item.id}
            item={item}
            active={active}
            collapsed={collapsed}
            onNavigate={onNavigate}
            onOpenSettings={onOpenSettings}
          />
        ))}
      </div>

      {version && <div className="vz-version">Vozel v{version}</div>}
    </nav>
  );
}
