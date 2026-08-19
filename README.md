# Vozel — Code

Ce dossier accueillera l'implémentation du produit (repo applicatif). Rien n'est encore scaffoldé.

Stack retenue (voir `../01_Recherche/Analyse_WisprFlow_BridgeVoice_vs_Vozel.md` section 4.7 et `../02_Plan_Projet/Roadmap.md`) :

- Shell desktop : Tauri 2 + Rust
- UI : React/Svelte + TypeScript
- ASR local : Parakeet-TDT (ONNX INT8) ou whisper.cpp quantifié, selon benchmark français à mener
- ASR cloud (option) : multi-fournisseurs (Groq, OpenAI, Deepgram...) au choix utilisateur
- Nettoyage IA : LLM local quantifié (GGUF via llama.cpp) + option cloud
- Stockage local : SQLite embarqué (dictionnaire, réglages)
- Backend léger (auth/sync uniquement) : Rust/Axum ou Cloudflare Workers

Prochaine étape : initialiser le projet Tauri (`cargo create-tauri-app` ou équivalent) une fois la spec fonctionnelle du MVP validée (voir `../02_Plan_Projet/Roadmap.md`, Phase 0).
