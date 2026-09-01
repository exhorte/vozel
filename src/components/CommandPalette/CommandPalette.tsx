// Command Mode — palette de reformulation rapide d'une sélection
// (Spec_Frontend.md §2.2). Vit dans la fenêtre Tauri `command`, affichée par
// le backend sur appui du raccourci dédié après capture de la sélection
// courante. La palette elle-même ne pilote pas la fenêtre : elle appelle les
// commandes IPC `command_mode_*` / `run_command_mode` / `close_command_palette`
// (via `lib/tauri.ts`), le backend se charge de cacher la fenêtre et de rendre
// le focus à l'app d'origine.
//
// Composant shadcn : `command` (installé via `npx shadcn@latest add command`).
// Retour visuel pendant le traitement LLM (§2.2 point 2) : le modèle int4 local
// peut prendre ~10-40 s — un overlay « spinner » recouvre la liste pendant
// l'appel (pas de composant `Skeleton` installé, spinner CSS shadcn-compatible).

import { useCallback, useEffect, useState } from "react";
import {
  Command,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
} from "@/components/ui/command";
import {
  closeCommandPalette,
  commandModeContext,
  commandModeReformulations,
  listenCommandPaletteOpened,
  runCommandMode,
} from "@/lib/tauri";
import type { CommandModeContext, Reformulation } from "@/types";

const PREVIEW_MAX = 220;

export function CommandPalette() {
  const [reformulations, setReformulations] = useState<Reformulation[]>([]);
  const [context, setContext] = useState<CommandModeContext | null>(null);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refreshContext = useCallback(() => {
    setError(null);
    setBusyId(null);
    commandModeContext()
      .then(setContext)
      .catch((e) => setError(String(e)));
  }, []);

  useEffect(() => {
    let cancelled = false;
    commandModeReformulations()
      .then((list) => {
        if (!cancelled) setReformulations(list);
      })
      .catch((e) => {
        if (!cancelled) setError(String(e));
      });
    refreshContext();

    // La fenêtre est cachée/réaffichée (jamais démontée) : à chaque ouverture
    // le backend émet `command_palette_opened` avec la nouvelle sélection.
    const unlistenPromise = listenCommandPaletteOpened((selectedText) => {
      if (cancelled) return;
      setError(null);
      setBusyId(null);
      setContext((prev) => ({
        selected_text: selectedText,
        model_available: prev?.model_available ?? true,
      }));
      // Recharge aussi `model_available` (peu coûteux, pas de chargement modèle).
      commandModeContext()
        .then((c) => {
          if (!cancelled) setContext(c);
        })
        .catch(() => {});
    });

    return () => {
      cancelled = true;
      unlistenPromise.then((un) => un());
    };
  }, [refreshContext]);

  async function run(reformulationId: string) {
    if (busyId) return;
    setBusyId(reformulationId);
    setError(null);
    try {
      await runCommandMode(reformulationId);
      // Succès : le backend a déjà caché la fenêtre et collé le résultat.
      setBusyId(null);
    } catch (e) {
      // Échec (modèle absent, plus de sélection…) : la fenêtre reste ouverte.
      setBusyId(null);
      setError(String(e));
    }
  }

  function onKeyDown(event: React.KeyboardEvent) {
    if (event.key === "Escape") {
      event.preventDefault();
      closeCommandPalette().catch(() => {});
    }
  }

  const selectedText = context?.selected_text?.trim() ?? "";
  const hasSelection = selectedText.length > 0;
  const modelAvailable = context?.model_available ?? true;
  const preview =
    selectedText.length > PREVIEW_MAX
      ? `${selectedText.slice(0, PREVIEW_MAX)}…`
      : selectedText;
  const disabled = !hasSelection || !modelAvailable || busyId !== null;

  return (
    <div
      className="command-palette flex h-screen flex-col overflow-hidden rounded-xl border border-border bg-popover text-popover-foreground shadow-lg"
      onKeyDown={onKeyDown}
    >
      <div className="shrink-0 border-b border-border/60 px-4 py-3">
        <p className="text-sm font-medium">Reformuler la sélection</p>
        {hasSelection ? (
          <p className="mt-1 line-clamp-3 text-xs text-muted-foreground">{preview}</p>
        ) : (
          <p className="mt-1 text-xs text-destructive">
            Aucun texte sélectionné — sélectionnez du texte puis relancez le
            raccourci.
          </p>
        )}
        {!modelAvailable && (
          <p className="mt-1 text-xs text-destructive">
            Modèle LLM local introuvable dans <code>models\llm\</code> — le
            Command Mode a besoin du modèle pour reformuler.
          </p>
        )}
      </div>

      <div className="relative flex-1 overflow-hidden">
        <Command
          className="h-full bg-transparent"
          // Filtre par libellé (la valeur des items est le libellé).
        >
          <CommandInput placeholder="Filtrer les reformulations…" autoFocus />
          <CommandList className="max-h-none">
            <CommandEmpty>Aucune reformulation.</CommandEmpty>
            <CommandGroup>
              {reformulations.map((r) => (
                <CommandItem
                  key={r.id}
                  value={r.label}
                  disabled={disabled}
                  onSelect={() => run(r.id)}
                >
                  {r.label}
                </CommandItem>
              ))}
            </CommandGroup>
          </CommandList>
        </Command>

        {busyId !== null && (
          <div className="absolute inset-0 flex flex-col items-center justify-center gap-3 bg-popover/85 backdrop-blur-sm">
            <div
              className="size-6 animate-spin rounded-full border-2 border-muted-foreground/30 border-t-foreground"
              role="status"
              aria-label="Reformulation en cours"
            />
            <p className="text-xs text-muted-foreground">
              Reformulation en cours… (le modèle local peut prendre un moment)
            </p>
          </div>
        )}
      </div>

      {error && (
        <div className="shrink-0 border-t border-border/60 px-4 py-2 text-xs text-destructive">
          {error}
        </div>
      )}
    </div>
  );
}
