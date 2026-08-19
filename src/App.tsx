import { useState } from "react";
import { FloatingWidget } from "./components/FloatingWidget/FloatingWidget";
import { SettingsWindow } from "./components/SettingsWindow/SettingsWindow";
import { startDictation, stopDictation } from "./lib/tauri";
import type { DictationStatus } from "./types";
import "./App.css";

// Point d'entrée UI (Phase 1). Deux surfaces prévues par l'architecture
// (voir analyse section 4.2) : le widget flottant pendant la dictée et la
// fenêtre de réglages. Le routage entre les deux fenêtres réelles
// (multi-fenêtres Tauri) sera mis en place quand la spec UI sera figée —
// pour l'instant les deux sont montées ensemble pour visualiser la structure.

function App() {
  const [status, setStatus] = useState<DictationStatus>("idle");

  async function toggleDictation() {
    if (status === "idle") {
      setStatus("listening");
      await startDictation();
    } else {
      setStatus("idle");
      await stopDictation();
    }
  }

  return (
    <main className="container">
      <h1>Vozel</h1>
      <button onClick={toggleDictation}>
        {status === "idle" ? "Démarrer la dictée" : "Arrêter"}
      </button>
      <FloatingWidget status={status} />
      <SettingsWindow />
    </main>
  );
}

export default App;
