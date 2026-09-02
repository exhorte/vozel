// Page Historique (Session 17, P2) — reprend l'esprit de la page History de
// Wispr Flow, adapté à Vozel : uniquement du texte (aucun audio n'est
// stocké — voir `commands::run_pipeline`), mono-utilisateur, local-first.
//
// NON reproduit (choix du prompt de reprise) : lecture audio / waveform,
// l'icône « ⋮ » ambiguë de la capture, l'état « Audio is silent » (le
// pipeline n'enregistre jamais une capture vide), toute action « reformuler ».

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Copy, MoreHorizontal, Search, Trash2 } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import {
  getSettings,
  historyClear,
  historyDelete,
  historyList,
  historySearch,
} from "@/lib/tauri";
import type { HistoryEntry } from "@/types";

const LIMIT = 100;
const PREVIEW_MAX = 220;
const SEARCH_DEBOUNCE_MS = 250;

// `created_at` est un UTC « YYYY-MM-DD HH:MM:SS » sans marqueur de fuseau —
// on le rend explicitement UTC avant de le passer à `Date` (sinon V8 le lit
// comme une heure locale).
function parseUtc(created_at: string): Date {
  return new Date(created_at.replace(" ", "T") + "Z");
}

function localTime(d: Date): string {
  return d.toLocaleTimeString("fr-FR", { hour: "2-digit", minute: "2-digit" });
}

function preview(text: string): string {
  const flat = text.replace(/\s+/g, " ").trim();
  return flat.length > PREVIEW_MAX
    ? flat.slice(0, PREVIEW_MAX).trimEnd() + "…"
    : flat;
}

function durationLabel(ms: number | null): string | null {
  if (ms == null) return null;
  const s = Math.round(ms / 1000);
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
}

// Regroupe par « Aujourd'hui » / « Hier » / « Cette semaine » (7 derniers
// jours), puis par date pour le plus ancien. Bornes en fuseau local, même
// logique que `historyStats`. Les entrées arrivent déjà triées récent→ancien,
// donc les groupes apparaissent naturellement dans le bon ordre.
function groupByDate(entries: HistoryEntry[]): Array<{ label: string; items: HistoryEntry[] }> {
  const now = new Date();
  const startToday = new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime();
  const startYesterday = startToday - 86_400_000;
  const startWeek = startToday - 6 * 86_400_000;

  const groups: Array<{ label: string; items: HistoryEntry[] }> = [];
  const byLabel = new Map<string, HistoryEntry[]>();
  for (const entry of entries) {
    const ts = parseUtc(entry.created_at).getTime();
    let label: string;
    if (ts >= startToday) label = "Aujourd'hui";
    else if (ts >= startYesterday) label = "Hier";
    else if (ts >= startWeek) label = "Cette semaine";
    else
      label = parseUtc(entry.created_at).toLocaleDateString("fr-FR", {
        day: "numeric",
        month: "long",
        year: "numeric",
      });
    let bucket = byLabel.get(label);
    if (!bucket) {
      bucket = [];
      byLabel.set(label, bucket);
      groups.push({ label, items: bucket });
    }
    bucket.push(entry);
  }
  return groups;
}

export function HistoryPage() {
  const [entries, setEntries] = useState<HistoryEntry[] | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [rawQuery, setRawQuery] = useState("");
  const [query, setQuery] = useState("");
  const [cloudEnabled, setCloudEnabled] = useState<boolean | null>(null);
  const [deleting, setDeleting] = useState<HistoryEntry | null>(null);
  const [clearOpen, setClearOpen] = useState(false);
  const [clearing, setClearing] = useState(false);
  const seq = useRef(0);

  // Debounce du champ de recherche.
  useEffect(() => {
    const t = setTimeout(() => setQuery(rawQuery.trim()), SEARCH_DEBOUNCE_MS);
    return () => clearTimeout(t);
  }, [rawQuery]);

  const reload = useCallback(async () => {
    const mine = ++seq.current;
    try {
      const list = query
        ? await historySearch(query, LIMIT)
        : await historyList(LIMIT);
      if (seq.current === mine) {
        setEntries(list);
        setLoadError(null);
      }
    } catch (e) {
      if (seq.current === mine) setLoadError(String(e));
    }
  }, [query]);

  useEffect(() => {
    void reload();
  }, [reload]);

  useEffect(() => {
    let cancelled = false;
    getSettings()
      .then((s) => {
        if (!cancelled) setCloudEnabled(s.cloud_enabled);
      })
      .catch(() => {
        /* le sous-titre reste au libellé neutre */
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const groups = useMemo(
    () => (entries ? groupByDate(entries) : []),
    [entries],
  );

  async function copy(text: string) {
    try {
      await navigator.clipboard.writeText(text);
      toast.success("Texte copié");
    } catch {
      toast.error("Copie impossible");
    }
  }

  async function confirmDelete() {
    if (!deleting) return;
    const target = deleting;
    setDeleting(null);
    try {
      await historyDelete(target.id);
      setEntries((prev) => prev?.filter((e) => e.id !== target.id) ?? prev);
      toast.success("Dictée supprimée");
    } catch (e) {
      toast.error(String(e));
    }
  }

  async function confirmClearAll() {
    setClearing(true);
    try {
      await historyClear();
      setEntries([]);
      setClearOpen(false);
      toast.success("Historique effacé");
    } catch (e) {
      toast.error(String(e));
    } finally {
      setClearing(false);
    }
  }

  const isSearching = query.length > 0;

  return (
    <section className="history-page flex flex-col gap-4">
      <div className="flex flex-col gap-1">
        <h2 className="text-xl font-semibold">Historique</h2>
        <p className="text-sm text-muted-foreground">
          {cloudEnabled
            ? "Vos dictées sont enregistrées localement sur cet appareil. Le mode cloud étant actif, l'audio est envoyé à un fournisseur tiers au moment de la transcription (le texte, lui, reste local)."
            : "Vos dictées sont enregistrées localement sur cet appareil et n'en sortent pas."}
        </p>
      </div>

      <div className="relative">
        <Search className="pointer-events-none absolute top-1/2 left-2.5 size-4 -translate-y-1/2 text-muted-foreground" />
        <Input
          value={rawQuery}
          onChange={(e) => setRawQuery(e.target.value)}
          placeholder="Rechercher dans l'historique…"
          className="h-9 pl-8"
          aria-label="Rechercher dans l'historique"
        />
      </div>

      {loadError ? (
        <p className="text-sm text-destructive">
          Impossible de charger l'historique&nbsp;: {loadError}
        </p>
      ) : entries === null ? (
        <p className="text-sm text-muted-foreground">Chargement…</p>
      ) : entries.length === 0 ? (
        <p className="rounded-md border border-dashed p-6 text-center text-sm text-muted-foreground">
          {isSearching
            ? `Aucune dictée ne contient « ${query} ».`
            : "Aucune dictée pour l'instant. Vos dictées apparaîtront ici au fil de leur enregistrement."}
        </p>
      ) : (
        <div className="flex flex-col gap-5">
          {groups.map((group) => (
            <div key={group.label} className="flex flex-col gap-1">
              <h3 className="px-1 text-xs font-medium uppercase tracking-wide text-muted-foreground">
                {group.label}
              </h3>
              <ul className="divide-y divide-border/60 rounded-md border">
                {group.items.map((entry) => {
                  const d = parseUtc(entry.created_at);
                  const dur = durationLabel(entry.duration_ms);
                  return (
                    <li
                      key={entry.id}
                      className="flex items-start gap-3 p-3"
                    >
                      <span className="mt-0.5 w-12 shrink-0 text-xs tabular-nums text-muted-foreground">
                        {localTime(d)}
                      </span>
                      <div className="min-w-0 flex-1">
                        <p className="text-sm text-foreground">{preview(entry.text)}</p>
                        <p className="mt-0.5 text-xs text-muted-foreground">
                          {entry.word_count} mot{entry.word_count > 1 ? "s" : ""}
                          {dur && ` · ${dur}`}
                        </p>
                      </div>
                      <div className="flex shrink-0 items-center gap-0.5">
                        <Button
                          variant="ghost"
                          size="icon"
                          className="size-8 text-muted-foreground"
                          aria-label="Copier le texte"
                          onClick={() => void copy(entry.text)}
                        >
                          <Copy className="size-4" />
                        </Button>
                        <DropdownMenu>
                          <DropdownMenuTrigger asChild>
                            <Button
                              variant="ghost"
                              size="icon"
                              className="size-8 text-muted-foreground"
                              aria-label="Autres actions"
                            >
                              <MoreHorizontal className="size-4" />
                            </Button>
                          </DropdownMenuTrigger>
                          <DropdownMenuContent align="end">
                            <DropdownMenuItem onSelect={() => void copy(entry.text)}>
                              Copier le texte
                            </DropdownMenuItem>
                            <DropdownMenuItem
                              variant="destructive"
                              onSelect={() => setDeleting(entry)}
                            >
                              Supprimer
                            </DropdownMenuItem>
                          </DropdownMenuContent>
                        </DropdownMenu>
                      </div>
                    </li>
                  );
                })}
              </ul>
            </div>
          ))}
        </div>
      )}

      <AlertDialog
        open={deleting !== null}
        onOpenChange={(open) => {
          if (!open) setDeleting(null);
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Supprimer cette dictée&nbsp;?</AlertDialogTitle>
            <AlertDialogDescription>
              Elle sera retirée définitivement de l'historique local. Cette
              action est irréversible.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Annuler</AlertDialogCancel>
            <AlertDialogAction
              onClick={confirmDelete}
              className="bg-destructive text-white hover:bg-destructive/90"
            >
              Supprimer
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>

      {entries !== null && entries.length > 0 && !isSearching && (
        <div className="flex justify-end pt-1">
          <Button
            variant="ghost"
            size="sm"
            className="text-muted-foreground hover:text-destructive"
            onClick={() => setClearOpen(true)}
          >
            <Trash2 className="size-4" />
            Effacer tout l'historique
          </Button>
        </div>
      )}

      <AlertDialog open={clearOpen} onOpenChange={setClearOpen}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Effacer tout l'historique&nbsp;?</AlertDialogTitle>
            <AlertDialogDescription>
              Toutes les dictées enregistrées localement seront supprimées
              définitivement, y compris leur texte. Cette action est
              irréversible.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Annuler</AlertDialogCancel>
            <AlertDialogAction
              onClick={confirmClearAll}
              disabled={clearing}
              className="bg-destructive text-white hover:bg-destructive/90"
            >
              Tout effacer
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </section>
  );
}
