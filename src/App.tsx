import { getCurrentWindow } from "@tauri-apps/api/window";
import { FloatingWidget } from "./components/FloatingWidget/FloatingWidget";
import { SettingsWindow } from "./components/SettingsWindow/SettingsWindow";
import { useDictationStatus } from "./state/store";

// Point d'entrée UI : Vozel a deux fenêtres Tauri distinctes (voir
// Spec_Frontend.md Phase 0 — multi-fenêtre) qui chargent la même app React
// mais routent vers un composant racine différent selon le label de la
// fenêtre courante, lu au démarrage.

function App() {
  const label = getCurrentWindow().label;
  const [status] = useDictationStatus();

  if (label === "overlay") {
    return <FloatingWidget status={status} />;
  }

  return <SettingsWindow />;
}

export default App;
