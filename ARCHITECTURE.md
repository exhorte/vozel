# Vozel — Architecture du code

Ce document mappe la structure du repo aux choix techniques actés dans
`../01_Recherche/Analyse_WisprFlow_BridgeVoice_vs_Vozel.md` (section 4) et au
séquencement défini dans `../02_Plan_Projet/Roadmap.md`. À tenir à jour à
mesure que l'architecture évolue.

## Vue d'ensemble

Shell **Tauri 2 + Rust** : logique applicative (audio, ASR, injection, etc.)
en Rust dans `src-tauri/`, UI en **React + TypeScript** dans `src/`, communication
via commandes IPC Tauri (`invoke`).

## Backend (`src-tauri/src/`)

| Module | Rôle | Phase |
|---|---|---|
| `audio/` | Capture micro (`cpal`) + VAD (coupe les silences) | 1 |
| `asr/` | Reconnaissance vocale — `local.rs` (Parakeet-TDT/whisper.cpp), `cloud.rs` (multi-fournisseurs), interface commune `AsrEngine` | 1 (local) / 2 (cloud) |
| `postprocess/` | Nettoyage ponctuation/grammaire (`cleanup.rs`), reformulation vocale d'une sélection façon Command Mode (`command_mode.rs`) | 1 (basique) / 2 (LLM + Command Mode) |
| `injection/` | Un adaptateur par OS (`windows.rs`, `macos.rs`, `linux.rs`) + repli commun `clipboard.rs` | 1 (Windows) / 3 (macOS, Linux) |
| `hotkey/` | Raccourci global (push-to-talk / toggle) | 1 |
| `storage/` | SQLite embarqué : dictionnaire personnalisé (`dictionary.rs`), réglages (`settings.rs`) | 1 (settings) / 2 (dictionary) |
| `cloud/` | Client vers le backend léger optionnel : auth, sync inter-appareils, proxy fournisseurs tiers | 2 / 3 |
| `commands/` | Commandes IPC exposées au frontend (`start_dictation`, `stop_dictation`, `get_settings`, `save_settings`, à étoffer) | continu |

Le backend serveur léger (auth/sync/proxy, Rust/Axum ou Cloudflare Workers,
voir analyse section 4.6) vit dans un repo séparé, pas dans `04_Code/` — ce
dossier ne contient que l'app desktop.

## Frontend (`src/`)

| Dossier | Rôle |
|---|---|
| `components/FloatingWidget/` | Widget flottant affiché pendant la dictée + visualiseur audio |
| `components/SettingsWindow/` | Fenêtre de réglages : choix modèle ASR, dictionnaire |
| `lib/tauri.ts` | Wrappers typés autour des commandes IPC — point d'entrée unique vers le backend |
| `state/` | État local (statut de dictée, réglages en mémoire) |
| `types/` | Types partagés, à garder synchronisés avec les structs Rust sérialisées (`storage/settings.rs`, `asr/types.rs`) |

## Points d'attention architecturaux

- **Injection de texte** : le module le plus sensible (voir analyse section
  4.5). Windows d'abord (`SendInput` + repli presse-papiers), Linux en
  dernier avec un vrai budget pour la distinction X11/Wayland (`ydotool`).
- **ASR local par défaut** : le choix Parakeet-TDT vs whisper.cpp comme
  modèle par défaut n'est pas figé — à trancher après benchmark sur le
  français (voir Roadmap Phase 0), car Parakeet est historiquement plus
  fort en anglais.
- **Découplage local/cloud** : chaque module (`asr`, `postprocess`, `cloud`)
  est pensé pour permettre un mode 100% local par défaut avec option cloud
  activable, plutôt qu'un choix figé à la compilation.

## État du scaffold

Squelette généré via `create-tauri-app` (template `react-ts`) puis restructuré
en modules. Tous les modules backend sont des stubs (`todo!`/`Err("not
implemented")`) qui compilent (`cargo check` validé) mais n'ont pas encore de
logique réelle. Le frontend compile (`tsc --noEmit` validé) avec des
composants placeholders. La prochaine étape est la spec fonctionnelle du MVP
(voir Roadmap Phase 0) avant de commencer l'implémentation réelle module par
module.
