// Page d'accueil (Session 16, pas de section de spec — prompt de reprise +
// 01_Recherche/Analyse_Fonctionnalites_WisprFlow_vs_Vozel.md §5). Nouvel
// écran de destination par défaut de la fenêtre Réglages, en tête de sidebar
// avant « Dictée ».
//
// Contenu, adapté au positionnement sans-compte de Vozel (explicitement
// AUCUNE brique SaaS de Wispr Flow : pas de série/streak présentée comme
// comparaison sociale, pas de nudge « essayez telle fonctionnalité », pas de
// quota/upsell) :
//   1. message de bienvenue simple ;
//   2. rappel du déclencheur de dictée (maintien de Ctrl + Win, figé) ;
//   3. statistiques locales légères : dictées + mots aujourd'hui et sur 7
//      jours, via l'agrégat `history_stats` ;
//   4. fil des dictées récentes (`history_list`), aperçu tronqué + date
//      relative, NON cliquable (aucune destination pertinente n'existe) ;
//   5. bouton « Effacer l'historique » discret, avec confirmation.
//
// État vide (premier lancement ou juste après un effacement) : ni crash ni
// « 0 » présenté comme un échec — un simple message à la place du fil et des
// compteurs.

import { useCallback, useEffect, useState } from "react";
import { Trash2 } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
} from "@/components/ui/alert-dialog";
import { historyClear, historyList, historyStats } from "@/lib/tauri";
import type { HistoryEntry, HistoryStatsPair } from "@/types";

const RECENT_LIMIT = 8;
const PREVIEW_MAX = 140;

// `created_at` est un UTC « YYYY-MM-DD HH:MM:SS » (sans marqueur de fuseau) :
// on le rend explicitement UTC avant de le passer à `Date`.
function parseUtc(created_at: string): number {
  return new Date(created_at.replace(" ", "T") + "Z").getTime();
}

function relativeTime(created_at: string): string {
  const then = parseUtc(created_at);
  if (Number.isNaN(then)) return "";
  const sec = Math.round((Date.now() - then) / 1000);
  if (sec < 45) return "à l'instant";
  const min = Math.round(sec / 60);
  if (min < 60) return `il y a ${min} min`;
  const hours = Math.round(min / 60);
  if (hours < 24) return `il y a ${hours} h`;
  const days = Math.round(hours / 24);
  if (days < 7) return `il y a ${days} j`;
  return new Date(then).toLocaleDateString("fr-FR", {
    day: "numeric",
    month: "short",
  });
}

function preview(text: string): string {
  const flat = text.replace(/\s+/g, " ").trim();
  return flat.length > PREVIEW_MAX ? flat.slice(0, PREVIEW_MAX).trimEnd() + "…" : flat;
}

function plural(n: number, singular: string, plural: string): string {
  return `${n} ${n > 1 ? plural : singular}`;
}

function StatBlock({ label, stats }: { label: string; stats: { count: number; word_count: number } }) {
  return (
    <div className="flex flex-col gap-0.5 rounded-md border border-border/60 bg-muted/30 p-3">
      <span className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
        {label}
      </span>
      <span className="text-2xl font-semibold tabular-nums">
        {plural(stats.count, "dictée", "dictées")}
      </span>
      <span className="text-sm text-muted-foreground">
        {plural(stats.word_count, "mot", "mots")}
      </span>
    </div>
  );
}

export function HomePage() {
  const [recent, setRecent] = useState<HistoryEntry[] | null>(null);
  const [stats, setStats] = useState<HistoryStatsPair | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [clearing, setClearing] = useState(false);

  const reloadHistory = useCallback(async () => {
    const now = new Date();
    const startOfToday = new Date(
      now.getFullYear(),
      now.getMonth(),
      now.getDate(),
    );
    const sevenDaysAgo = new Date(now.getTime() - 7 * 24 * 60 * 60 * 1000);
    try {
      const [entries, pair] = await Promise.all([
        historyList(RECENT_LIMIT),
        historyStats(startOfToday.toISOString(), sevenDaysAgo.toISOString()),
      ]);
      setRecent(entries);
      setStats(pair);
      setLoadError(null);
    } catch (e) {
      setLoadError(String(e));
    }
  }, []);

  useEffect(() => {
    void reloadHistory();
  }, [reloadHistory]);

  async function confirmClear() {
    setClearing(true);
    try {
      await historyClear();
      toast.success("Historique effacé");
      await reloadHistory();
    } catch (e) {
      toast.error(String(e));
    } finally {
      setClearing(false);
    }
  }

  const isEmpty = recent !== null && recent.length === 0;

  return (
    <section className="home-page flex flex-col gap-8">
      <div className="flex flex-col gap-1">
        <h2 className="text-xl font-semibold">Bienvenue dans Vozel</h2>
        <p className="text-sm text-muted-foreground">
          Dictée voix-vers-texte, 100&nbsp;% locale. Placez le curseur où vous
          voulez écrire, maintenez le raccourci, parlez&nbsp;: à la fin, le
          texte est inséré à l'endroit du curseur.
        </p>
      </div>

      <Card>
        <CardHeader>
          <CardTitle className="text-base">Déclencheur de dictée</CardTitle>
          <CardDescription>
            Maintenez les deux touches ensemble pendant que vous parlez,
            relâchez pour transcrire et insérer.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <kbd className="inline-flex items-center rounded-md border border-border bg-muted px-2.5 py-1 font-mono text-sm font-medium">
            Ctrl + Win
          </kbd>
          <p className="mt-2 text-sm text-muted-foreground">
            Raccourci unique et fixe. Note&nbsp;: relâcher la touche Windows en
            dernier peut ouvrir le menu Démarrer.
          </p>
        </CardContent>
      </Card>

      {loadError ? (
        <p className="text-sm text-destructive">
          Impossible de charger l'historique&nbsp;: {loadError}
        </p>
      ) : recent === null || stats === null ? (
        <p className="text-sm text-muted-foreground">Chargement…</p>
      ) : isEmpty ? (
        <Card>
          <CardHeader>
            <CardTitle className="text-base">Vos dictées</CardTitle>
          </CardHeader>
          <CardContent>
            <p className="rounded-md border border-dashed p-6 text-center text-sm text-muted-foreground">
              Aucune dictée pour l'instant. Vos dictées apparaîtront ici, avec un
              résumé de votre activité du jour et de la semaine.
            </p>
          </CardContent>
        </Card>
      ) : (
        <>
          <Card>
            <CardHeader>
              <CardTitle className="text-base">Votre activité</CardTitle>
              <CardDescription>
                Compté localement sur cette machine, jamais partagé.
              </CardDescription>
            </CardHeader>
            <CardContent className="grid grid-cols-2 gap-3">
              <StatBlock label="Aujourd'hui" stats={stats.today} />
              <StatBlock label="7 derniers jours" stats={stats.week} />
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle className="text-base">Dictées récentes</CardTitle>
            </CardHeader>
            <CardContent className="flex flex-col gap-3">
              <ul className="flex flex-col divide-y divide-border/60">
                {recent.map((entry) => (
                  <li key={entry.id} className="flex flex-col gap-1 py-3 first:pt-0 last:pb-0">
                    <p className="text-sm text-foreground">{preview(entry.text)}</p>
                    <p className="text-xs text-muted-foreground">
                      {relativeTime(entry.created_at)}
                      {" · "}
                      {plural(entry.word_count, "mot", "mots")}
                      {entry.duration_ms != null &&
                        ` · ${Math.max(1, Math.round(entry.duration_ms / 1000))} s`}
                    </p>
                  </li>
                ))}
              </ul>

              <div className="flex justify-end">
                <AlertDialog>
                  <AlertDialogTrigger asChild>
                    <Button
                      variant="ghost"
                      size="sm"
                      className="text-muted-foreground hover:text-destructive"
                    >
                      <Trash2 className="size-4" />
                      Effacer l'historique
                    </Button>
                  </AlertDialogTrigger>
                  <AlertDialogContent>
                    <AlertDialogHeader>
                      <AlertDialogTitle>Effacer tout l'historique&nbsp;?</AlertDialogTitle>
                      <AlertDialogDescription>
                        Toutes les dictées enregistrées localement seront
                        supprimées définitivement, y compris leur texte. Les
                        compteurs repartiront de zéro. Cette action est
                        irréversible.
                      </AlertDialogDescription>
                    </AlertDialogHeader>
                    <AlertDialogFooter>
                      <AlertDialogCancel>Annuler</AlertDialogCancel>
                      <AlertDialogAction
                        onClick={confirmClear}
                        disabled={clearing}
                        className="bg-destructive text-white hover:bg-destructive/90"
                      >
                        Effacer
                      </AlertDialogAction>
                    </AlertDialogFooter>
                  </AlertDialogContent>
                </AlertDialog>
              </div>
            </CardContent>
          </Card>
        </>
      )}
    </section>
  );
}
