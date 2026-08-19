// Visualiseur audio temps réel (ex. "7 bandes de fréquence" façon
// BridgeVoice). TODO : brancher sur un flux d'amplitude émis par le
// backend pendant l'écoute (event Tauri).

interface AudioVisualizerProps {
  active: boolean;
}

export function AudioVisualizer({ active }: AudioVisualizerProps) {
  // TODO: remplacer par un vrai rendu de bandes de fréquence.
  return <div className="audio-visualizer" data-active={active} />;
}
