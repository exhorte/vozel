# Vozel — État du projet (code)

## État actuel
- Phase en cours : Phase 0 — mise en place de shadcn/ui (frontend)
- Dernière étape terminée : Scaffold initial (`create-tauri-app`, modules backend/frontend en stubs, `cargo check` et `tsc --noEmit` validés) — fait avant le début du suivi dans ce fichier
- Prochaine étape : `Spec_Frontend.md` Phase 0 §1-5 — Tailwind + shadcn/ui init
- Blocages ouverts :
  - Le benchmark Parakeet-TDT vs whisper.cpp en français, censé être fait en Phase 0 (`Spec_Backend_Desktop.md` §1.3, `Roadmap.md` Phase 0), n'a pas encore été réalisé — il est toujours listé en "Prochaines étapes ouvertes" dans `../03_Suivi_Projet/Suivi.md`. Ne bloque pas la Phase 0 frontend (shadcn/ui), mais bloquera `Spec_Backend_Desktop.md` §1.3 (choix du moteur ASR par défaut) tant qu'il n'est pas tranché — à signaler avant de démarrer ce module.
  - Dépôt distant GitHub `https://github.com/exhorte/vozel.git` : existence non vérifiée côté GitHub à ce stade — à confirmer avant le premier `git push`.

## Journal des sessions

### 2026-08-19 — Session 1
- Lecture des documents de référence (`_INDEX.md`, analyse concurrentielle, `Roadmap.md`, les 3 specs, `Suivi.md`).
- Création de `CLAUDE.md` et `PROGRESS.md` (ce fichier) dans `04_Code/`.
- Initialisation du dépôt Git dans `04_Code/` (n'existait pas encore), vérification/complément de `.gitignore`.
- Décisions : aucune décision technique prise cette session, uniquement mise en place du suivi.
- Ouvert pour la prochaine étape : démarrer `Spec_Frontend.md` Phase 0 (Tailwind + shadcn/ui init).
