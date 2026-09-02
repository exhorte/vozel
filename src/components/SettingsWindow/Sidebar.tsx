// Barre latérale de la fenêtre Réglages, inspirée de Wispr Flow : liste de
// navigation icône + libellé, repliable en mode icônes seules.
//
// Périmètre (décidé avec l'utilisateur) : la nav reprend les 8 sections de
// Wispr Flow pour la fidélité visuelle, mais seules « Dictée » et
// « Dictionnaire » sont réelles (elles existent côté Vozel) — les autres
// sont des entrées désactivées « bientôt », sans destination. Les widgets
// SaaS de Wispr Flow (compte, équipe, parrainage, essai/upgrade) sont
// volontairement omis : Vozel est local-first, sans comptes ni facturation.

import { useEffect, useState } from "react";
import {
  AudioLines,
  BarChart3,
  BookText,
  Disc,
  HelpCircle,
  Home,
  Mic,
  NotebookPen,
  Scissors,
  Settings,
  Type,
  Wand2,
  type LucideIcon,
} from "lucide-react";
import { getVersion } from "@tauri-apps/api/app";

export type SettingsPage = "home" | "dictation" | "dictionary";

interface NavItem {
  id: string;
  label: string;
  icon: LucideIcon;
  page?: SettingsPage; // absent => pas encore implémenté
}

const TOP_ITEMS: NavItem[] = [
  { id: "home", label: "Accueil", icon: Home, page: "home" },
  { id: "dictation", label: "Dictée", icon: Mic, page: "dictation" },
  { id: "notetaker", label: "Prise de notes", icon: Disc },
  { id: "insights", label: "Statistiques", icon: BarChart3 },
  { id: "dictionary", label: "Dictionnaire", icon: BookText, page: "dictionary" },
  { id: "snippets", label: "Snippets", icon: Scissors },
  { id: "style", label: "Style", icon: Type },
  { id: "transforms", label: "Transformations", icon: Wand2 },
  { id: "scratchpad", label: "Bloc-notes", icon: NotebookPen },
];

const BOTTOM_ITEMS: NavItem[] = [
  { id: "settings", label: "Réglages avancés", icon: Settings },
  { id: "help", label: "Aide", icon: HelpCircle },
];

interface SidebarProps {
  collapsed: boolean;
  active: SettingsPage;
  onNavigate: (page: SettingsPage) => void;
}

function NavButton({
  item,
  active,
  collapsed,
  onNavigate,
}: {
  item: NavItem;
  active: SettingsPage;
  collapsed: boolean;
  onNavigate: (page: SettingsPage) => void;
}) {
  const Icon = item.icon;
  const disabled = !item.page;
  const isActive = item.page === active;
  return (
    <button
      type="button"
      className="vz-navitem"
      aria-current={isActive ? "page" : undefined}
      aria-disabled={disabled || undefined}
      title={collapsed ? item.label + (disabled ? " — bientôt" : "") : undefined}
      onClick={() => {
        if (item.page) onNavigate(item.page);
      }}
    >
      <Icon />
      <span className="vz-navitem__label">{item.label}</span>
      {disabled && !collapsed && <span className="vz-navitem__soon">bientôt</span>}
    </button>
  );
}

export function Sidebar({ collapsed, active, onNavigate }: SidebarProps) {
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
          />
        ))}
      </div>

      {version && <div className="vz-version">Vozel v{version}</div>}
    </nav>
  );
}
