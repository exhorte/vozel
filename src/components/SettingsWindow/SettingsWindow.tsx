// Fenêtre principale — coque façon Wispr Flow : barre de titre custom
// (`TitleBar`, la fenêtre `main` est en `decorations: false`) + barre latérale
// repliable (`Sidebar`) + page active.
//
// Session 17 — 3 pages routées : Accueil (`HomePage`), Dictionnaire
// (`DictionaryPanel`), Historique (`HistoryPage`). Les réglages ne sont plus
// une page mais une **fenêtre modale** (`SettingsModal`), ouverte depuis
// l'entrée « Réglages » de la sidebar (qui garde sa position en bas).
//
// `TooltipProvider` monté ici pour les tooltips explicatifs de `ModelPanel`
// (rendu dans la modale). `Toaster` (sonner) pour les confirmations.

import { useEffect, useState } from "react";
import { ScrollArea } from "@/components/ui/scroll-area";
import { TooltipProvider } from "@/components/ui/tooltip";
import { Toaster } from "@/components/ui/sonner";
import { LanguageProvider, useTranslation } from "@/lib/i18n";
import { HomePage } from "./HomePage";
import { DictionaryPanel } from "./DictionaryPanel";
import { HistoryPage } from "./HistoryPage";
import { SettingsModal } from "./SettingsModal";
import { TitleBar } from "./TitleBar";
import { Sidebar, type SettingsPage } from "./Sidebar";

const COLLAPSE_KEY = "vozel:sidebar-collapsed";

function readCollapsed(): boolean {
  try {
    return localStorage.getItem(COLLAPSE_KEY) === "1";
  } catch {
    return false;
  }
}

function SettingsWindowInner() {
  const { t } = useTranslation();
  const [collapsed, setCollapsed] = useState(readCollapsed);
  const [page, setPage] = useState<SettingsPage>("home");
  const [settingsOpen, setSettingsOpen] = useState(false);

  useEffect(() => {
    try {
      localStorage.setItem(COLLAPSE_KEY, collapsed ? "1" : "0");
    } catch {
      /* stockage indisponible : préférence non persistée, pas bloquant */
    }
  }, [collapsed]);

  const pageTitles: Record<SettingsPage, string> = {
    home: t("page.home"),
    dictionary: t("page.dictionary"),
    history: t("page.history"),
  };

  return (
    <TooltipProvider delayDuration={200}>
      <div className="app-shell">
        <TitleBar
          collapsed={collapsed}
          onToggleSidebar={() => setCollapsed((c) => !c)}
        />

        <div className="vz-shell-body">
          <Sidebar
            collapsed={collapsed}
            active={page}
            onNavigate={setPage}
            onOpenSettings={() => setSettingsOpen(true)}
          />

          <main className="vz-page">
            <header className="shrink-0 border-b border-black/10 px-8 py-4">
              <h1 className="text-lg font-semibold leading-none text-foreground">
                {pageTitles[page]}
              </h1>
            </header>
            <ScrollArea className="min-h-0 flex-1">
              <div className="mx-auto flex max-w-2xl flex-col gap-8 p-8">
                {page === "home" && <HomePage onNavigate={setPage} />}
                {page === "dictionary" && <DictionaryPanel />}
                {page === "history" && <HistoryPage />}
              </div>
            </ScrollArea>
          </main>
        </div>
      </div>

      <SettingsModal open={settingsOpen} onOpenChange={setSettingsOpen} />
      <Toaster />
    </TooltipProvider>
  );
}

// `LanguageProvider` doit englober tout l'arbre de la fenêtre `main` — d'où
// ce petit composant hôte : `useTranslation()` (utilisé plus haut) ne peut
// pas être appelé dans le composant qui monte son propre `Provider`.
export function SettingsWindow() {
  return (
    <LanguageProvider>
      <SettingsWindowInner />
    </LanguageProvider>
  );
}
