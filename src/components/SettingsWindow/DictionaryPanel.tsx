// Dictionnaire personnalisé (Spec_Frontend.md §2.1) : liste + CRUD des
// entrées de remplacement, connecté aux commandes `dict_*` du backend
// (`storage::dictionary`, Spec_Backend_Desktop.md §2.2).
//
// Session 17 (P4) : **restylage visuel seulement** façon Wispr Flow (carte
// d'intro dismissible + pills d'exemples + liste en dessous). La logique et
// le modèle de données sont inchangés — toujours des paires `from`/`to`
// appliquées en post-traitement texte. Vozel n'a **pas** de mode « mot
// isolé » pour biaiser le moteur ASR (Parakeet local / fournisseurs cloud) :
// aucun mécanisme de ce type n'existe, et le simuler par une paire
// `from == to` laisserait croire à une fonctionnalité de boost inexistante.
// L'idée part en recherche (`01_Recherche/`), pas ici.
//
// Écart signalé (PROGRESS.md) : le formulaire d'ajout/édition est un
// formulaire contrôlé simple plutôt que le trio `react-hook-form` + `zod` +
// shadcn `form` "recommandé" par la spec — deux champs texte, validation
// triviale (non vides), l'unicité de `from` est vérifiée côté backend qui
// renvoie un message clair. Trois dépendances pour ça ne se justifient pas.

import { useCallback, useEffect, useMemo, useState } from "react";
import { MoreHorizontal, Plus, X } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
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
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import {
  dictCreate,
  dictDelete,
  dictList,
  dictUpdate,
} from "@/lib/tauri";
import type { DictionaryEntry } from "@/types";

type SortKey = "from" | "to";

// `null` = fermé ; sans `id` = ajout ; avec `id` = édition.
interface DraftEntry {
  id?: number;
  from: string;
  to: string;
}

// Exemples purement illustratifs de la carte d'intro — de vraies paires
// `from → to` (jamais des « mots isolés », voir en-tête de fichier).
const EXAMPLE_PAIRS: Array<[string, string]> = [
  ["type script", "TypeScript"],
  ["parak it", "Parakeet"],
  ["e xponent value", "ExponentValue"],
  ["get up", "GitHub"],
];

const INTRO_DISMISSED_KEY = "vozel:dict-intro-dismissed";

function readIntroDismissed(): boolean {
  try {
    return localStorage.getItem(INTRO_DISMISSED_KEY) === "1";
  } catch {
    return false;
  }
}

export function DictionaryPanel() {
  const [entries, setEntries] = useState<DictionaryEntry[] | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [filter, setFilter] = useState("");
  const [sortKey, setSortKey] = useState<SortKey>("from");
  const [sortAsc, setSortAsc] = useState(true);
  const [draft, setDraft] = useState<DraftEntry | null>(null);
  const [saving, setSaving] = useState(false);
  const [deleting, setDeleting] = useState<DictionaryEntry | null>(null);
  const [introDismissed, setIntroDismissed] = useState(readIntroDismissed);

  function dismissIntro() {
    setIntroDismissed(true);
    try {
      localStorage.setItem(INTRO_DISMISSED_KEY, "1");
    } catch {
      /* préférence non persistée, pas bloquant */
    }
  }

  const reload = useCallback(async () => {
    try {
      setEntries(await dictList());
      setLoadError(null);
    } catch (e) {
      setLoadError(String(e));
    }
  }, []);

  useEffect(() => {
    void reload();
  }, [reload]);

  const visible = useMemo(() => {
    if (!entries) return [];
    const needle = filter.trim().toLowerCase();
    const filtered = needle
      ? entries.filter(
          (e) =>
            e.from.toLowerCase().includes(needle) ||
            e.to.toLowerCase().includes(needle),
        )
      : entries;
    return [...filtered].sort((a, b) => {
      const cmp = a[sortKey].localeCompare(b[sortKey], "fr", {
        sensitivity: "base",
      });
      return sortAsc ? cmp : -cmp;
    });
  }, [entries, filter, sortKey, sortAsc]);

  function toggleSort(key: SortKey) {
    if (key === sortKey) {
      setSortAsc((v) => !v);
    } else {
      setSortKey(key);
      setSortAsc(true);
    }
  }

  async function submitDraft() {
    if (!draft) return;
    const from = draft.from.trim();
    const to = draft.to.trim();
    if (!from || !to) return;
    setSaving(true);
    try {
      if (draft.id === undefined) {
        const created = await dictCreate(from, to);
        toast.success(`« ${created.from} » ajouté au dictionnaire`);
      } else {
        await dictUpdate(draft.id, from, to);
        toast.success("Entrée mise à jour");
      }
      setDraft(null);
      await reload();
    } catch (e) {
      toast.error(String(e));
    } finally {
      setSaving(false);
    }
  }

  async function confirmDelete() {
    if (!deleting) return;
    const target = deleting;
    setDeleting(null);
    try {
      await dictDelete(target.id);
      toast.success(`« ${target.from} » supprimé`);
      await reload();
    } catch (e) {
      toast.error(String(e));
    }
  }

  return (
    <section className="dictionary-panel flex flex-col gap-4">
      {introDismissed ? (
        <div className="flex flex-col gap-1">
          <h2 className="text-base font-semibold">Dictionnaire personnalisé</h2>
          <p className="text-sm text-muted-foreground">
            Remplacements appliqués après la transcription, avant le nettoyage —
            pour les noms propres et le jargon.
          </p>
        </div>
      ) : (
        <Card className="relative">
          <button
            type="button"
            onClick={dismissIntro}
            aria-label="Masquer cette carte"
            className="absolute top-2 right-2 rounded-md p-1 text-muted-foreground transition-colors hover:text-foreground"
          >
            <X className="size-4" />
          </button>
          <CardContent className="flex flex-col gap-3 pr-8">
            <h2 className="text-lg font-semibold tracking-tight">
              Vozel écrit les mots comme vous les dites.
            </h2>
            <p className="text-sm text-muted-foreground">
              Quand une transcription se trompe régulièrement sur un nom propre
              ou un terme métier, ajoutez une correction&nbsp;: le texte reconnu
              (à gauche) est remplacé par la forme voulue (à droite) dans chaque
              dictée, avant le nettoyage. Insensible à la casse et aux espaces.
            </p>
            <div className="flex flex-wrap gap-1.5">
              {EXAMPLE_PAIRS.map(([from, to]) => (
                <span
                  key={from}
                  className="inline-flex items-center gap-1.5 rounded-full border bg-background px-2.5 py-1 text-xs"
                >
                  <span className="text-muted-foreground line-through">{from}</span>
                  <span aria-hidden>→</span>
                  <span className="font-medium">{to}</span>
                </span>
              ))}
            </div>
            <div>
              <Button size="sm" onClick={() => setDraft({ from: "", to: "" })}>
                <Plus className="size-4" />
                Ajouter une correction
              </Button>
            </div>
          </CardContent>
        </Card>
      )}

      <div className="flex items-center gap-2">
        <Input
          placeholder="Filtrer…"
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
          className="h-9 max-w-xs"
          aria-label="Filtrer les entrées du dictionnaire"
        />
        <Button
          size="sm"
          className="ml-auto"
          onClick={() => setDraft({ from: "", to: "" })}
        >
          <Plus className="size-4" />
          Ajouter
        </Button>
      </div>

      {loadError ? (
        <p className="text-sm text-destructive">
          Impossible de charger le dictionnaire : {loadError}
        </p>
      ) : entries === null ? (
        <p className="text-sm text-muted-foreground">Chargement…</p>
      ) : entries.length === 0 ? (
        <p className="rounded-md border border-dashed p-6 text-center text-sm text-muted-foreground">
          Aucune entrée. Ajoutez-en une pour corriger automatiquement un mot
          mal transcrit.
        </p>
      ) : (
        <div className="overflow-x-auto rounded-md border">
          <Table>
            <TableHeader>
              <TableRow>
                <SortableHead
                  label="Reconnu"
                  active={sortKey === "from"}
                  asc={sortAsc}
                  onClick={() => toggleSort("from")}
                />
                <SortableHead
                  label="Corrigé en"
                  active={sortKey === "to"}
                  asc={sortAsc}
                  onClick={() => toggleSort("to")}
                />
                <TableHead className="w-10" />
              </TableRow>
            </TableHeader>
            <TableBody>
              {visible.map((entry) => (
                <TableRow key={entry.id}>
                  <TableCell className="font-medium">{entry.from}</TableCell>
                  <TableCell>{entry.to}</TableCell>
                  <TableCell className="text-right">
                    <DropdownMenu>
                      <DropdownMenuTrigger asChild>
                        <Button
                          variant="ghost"
                          size="icon"
                          className="size-8"
                          aria-label={`Actions pour « ${entry.from} »`}
                        >
                          <MoreHorizontal className="size-4" />
                        </Button>
                      </DropdownMenuTrigger>
                      <DropdownMenuContent align="end">
                        <DropdownMenuItem
                          onSelect={() =>
                            setDraft({
                              id: entry.id,
                              from: entry.from,
                              to: entry.to,
                            })
                          }
                        >
                          Modifier
                        </DropdownMenuItem>
                        <DropdownMenuItem
                          variant="destructive"
                          onSelect={() => setDeleting(entry)}
                        >
                          Supprimer
                        </DropdownMenuItem>
                      </DropdownMenuContent>
                    </DropdownMenu>
                  </TableCell>
                </TableRow>
              ))}
              {visible.length === 0 && (
                <TableRow>
                  <TableCell
                    colSpan={3}
                    className="text-center text-sm text-muted-foreground"
                  >
                    Aucune entrée ne correspond à « {filter} ».
                  </TableCell>
                </TableRow>
              )}
            </TableBody>
          </Table>
        </div>
      )}

      {/* Ajout / édition */}
      <Dialog
        open={draft !== null}
        onOpenChange={(open) => {
          if (!open) setDraft(null);
        }}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>
              {draft?.id === undefined ? "Ajouter une entrée" : "Modifier l'entrée"}
            </DialogTitle>
            <DialogDescription>
              Le texte « reconnu » est remplacé par le texte « corrigé » dans
              chaque dictée (insensible à la casse et aux espaces).
            </DialogDescription>
          </DialogHeader>
          <form
            className="flex flex-col gap-4"
            onSubmit={(e) => {
              e.preventDefault();
              void submitDraft();
            }}
          >
            <div className="flex flex-col gap-2">
              <Label htmlFor="dict-from">Reconnu</Label>
              <Input
                id="dict-from"
                autoFocus
                value={draft?.from ?? ""}
                onChange={(e) =>
                  setDraft((d) => (d ? { ...d, from: e.target.value } : d))
                }
                placeholder="type script"
              />
            </div>
            <div className="flex flex-col gap-2">
              <Label htmlFor="dict-to">Corrigé en</Label>
              <Input
                id="dict-to"
                value={draft?.to ?? ""}
                onChange={(e) =>
                  setDraft((d) => (d ? { ...d, to: e.target.value } : d))
                }
                placeholder="TypeScript"
              />
            </div>
            <DialogFooter>
              <Button
                type="button"
                variant="outline"
                onClick={() => setDraft(null)}
              >
                Annuler
              </Button>
              <Button
                type="submit"
                disabled={
                  saving ||
                  !draft?.from.trim() ||
                  !draft?.to.trim()
                }
              >
                {draft?.id === undefined ? "Ajouter" : "Enregistrer"}
              </Button>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>

      {/* Suppression */}
      <AlertDialog
        open={deleting !== null}
        onOpenChange={(open) => {
          if (!open) setDeleting(null);
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Supprimer cette entrée ?</AlertDialogTitle>
            <AlertDialogDescription>
              « {deleting?.from} » → « {deleting?.to} » ne sera plus appliqué
              aux dictées. Cette action est définitive.
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
    </section>
  );
}

function SortableHead({
  label,
  active,
  asc,
  onClick,
}: {
  label: string;
  active: boolean;
  asc: boolean;
  onClick: () => void;
}) {
  return (
    <TableHead>
      <button
        type="button"
        onClick={onClick}
        className="inline-flex items-center gap-1 font-medium hover:text-foreground"
        aria-sort={active ? (asc ? "ascending" : "descending") : "none"}
      >
        {label}
        <span className="text-muted-foreground">
          {active ? (asc ? "▲" : "▼") : ""}
        </span>
      </button>
    </TableHead>
  );
}
