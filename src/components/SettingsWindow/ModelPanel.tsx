// Choix du moteur ASR local, activation du cloud et raccourci clavier
// global (Spec_Frontend.md Phase 1 §1.2). Toute modification appelle
// `saveSettings` immédiatement (persistée côté backend dans un fichier JSON,
// voir `src-tauri/src/storage/settings.rs` — Spec_Backend_Desktop.md §1.6).
//
// Layout (§1.3) : les quatre groupes de réglages sont séparés par un
// `Separator` ; les options techniques (moteur local, bascule cloud) portent
// un `Tooltip` explicatif.

import { useEffect, useState } from "react";
import { Info } from "lucide-react";
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
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group";
import { Separator } from "@/components/ui/separator";
import { Switch } from "@/components/ui/switch";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { Input } from "@/components/ui/input";
import { getSettings, saveSettings } from "@/lib/tauri";
import type { HotkeyMode, Settings } from "@/types";

// Moteurs ASR locaux connus. `whisper-cpp` reste désactivé dans le
// sélecteur : le benchmark français (voir PROGRESS.md, Session 4) a retenu
// Parakeet-TDT ONNX INT8 par défaut pour `asr::local` (Spec_Backend_Desktop.md
// §1.3, ~4-5x plus rapide et WER plus bas sur l'échantillon testé) ;
// whisper.cpp reste un module stub documenté, pas encore branché.
const ASR_ENGINES: Array<{
  value: string;
  label: string;
  description: string;
  disabled?: boolean;
}> = [
  {
    value: "parakeet-tdt",
    label: "Parakeet-TDT (recommandé)",
    description:
      "Modèle NVIDIA multilingue (ONNX INT8, ~670 Mo). Le plus rapide et le plus précis en français sur notre benchmark interne — moteur par défaut.",
  },
  {
    value: "whisper-cpp",
    label: "whisper.cpp (bientôt disponible)",
    description:
      "Alternative plus légère mais plus lente et moins précise en français sur notre benchmark interne. Pas encore branché côté backend.",
    disabled: true,
  },
];

const HOTKEY_MODE_OPTIONS: Array<{
  value: HotkeyMode;
  label: string;
  description: string;
}> = [
  {
    value: "toggle",
    label: "Bascule",
    description: "Un appui démarre la dictée, un second appui l'arrête.",
  },
  {
    value: "push_to_talk",
    label: "Maintien",
    description: "La dictée est active tant que le raccourci est maintenu.",
  },
];

// Touches de modificateur seules : ignorées tant qu'aucune touche
// "normale" n'est pressée en plus (on ne veut pas enregistrer "Control"
// tout seul comme raccourci).
const MODIFIER_CODES = new Set([
  "ControlLeft",
  "ControlRight",
  "AltLeft",
  "AltRight",
  "ShiftLeft",
  "ShiftRight",
  "MetaLeft",
  "MetaRight",
]);

// Construit une chaîne compatible `global_hotkey::hotkey::HotKey::from_str`
// (voir src-tauri/src/hotkey/mod.rs) à partir d'un événement clavier DOM.
// `event.code` (ex. "Space", "KeyA", "Digit1", "F5") correspond directement
// à la syntaxe attendue côté Rust pour la touche principale.
function hotkeyFromEvent(event: React.KeyboardEvent): string | null {
  if (MODIFIER_CODES.has(event.code)) {
    return null;
  }
  const mods: string[] = [];
  if (event.ctrlKey) mods.push("control");
  if (event.altKey) mods.push("alt");
  if (event.shiftKey) mods.push("shift");
  if (event.metaKey) mods.push("super");
  if (mods.length === 0) {
    // Un raccourci global sans modificateur intercepterait la touche dans
    // toutes les apps — non supporté ici, cohérent avec le défaut
    // "control+shift+Space" (voir storage::settings::Settings::default).
    return null;
  }
  return [...mods, event.code].join("+");
}

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
  return (
    <div className="flex items-center gap-1.5">
      <Label htmlFor={htmlFor}>{children}</Label>
      {tooltip && (
        <Tooltip>
          <TooltipTrigger asChild>
            <button
              type="button"
              className="text-muted-foreground transition-colors hover:text-foreground"
              aria-label="Plus d'informations"
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
  const [settings, setSettings] = useState<Settings | null>(null);
  const [recording, setRecording] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [saveError, setSaveError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    getSettings()
      .then((s) => {
        if (!cancelled) setSettings(s);
      })
      .catch((e) => {
        if (!cancelled) setLoadError(String(e));
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

  if (loadError) {
    return (
      <section className="model-panel text-sm text-destructive">
        Impossible de charger les réglages : {loadError}
      </section>
    );
  }

  if (!settings) {
    return (
      <section className="model-panel text-sm text-muted-foreground">
        Chargement des réglages…
      </section>
    );
  }

  return (
    <section className="model-panel">
      <Card>
        <CardHeader>
          <CardTitle>Moteur de dictée</CardTitle>
          <CardDescription>
            Choix du moteur de reconnaissance vocale local et du raccourci
            global d'activation.
          </CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-6">
          <div className="flex flex-col gap-2">
            <FieldLabel
              htmlFor="asr-engine"
              tooltip="Traitement 100 % sur votre machine : l'audio ne quitte jamais l'appareil, la dictée fonctionne hors ligne."
            >
              Moteur ASR local
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
                    {engine.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            <p className="text-sm text-muted-foreground">
              {
                ASR_ENGINES.find((e) => e.value === settings.asr_provider)
                  ?.description
              }
            </p>
          </div>

          <Separator />

          <div className="flex items-center justify-between gap-4">
            <div className="flex flex-col gap-0.5">
              <FieldLabel
                htmlFor="cloud-enabled"
                tooltip="En local, l'audio ne quitte jamais l'appareil. Le cloud peut être plus rapide ou plus précis, mais envoie l'audio à un fournisseur tiers — désactivé par défaut."
              >
                Utiliser le cloud
              </FieldLabel>
              <p className="text-sm text-muted-foreground">
                Désactivé par défaut — Vozel fonctionne entièrement en local.
              </p>
            </div>
            <Switch
              id="cloud-enabled"
              checked={settings.cloud_enabled}
              onCheckedChange={(checked) =>
                persist({ ...settings, cloud_enabled: checked })
              }
            />
          </div>

          <Separator />

          <div className="flex flex-col gap-2">
            <Label htmlFor="hotkey-input">Raccourci global</Label>
            <Input
              id="hotkey-input"
              readOnly
              value={recording ? "Appuyez sur une combinaison…" : settings.hotkey}
              placeholder="Cliquez puis appuyez sur une combinaison"
              onFocus={() => setRecording(true)}
              onBlur={() => setRecording(false)}
              onKeyDown={(event) => {
                event.preventDefault();
                if (event.code === "Escape") {
                  setRecording(false);
                  event.currentTarget.blur();
                  return;
                }
                const next = hotkeyFromEvent(event);
                if (next) {
                  setRecording(false);
                  event.currentTarget.blur();
                  persist({ ...settings, hotkey: next });
                }
              }}
            />
            <p className="text-sm text-muted-foreground">
              Cliquez dans le champ puis appuyez sur la combinaison désirée
              (au moins un modificateur : Ctrl, Alt, Maj ou Cmd/Win). Échap
              pour annuler. Un redémarrage de l'app est nécessaire pour
              qu'un nouveau raccourci prenne effet.
            </p>
          </div>

          <Separator />

          <div className="flex flex-col gap-2">
            <Label>Mode du raccourci</Label>
            <RadioGroup
              value={settings.hotkey_mode}
              onValueChange={(value) =>
                persist({ ...settings, hotkey_mode: value as HotkeyMode })
              }
              className="flex flex-col gap-2"
            >
              {HOTKEY_MODE_OPTIONS.map((option) => (
                <div key={option.value} className="flex items-start gap-2">
                  <RadioGroupItem
                    value={option.value}
                    id={`hotkey-mode-${option.value}`}
                    className="mt-0.5"
                  />
                  <Label
                    htmlFor={`hotkey-mode-${option.value}`}
                    className="flex flex-col items-start gap-0.5 font-normal"
                  >
                    <span>{option.label}</span>
                    <span className="text-sm text-muted-foreground">
                      {option.description}
                    </span>
                  </Label>
                </div>
              ))}
            </RadioGroup>
          </div>

          {saveError && (
            <p className="text-sm text-destructive">
              Échec de la sauvegarde : {saveError}
            </p>
          )}
        </CardContent>
      </Card>
    </section>
  );
}
