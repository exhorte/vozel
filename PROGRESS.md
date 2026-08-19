# Vozel — État du projet (code)

## État actuel
- Phase en cours : Phase 0 — mise en place de shadcn/ui (frontend), section "Fenêtres Tauri (multi-fenêtre)" restante
- Dernière étape terminée : `Spec_Frontend.md` Phase 0 §1-5 (Tailwind CSS v4 + shadcn/ui init, composant `button` installé et validé) **et** la sous-section "Fenêtres Tauri (multi-fenêtre)" §1-4 (deux fenêtres `main`/`overlay` déclarées dans `tauri.conf.json`, routage par label dans `App.tsx`)
- Prochaine étape : `Spec_Frontend.md` Phase 1 §1.1 — Widget flottant (`FloatingWidget.tsx` + `AudioVisualizer.tsx`), **ou** `Spec_Backend_Desktop.md` Phase 1 §1.1 — module `hotkey/` (les deux peuvent démarrer, Phase 0 des deux specs est maintenant terminée)
- Blocages ouverts :
  - Le benchmark Parakeet-TDT vs whisper.cpp en français, censé être fait en Phase 0 (`Spec_Backend_Desktop.md` §1.3, `Roadmap.md` Phase 0), n'a pas encore été réalisé — il est toujours listé en "Prochaines étapes ouvertes" dans `../03_Suivi_Projet/Suivi.md`. Ne bloque aucun travail frontend, mais bloquera `Spec_Backend_Desktop.md` §1.3 (choix du moteur ASR par défaut) tant qu'il n'est pas tranché — à signaler avant de démarrer ce module.
  - Dépôt distant GitHub `https://github.com/exhorte/vozel.git` existe (confirmé via `git ls-remote origin`, dépôt vide) — prêt pour le premier push en fin de session.

## Déviations signalées (pas de nouvelle dépendance hors spec, mais écarts d'exécution à noter)
- Le CLI `shadcn` a été profondément redessiné depuis la rédaction de `Spec_Frontend.md` : il n'a plus de prompts "style" (New York/Default) ni "base color" au sens classique, mais un système de presets (Nova, Vega, Maia...) combinés à un choix de bibliothèque primitive (Radix UI / Base UI / React Aria). Preset retenu : **Nova** avec base **Radix UI** — c'est le plus proche des exigences explicites de la spec (Radix UI sous-jacent, icônes `lucide-react`, base color neutre — confirmé dans `components.json` : `"baseColor": "neutral"`, `"iconLibrary": "lucide"`). `components.json` contient `"style": "radix-nova"` au lieu de `"new-york"` — à garder en tête si un futur `npx shadcn add` se comporte différemment de ce que la spec suppose.

## Tests manuels restants
- `npm run tauri dev` n'a pas été lancé dans cette session (environnement d'exécution de l'agent, pas de session interactive Windows pour ouvrir une vraie fenêtre) — la validation "deux fenêtres distinctes s'ouvrent, overlay transparent/sans bordure, fenêtre de réglages normale" (critère d'acceptation Phase 0 §4 multi-fenêtre) repose pour l'instant sur `tsc --noEmit`, `npm run build` et `cargo check`, tous validés. **À faire par l'utilisateur** : lancer `npm run tauri dev` et confirmer visuellement le rendu des deux fenêtres avant de considérer la Phase 0 frontend entièrement close.

## Journal des sessions

### 2026-08-19/20 — Session 1
- Lecture des documents de référence (`_INDEX.md`, analyse concurrentielle, `Roadmap.md`, les 3 specs, `Suivi.md`).
- Création de `CLAUDE.md` et `PROGRESS.md` (ce fichier) dans `04_Code/`.
- Initialisation du dépôt Git dans `04_Code/` (n'existait pas encore), vérification/complément de `.gitignore` (ajout `src-tauri/target`, `src-tauri/gen`, secrets), ajout du remote `origin` (dépôt distant confirmé existant et vide).
- Commit 1 (`dc9601b`) : baseline du scaffold existant + fichiers de suivi.
- `Spec_Frontend.md` Phase 0 complétée : Tailwind CSS v4 + shadcn/ui installés (preset Nova/Radix UI, voir section "Déviations" ci-dessus), composant `button` shadcn installé et validé, alias `@/*` configuré (tsconfig + vite), multi-fenêtre `main`/`overlay` déclarée dans `tauri.conf.json` avec routage par label dans `App.tsx`, suppression de `App.css` (boilerplate obsolète). `ARCHITECTURE.md` et `README.md` mis à jour en conséquence (étaient périmés). `tsc --noEmit`, `npm run build` et `cargo check` validés.
- Commit 2 (`7eb585e`) : Phase 0 frontend.
- Décisions prises : adaptation du preset shadcn/ui (Nova/Radix) faute d'équivalent exact au CLI classique décrit dans la spec (documenté dans "Déviations signalées" ci-dessus).
- Ouvert pour la prochaine session : test manuel `npm run tauri dev` (voir "Tests manuels restants"), puis démarrage `Spec_Frontend.md` Phase 1 §1.1 (FloatingWidget) et/ou `Spec_Backend_Desktop.md` Phase 1 §1.1 (module `hotkey/`).
