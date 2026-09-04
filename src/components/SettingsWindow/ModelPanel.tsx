// Section « Général » de la fenêtre modale Réglages (Session 17 — ce n'est
// plus une page routée de la sidebar). Contenu : moteur ASR local (§1.2) ;
// — quand le cloud est activé — fournisseur cloud + clé API + avertissement
// confidentialité (§2.3) ; switch « nettoyage IA local » (§2.4). Toute
// modification appelle `saveSettings` immédiatement, persisté en SQLite côté
// backend (`src-tauri/src/storage/settings.rs`, Spec_Backend_Desktop.md §2.1).
//
// Le déclenchement de la dictée n'est plus un réglage : maintien de Ctrl+Win,
// câblé en dur (`hotkey::modifier_combo`, demande utilisateur 2026-09-02) —
// ne reste ici qu'un rappel statique.
//
// Layout (§1.3) : groupes de réglages séparés par un `Separator` ; les
// options techniques portent un `Tooltip` explicatif. La bascule local/cloud
// est entièrement pilotable ici, sans redémarrage (l'aiguilleur backend
// `asr::RoutingAsrEngine` relit les réglages à chaque dictée — §2.4).

import { useEffect, useState } from "react";
import { Info, TriangleAlert } from "lucide-react";
import {
  Alert,
  AlertDescription,
  AlertTitle,
} from "@/components/ui/alert";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Combobox, type ComboboxOption } from "@/components/ui/combobox";
import { Separator } from "@/components/ui/separator";
import { Switch } from "@/components/ui/switch";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { Input } from "@/components/ui/input";
import { getSettings, llmModelAvailable, saveSettings } from "@/lib/tauri";
import { useTranslation, type Lang } from "@/lib/i18n";
import type { Settings } from "@/types";

// Fournisseurs ASR cloud, tous branchés côté backend (`asr::cloud`,
// Spec_Backend_Desktop.md §2.4 + §2.4.3 : Groq, OpenAI, Deepgram). Affichés
// dans un `Combobox` shadcn (`command` + `popover`) conformément à
// Spec_Frontend.md §2.3 — le `Select` précédent (avec OpenAI/Deepgram
// désactivés) datait d'avant l'implémentation des deux autres fournisseurs.
// Noms de marque + modèle : pas traduits (identiques en anglais).
const CLOUD_PROVIDERS: ComboboxOption[] = [
  { value: "groq", label: "Groq — Whisper large v3 turbo" },
  { value: "openai", label: "OpenAI — Whisper-1" },
  { value: "deepgram", label: "Deepgram — Nova-2" },
];

// Moteurs ASR locaux connus. `whisper-cpp` reste désactivé dans le
// sélecteur : le benchmark français (voir PROGRESS.md, Session 4) a retenu
// Parakeet-TDT ONNX INT8 par défaut pour `asr::local` (Spec_Backend_Desktop.md
// §1.3, ~4-5x plus rapide et WER plus bas sur l'échantillon testé) ;
// whisper.cpp reste un module stub documenté, pas encore branché.
const ASR_ENGINES: Array<{
  value: string;
  labelKey: string;
  descriptionKey: string;
  disabled?: boolean;
}> = [
  {
    value: "parakeet-tdt",
    labelKey: "model.asr_parakeet_label",
    descriptionKey: "model.asr_parakeet_desc",
  },
  {
    value: "whisper-cpp",
    labelKey: "model.asr_whisper_cpp_label",
    descriptionKey: "model.asr_whisper_cpp_desc",
    disabled: true,
  },
];

// Libellé de champ avec, en option, une icône déclenchant un `Tooltip`
// explicatif (§1.3 point 2 — options techniques).
function FieldLabel({
  htmlFor,
  children,
  tooltip,
}: {
  htmlFor?: string;
  children: React.ReactNode;
  tooltip?: string;
}) {
  const { t } = useTranslation();
  return (
    <div className="flex items-center gap-1.5">
      <Label htmlFor={htmlFor}>{children}</Label>
      {tooltip && (
        <Tooltip>
          <TooltipTrigger asChild>
            <button
              type="button"
              className="text-muted-foreground transition-colors hover:text-foreground"
              aria-label={t("model.tooltip_more_info")}
            >
              <Info className="size-3.5" />
            </button>
          </TooltipTrigger>
          <TooltipContent>{tooltip}</TooltipContent>
        </Tooltip>
      )}
    </div>
  );
}

export function ModelPanel() {
  const { t, setLang } = useTranslation();
  const [settings, setSettings] = useState<Settings | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [saveError, setSaveError] = useState<string | null>(null);
  // `null` tant que la vérification n'a pas répondu — on n'affiche l'alerte
  // « modèle introuvable » qu'une fois `false` confirmé (pas pendant le
  // chargement, pour éviter un clignotement).
  const [llmModelPresent, setLlmModelPresent] = useState<boolean | null>(null);

  useEffect(() => {
    let cancelled = false;
    getSettings()
      .then((s) => {
        if (!cancelled) setSettings(s);
      })
      .catch((e) => {
        if (!cancelled) setLoadError(String(e));
      });
    llmModelAvailable()
      .then((present) => {
        if (!cancelled) setLlmModelPresent(present);
      })
      .catch(() => {
        // Pas bloquant : en cas d'échec on n'affiche simplement pas
        // l'indication de présence du modèle.
        if (!cancelled) setLlmModelPresent(null);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  async function persist(next: Settings) {
    setSettings(next);
    setSaveError(null);
    try {
      await saveSettings(next);
    } catch (e) {
      setSaveError(String(e));
    }
  }

  // La langue est un cas particulier : `setLang` (contexte `i18n.tsx`) met
  // à jour l'affichage de toute la fenêtre immédiatement, en plus de
  // persister via `persist` ci-dessus comme les autres réglages.
  function changeLanguage(value: Lang) {
    setLang(value);
    void persist({ ...settings!, ui_language: value });
  }

  if (loadError) {
    return (
      <section className="model-panel text-sm text-destructive">
        {t("model.load_error", { error: loadError })}
      </section>
    );
  }

  if (!settings) {
    return (
      <section className="model-panel text-sm text-muted-foreground">
        {t("model.loading")}
      </section>
    );
  }

  return (
    <section className="model-panel flex flex-col gap-6">
      <Card>
        <CardHeader>
          <CardTitle>{t("model.language_card_title")}</CardTitle>
          <CardDescription>{t("model.language_card_desc")}</CardDescription>
        </CardHeader>
        <CardContent>
          <div className="flex flex-col gap-2">
            <Label htmlFor="ui-language">{t("model.language_label")}</Label>
            <Select
              value={settings.ui_language}
              onValueChange={(value) => changeLanguage(value as Lang)}
            >
              <SelectTrigger id="ui-language" className="w-full">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="en">{t("model.language_en")}</SelectItem>
                <SelectItem value="fr">{t("model.language_fr")}</SelectItem>
              </SelectContent>
            </Select>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>{t("model.card_title")}</CardTitle>
          <CardDescription>{t("model.card_desc")}</CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-6">
          <div className="flex flex-col gap-2">
            <FieldLabel htmlFor="asr-engine" tooltip={t("model.asr_engine_tooltip")}>
              {t("model.asr_engine_label")}
            </FieldLabel>
            <Select
              value={settings.asr_provider}
              onValueChange={(value) =>
                persist({ ...settings, asr_provider: value })
              }
            >
              <SelectTrigger id="asr-engine" className="w-full">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {ASR_ENGINES.map((engine) => (
                  <SelectItem
                    key={engine.value}
                    value={engine.value}
                    disabled={engine.disabled}
                  >
                    {t(engine.labelKey)}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            <p className="text-sm text-muted-foreground">
              {t(
                ASR_ENGINES.find((e) => e.value === settings.asr_provider)
                  ?.descriptionKey ?? "",
              )}
            </p>
          </div>

          <Separator />

          <div className="flex items-center justify-between gap-4">
            <div className="flex flex-col gap-0.5">
              <FieldLabel htmlFor="cloud-enabled" tooltip={t("model.cloud_enabled_tooltip")}>
                {t("model.cloud_enabled_label")}
              </FieldLabel>
              <p className="text-sm text-muted-foreground">{t("model.cloud_enabled_desc")}</p>
            </div>
            <Switch
              id="cloud-enabled"
              checked={settings.cloud_enabled}
              onCheckedChange={(checked) =>
                persist({ ...settings, cloud_enabled: checked })
              }
            />
          </div>

          {settings.cloud_enabled && (
            <div className="flex flex-col gap-4 rounded-md border border-border/60 bg-muted/30 p-3">
              <Alert variant="destructive">
                <TriangleAlert />
                <AlertTitle>{t("model.cloud_alert_title")}</AlertTitle>
                <AlertDescription>{t("model.cloud_alert_desc")}</AlertDescription>
              </Alert>

              <div className="flex flex-col gap-2">
                <Label htmlFor="cloud-provider">{t("model.cloud_provider_label")}</Label>
                <Combobox
                  id="cloud-provider"
                  options={CLOUD_PROVIDERS}
                  value={settings.cloud_provider}
                  onValueChange={(value) =>
                    persist({ ...settings, cloud_provider: value })
                  }
                  placeholder={t("model.cloud_provider_placeholder")}
                  searchPlaceholder={t("model.cloud_provider_search_placeholder")}
                  emptyText={t("model.cloud_provider_empty")}
                />
              </div>

              <div className="flex flex-col gap-2">
                <Label htmlFor="cloud-api-key">{t("model.cloud_api_key_label")}</Label>
                <Input
                  id="cloud-api-key"
                  type="password"
                  autoComplete="off"
                  spellCheck={false}
                  placeholder="gsk_…"
                  value={settings.cloud_api_key}
                  onChange={(e) =>
                    persist({ ...settings, cloud_api_key: e.target.value })
                  }
                />
                <p className="text-sm text-muted-foreground">{t("model.cloud_api_key_note")}</p>
              </div>
            </div>
          )}

          <Separator />

          {/* Nettoyage IA local (§2.4) — même pattern visuel que la bascule
              cloud ci-dessus. Différence clé : le modèle LLM n'est chargé
              qu'au démarrage (`CleanerState` dans `lib.rs::run().setup()`),
              donc le changement ne prend PAS effet à chaud (note discrète
              sous le switch, pas une Alert destructive). */}
          <div className="flex flex-col gap-2">
            <div className="flex items-center justify-between gap-4">
              <div className="flex flex-col gap-0.5">
                <FieldLabel htmlFor="llm-cleanup-enabled" tooltip={t("model.llm_cleanup_tooltip")}>
                  {t("model.llm_cleanup_label")}
                </FieldLabel>
                <p className="text-sm text-muted-foreground">{t("model.llm_cleanup_desc")}</p>
              </div>
              <Switch
                id="llm-cleanup-enabled"
                checked={settings.llm_cleanup_enabled}
                onCheckedChange={(checked) =>
                  persist({ ...settings, llm_cleanup_enabled: checked })
                }
              />
            </div>
            <p className="text-xs text-muted-foreground">{t("model.llm_cleanup_restart_note")}</p>
            {settings.llm_cleanup_enabled && llmModelPresent === false && (
              <p className="text-sm text-destructive">{t("model.llm_model_missing")}</p>
            )}
          </div>

          <Separator />

          {/* Déclenchement de la dictée : maintien de Ctrl + Win, câblé en
              dur (`hotkey::modifier_combo`, seul mécanisme depuis la demande
              utilisateur du 2026-09-02). Plus aucun raccourci configurable
              ici — simple rappel. */}
          <div className="flex flex-col gap-2">
            <Label>{t("model.trigger_label")}</Label>
            <div>
              <kbd className="inline-flex items-center rounded-md border border-input bg-muted px-2.5 py-1 font-mono text-sm font-medium">
                Ctrl + Win
              </kbd>
            </div>
            <p className="text-sm text-muted-foreground">{t("model.trigger_body")}</p>
          </div>

          {saveError && (
            <p className="text-sm text-destructive">
              {t("model.save_error", { error: saveError })}
            </p>
          )}
        </CardContent>
      </Card>
    </section>
  );
}
