// Réglages en fenêtre modale (Session 17, P3) — façon Wispr Flow, adapté à
// Vozel. Ouvert depuis l'entrée « Réglages » de la sidebar (qui garde sa
// position en bas). `"settings"` n'est PAS une page routée : c'est une
// superposition.
//
// Deux sections réelles seulement — pas d'onglet creux pour coller
// visuellement à Wispr (System / Vibe coding / Experimental / Account / Team
// / Billing n'ont aucun équivalent local-first) :
//   - « Général » : le contenu de `ModelPanel` (moteur ASR local/cloud,
//     fournisseur + clé, nettoyage IA, rappel du déclencheur Ctrl+Win).
//   - « Données et confidentialité » : ce que Vozel stocke en local, nuancé
//     si le cloud est actif. Le bouton « Effacer tout l'historique » vit sur
//     la page Historique (là où l'historique est réellement consultable) —
//     pas dupliqué ici, seulement rappelé.

import { useEffect, useState } from "react";
import { ShieldCheck, SlidersHorizontal } from "lucide-react";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { getSettings } from "@/lib/tauri";
import { useTranslation } from "@/lib/i18n";
import { ModelPanel } from "./ModelPanel";

type Section = "general" | "privacy";

const SECTIONS: Array<{ id: Section; labelKey: string; icon: typeof SlidersHorizontal }> = [
  { id: "general", labelKey: "settingsModal.section_general", icon: SlidersHorizontal },
  { id: "privacy", labelKey: "settingsModal.section_privacy", icon: ShieldCheck },
];

interface SettingsModalProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function SettingsModal({ open, onOpenChange }: SettingsModalProps) {
  const { t } = useTranslation();
  const [section, setSection] = useState<Section>("general");

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="flex h-[85vh] max-w-3xl flex-col gap-0 p-0 sm:max-w-3xl">
        <DialogHeader className="border-b px-5 py-3">
          <DialogTitle className="text-base">{t("settingsModal.title")}</DialogTitle>
          <DialogDescription className="sr-only">{t("settingsModal.desc")}</DialogDescription>
        </DialogHeader>

        <div className="flex min-h-0 flex-1">
          <nav className="flex w-52 shrink-0 flex-col gap-0.5 border-r bg-muted/30 p-2">
            {SECTIONS.map((s) => {
              const Icon = s.icon;
              return (
                <button
                  key={s.id}
                  type="button"
                  onClick={() => setSection(s.id)}
                  aria-current={section === s.id ? "page" : undefined}
                  className="flex items-center gap-2 rounded-md px-2.5 py-1.5 text-left text-sm text-muted-foreground transition-colors hover:bg-accent hover:text-foreground aria-[current=page]:bg-accent aria-[current=page]:font-medium aria-[current=page]:text-foreground"
                >
                  <Icon className="size-4 shrink-0" />
                  <span>{t(s.labelKey)}</span>
                </button>
              );
            })}
          </nav>

          <div className="min-h-0 flex-1 overflow-y-auto p-6">
            {section === "general" && <ModelPanel />}
            {section === "privacy" && <PrivacySection />}
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}

function PrivacySection() {
  const { t } = useTranslation();
  const [cloudEnabled, setCloudEnabled] = useState<boolean | null>(null);

  useEffect(() => {
    let cancelled = false;
    getSettings()
      .then((s) => {
        if (!cancelled) setCloudEnabled(s.cloud_enabled);
      })
      .catch(() => {
        if (!cancelled) setCloudEnabled(null);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <section className="flex flex-col gap-4">
      <div className="flex flex-col gap-1">
        <h2 className="text-base font-semibold">{t("settingsModal.privacy_title")}</h2>
        <p className="text-sm text-muted-foreground">
          {t("settingsModal.privacy_body_base")}
          {cloudEnabled
            ? t("settingsModal.privacy_body_cloud")
            : t("settingsModal.privacy_body_no_cloud")}
        </p>
      </div>

      <div className="rounded-md border border-border/60 p-3 text-sm text-muted-foreground">
        {t("settingsModal.privacy_history_note_before")}
        <span className="font-medium text-foreground">
          {" "}
          {t("settingsModal.privacy_history_note_link")}
        </span>{" "}
        {t("settingsModal.privacy_history_note_after")}
      </div>
    </section>
  );
}
