// Fenêtre de réglages (Spec_Frontend.md Phase 1 §1.3 — layout général).
// Structure : un en-tête fixe, puis le contenu en sections séparées par un
// `Separator`, le tout dans une `ScrollArea` si la fenêtre est trop petite.
// `TooltipProvider` est monté ici une fois pour tous les tooltips explicatifs
// des panneaux enfants (§1.3 point 2).
//
// Panneaux : `ModelPanel` (Phase 1) ; `DictionaryPanel` reste un stub visuel
// jusqu'à la Phase 2 (CRUD dictionnaire relié à `storage::dictionary`).

import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import { TooltipProvider } from "@/components/ui/tooltip";
import { ModelPanel } from "./ModelPanel";
import { DictionaryPanel } from "./DictionaryPanel";

export function SettingsWindow() {
  return (
    <TooltipProvider delayDuration={200}>
      <div className="settings-window flex h-screen flex-col bg-background text-foreground">
        <header className="shrink-0 border-b px-6 py-4">
          <h1 className="text-lg font-semibold leading-none">Réglages</h1>
          <p className="mt-1 text-sm text-muted-foreground">
            Moteur de dictée, raccourci global et dictionnaire personnalisé.
          </p>
        </header>

        <ScrollArea className="flex-1">
          <div className="mx-auto flex max-w-2xl flex-col gap-8 p-6">
            <ModelPanel />
            <Separator />
            <DictionaryPanel />
          </div>
        </ScrollArea>
      </div>
    </TooltipProvider>
  );
}
