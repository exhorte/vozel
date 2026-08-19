# Vozel — Code

Implémentation du produit (repo applicatif Tauri 2 + React/TypeScript).

Stack (voir `../01_Recherche/Analyse_WisprFlow_BridgeVoice_vs_Vozel.md` section 4.7 et `../02_Plan_Projet/Roadmap.md`) :

- Shell desktop : Tauri 2 + Rust
- UI : React + TypeScript + Vite, composants via **shadcn/ui** (Tailwind CSS v4)
- ASR local : Parakeet-TDT (ONNX INT8) ou whisper.cpp quantifié, selon benchmark français à mener
- ASR cloud (option) : multi-fournisseurs (Groq, OpenAI, Deepgram...) au choix utilisateur
- Nettoyage IA : LLM local quantifié (GGUF via llama.cpp) + option cloud
- Stockage local : SQLite embarqué (dictionnaire, réglages)
- Backend léger (auth/sync uniquement, repo séparé) : Rust/Axum ou Cloudflare Workers

## Démarrer

```bash
npm install
npm run tauri dev
```

## Structure

Voir `ARCHITECTURE.md` pour le mapping module ↔ phase, et `PROGRESS.md` pour
l'état d'avancement courant et le journal des sessions de développement.
