-- Command Mode (Spec_Frontend.md §2.2 / Spec_Backend_Desktop.md §2.3 point 3) :
-- palette de reformulation rapide d'une sélection de texte, déclenchée par un
-- raccourci global dédié et rendue dans une fenêtre `command` distincte.
-- Opt-in, désactivé par défaut (comme le cloud, le LLM de nettoyage et le
-- push-to-talk Ctrl+Win) : le raccourci n'est enregistré et le modèle LLM
-- n'est chargé pour cet usage qu'au démarrage si le drapeau est vrai — un
-- changement prend effet au redémarrage de l'app.
ALTER TABLE settings ADD COLUMN command_mode_enabled INTEGER NOT NULL DEFAULT 0;
-- Raccourci global dédié au Command Mode (syntaxe `global_hotkey::hotkey::HotKey`,
-- comme `hotkey`). Distinct du raccourci de dictée pour éviter toute ambiguïté
-- de déclenchement (Spec_Backend_Desktop.md §2.3 : déclenchement explicite en
-- Phase 2, pas de détection d'intention). Défaut `alt+shift+KeyC` :
-- `control+shift+KeyK` (essayé d'abord) est déjà pris globalement sur la
-- machine de dev — même cas que `control+alt+Space` pour la dictée en Session 2.
ALTER TABLE settings ADD COLUMN command_mode_hotkey TEXT NOT NULL DEFAULT 'alt+shift+KeyC';
