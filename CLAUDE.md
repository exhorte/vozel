# Vozel — Instructions pour Claude Code

Vozel est une application de dictée voix-vers-texte (dictée IA, insertion de
texte dans n'importe quelle app), produit ExponentValue. Projet réel en
construction (pas un prototype) : code propre, testé au fil de l'eau,
historique traçable d'une session à l'autre.

**Stack** : Tauri 2 + Rust (shell desktop) · React + TypeScript + Vite
(frontend) · **shadcn/ui** comme bibliothèque de référence pour tout composant
d'interface (installé via `npx shadcn@latest add <composant>`, jamais écrit à
la main) · ASR local Parakeet-TDT (ONNX INT8) ou whisper.cpp quantifié (choix
définitif après benchmark français, voir Blocages dans `PROGRESS.md`) · SQLite
embarqué (Phase 2) · cloud optionnel multi-fournisseurs (jamais requis).

## Démarrage de session

**Toujours lire `PROGRESS.md` en tout premier** pour savoir où le projet en
est (état actuel, prochaine étape, blocages ouverts) avant de reprendre le
travail.

## Documents de référence (hors dépôt Git, ne jamais modifier)

Chemins relatifs à `04_Code/` :
- `../_INDEX.md` — dashboard projet
- `../01_Recherche/Analyse_WisprFlow_BridgeVoice_vs_Vozel.md` — analyse concurrentielle et stack (§4-5)
- `../02_Plan_Projet/Roadmap.md` — jalons macro par phase
- `../02_Plan_Projet/Spec_Backend_Desktop.md` — spec backend Rust/Tauri étape par étape
- `../02_Plan_Projet/Spec_Frontend.md` — spec frontend React/TS + shadcn/ui étape par étape
- `../02_Plan_Projet/Spec_Backend_Cloud.md` — spec cloud (pertinent à partir de Phase 2)
- `../03_Suivi_Projet/Suivi.md` — journal de bord produit (historique des décisions)
- `../02_Plan_Projet/Prompt_Claude_Code.md` — prompt d'origine, règles complètes de fonctionnement

Ces documents sont maintenus par ailleurs et lus comme référence uniquement.
Si une info y semble périmée ou contredite par le code, le signaler plutôt
que de la corriger soi-même.

## Déroulé du travail

Avancer phase par phase dans l'ordre des specs (Phase 0 → 1 → 2 → 3). Pour
chaque étape numérotée : implémenter → vérifier le critère d'acceptation
(`cargo check` / `tsc --noEmit` a minima, test manuel quand spécifié) →
committer → mettre à jour `PROGRESS.md`. Ne jamais avancer sur une étape
suivante si la courante ne compile pas ou ne remplit pas son critère.

## Git

Dépôt distant : `https://github.com/exhorte/vozel.git`. Travail direct sur
`main` (projet solo, phase précoce). Un commit par étape de spec terminée,
format :

```
[Backend|Frontend|Cloud] <résumé court> (Spec_X §<numéro d'étape>)
```

Pousser vers `origin main` à la fin de chaque session. Ne jamais committer de
secret (clé API, token) — vérifier `.gitignore` avant tout commit touchant un
fichier de config locale.

## Garde-fous

- Ne pas sauter de phase, ne pas deviner de fonctionnalité non spécifiée —
  demander une clarification plutôt qu'improviser une décision produit.
- Ne pas ajouter de dépendance hors specs sans le signaler dans
  `PROGRESS.md` (avec la raison).
- Composants d'interface : toujours via shadcn/ui, jamais réimplémentés à la main.
- Ne jamais modifier `../01_Recherche/`, `../02_Plan_Projet/`, `../03_Suivi_Projet/`, `../_INDEX.md`.
- Ne pas marquer un critère d'acceptation nécessitant un test manuel comme
  validé sans l'avoir réellement fait — indiquer dans `PROGRESS.md` si un
  test manuel reste à faire par l'utilisateur.
