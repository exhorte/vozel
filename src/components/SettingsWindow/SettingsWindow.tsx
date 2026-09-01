// Fenêtre de réglages — coque façon Wispr Flow : barre de titre custom
// (`TitleBar`, la fenêtre `main` est en `decorations: false`) + barre
// latérale repliable (`Sidebar`) + page active.
//
// Pages réelles : « Dictée » (`ModelPanel` — moteur, raccourci, cloud,
// nettoyage IA, Command Mode, push-to-talk) et « Dictionnaire »
// (`DictionaryPanel`). Les autres entrées de la sidebar sont des
// placeholders « bientôt » (voir `Sidebar`).
//
// `TooltipProvider` est monté ici une fois pour les tooltips explicatifs des
// panneaux (§1.3 point 2). `Toaster` (sonner) pour les confirmations du
// `DictionaryPanel` (§2.1).

import { useEffect, useState } from "react";
import { ScrollArea } from "@/components/ui/scroll-area";
import { TooltipProvider } from "@/components/ui/tooltip";
import { Toaster } from "@/components/ui/sonner";
import { ModelPanel } from "./ModelPanel";
import { DictionaryPanel } from "./DictionaryPanel";
import { TitleBar } from "./TitleBar";
import { Sidebar, type SettingsPage } from "./Sidebar";

const COLLAPSE_KEY = "vozel:sidebar-collapsed";
const PAGE_TITLES: Record<SettingsPage, string> = {
  dictation: "Dictée",
  dictionary: "Dictionnaire",
};

function readCollapsed(): boolean {
  try {
    return localStorage.getItem(COLLAPSE_KEY) === "1";
  } catch {
    return false;
  }
}

export function SettingsWindow() {
  const [collapsed, setCollapsed] = useState(readCollapsed);
  const [page, setPage] = useState<SettingsPage>("dictation");

  useEffect(() => {
    try {
      localStorage.setItem(COLLAPSE_KEY, collapsed ? "1" : "0");
    } catch {
      /* stockage indisponible : préférence non persistée, pas bloquant */
    }
  }, [collapsed]);

  return (
    <TooltipProvider delayDuration={200}>
      <div className="app-shell">
        <TitleBar
          collapsed={collapsed}
          onToggleSidebar={() => setCollapsed((c) => !c)}
        />

        <div className="vz-shell-body">
          <Sidebar collapsed={collapsed} active={page} onNavigate={setPage} />

          <main className="vz-page">
            <header className="shrink-0 border-b border-black/10 px-8 py-4">
              <h1 className="text-lg font-semibold leading-none text-foreground">
                {PAGE_TITLES[page]}
              </h1>
            </header>
            <ScrollArea className="min-h-0 flex-1">
              <div className="mx-auto flex max-w-2xl flex-col gap-8 p-8">
                {page === "dictation" && <ModelPanel />}
                {page === "dictionary" && <DictionaryPanel />}
              </div>
            </ScrollArea>
          </main>
        </div>
      </div>
      <Toaster />
    </TooltipProvider>
  );
}
