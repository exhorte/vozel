// Fenêtre de réglages : choix du modèle ASR (local/cloud), raccourci
// clavier, dictionnaire personnalisé. Phase 1 pour le squelette,
// panneaux enrichis en Phase 2 (dictionnaire) et Phase 2/3 (cloud).

import { ModelPanel } from "./ModelPanel";
import { DictionaryPanel } from "./DictionaryPanel";

export function SettingsWindow() {
  return (
    <div className="settings-window">
      <ModelPanel />
      <DictionaryPanel />
    </div>
  );
}
