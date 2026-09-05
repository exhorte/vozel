// Internationalisation de l'interface (FR/EN) — demande utilisateur
// 2026-09-04. Jusqu'ici toute l'UI React était codée en dur en français ;
// `ui_language` (Settings, défaut "en") pilote maintenant l'affichage.
//
// Choix : dictionnaire à plat fait main (`DICTS` ci-dessous), pas de
// dépendance externe (i18next, react-intl…) — cohérent avec le reste du
// projet ("trois dépendances pour ça ne se justifient pas", voir
// DictionaryPanel.tsx) pour ~130 chaînes statiques, sans pluriels complexes
// au-delà du singulier/pluriel simple déjà géré à la main.
//
// Portée : seule la fenêtre `main` (Réglages) a du texte — l'overlay `flow
// bar` n'affiche aucun texte (voir FloatingWidget.tsx). Pas besoin d'y
// brancher ce module.
//
// Ne couvre PAS les messages d'erreur générés côté Rust (`Result<T, String>`
// renvoyés tels quels par les commandes IPC, ex. « le champ ne peut pas être
// vide ») : ils restent en français quelle que soit la langue choisie — les
// traduire demanderait un mécanisme de code d'erreur côté backend, hors
// périmètre de cette demande (signalé dans PROGRESS.md).

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import { getSettings, saveSettings } from "@/lib/tauri";

export type Lang = "en" | "fr";

// Clé plate `"section.name"` → chaîne. `{var}` est remplacé par
// `translate()`. Les paires singulier/pluriel utilisent `_one` / `_other`
// (anglais et français ont tous deux exactement deux formes ici).
type Dict = Record<string, string>;

const en: Dict = {
  "brand.name": "Vozel",

  "sidebar.home": "Home",
  "sidebar.dictionary": "Dictionary",
  "sidebar.history": "History",
  "sidebar.settings": "Settings",
  "sidebar.version": "Vozel v{version}",

  "titlebar.collapse": "Collapse sidebar",
  "titlebar.expand": "Expand sidebar",
  "titlebar.account": "Account (coming soon)",
  "titlebar.account_short": "Account — coming soon",
  "titlebar.notifications": "Notifications (coming soon)",
  "titlebar.notifications_short": "Notifications — coming soon",
  "titlebar.minimize": "Minimize",
  "titlebar.maximize": "Maximize",
  "titlebar.restore": "Restore",
  "titlebar.close": "Close",

  "page.home": "Home",
  "page.dictionary": "Dictionary",
  "page.history": "History",

  "home.welcome_title": "Welcome to Vozel",
  "home.welcome_body":
    "Voice-to-text dictation, 100% local. Place your cursor where you want to write, hold the shortcut, speak: the text is inserted at the cursor when you're done.",
  "home.trigger_title": "Dictation trigger",
  "home.trigger_desc":
    "Hold both keys together while you speak, release to transcribe and insert.",
  "home.trigger_note":
    "Single, fixed shortcut. Note: releasing the Windows key last may open the Start menu.",
  "home.load_error": "Couldn't load history: {error}",
  "home.loading": "Loading…",
  "home.your_dictations_title": "Your dictations",
  "home.empty_body":
    "No dictations yet. Your dictations will appear here, with a summary of your activity for today and this week.",
  "home.activity_title": "Your activity",
  "home.activity_desc": "Counted locally on this machine, never shared.",
  "home.stat_today": "Today",
  "home.stat_week": "Last 7 days",
  "home.recent_title": "Recent dictations",
  "home.see_all_history": "See all history",
  "home.time_just_now": "just now",
  "home.time_minutes_ago": "{n} min ago",
  "home.time_hours_ago": "{n} h ago",
  "home.time_days_ago": "{n} d ago",
  "home.stat_dictation_one": "{n} dictation",
  "home.stat_dictation_other": "{n} dictations",
  "home.stat_word_one": "{n} word",
  "home.stat_word_other": "{n} words",

  "model.card_title": "Dictation engine",
  "model.card_desc": "Local speech-recognition engine and global activation shortcut.",
  "model.asr_engine_label": "Local ASR engine",
  "model.asr_engine_tooltip":
    "100% on-device processing: audio never leaves the machine, dictation works offline.",
  "model.asr_parakeet_label": "Parakeet-TDT (recommended)",
  "model.asr_parakeet_desc":
    "NVIDIA multilingual model (ONNX INT8, ~670 MB). Fastest and most accurate in French on our internal benchmark — default engine.",
  "model.asr_whisper_cpp_label": "whisper.cpp (coming soon)",
  "model.asr_whisper_cpp_desc":
    "Lighter alternative, but slower and less accurate in French on our internal benchmark. Not wired up on the backend yet.",
  "model.cloud_enabled_label": "Use the cloud",
  "model.cloud_enabled_tooltip":
    "Locally, audio never leaves the device. Cloud can be faster or more accurate, but sends audio to a third-party provider — off by default.",
  "model.cloud_enabled_desc": "Off by default — Vozel runs entirely locally.",
  "model.cloud_alert_title": "Audio leaves your device",
  "model.cloud_alert_desc":
    "When cloud is enabled, dictated audio is sent to the chosen provider for transcription. Vozel stays local by default — only enable cloud if you're comfortable with that.",
  "model.cloud_provider_label": "Provider",
  "model.cloud_provider_placeholder": "Choose a provider",
  "model.cloud_provider_search_placeholder": "Filter providers…",
  "model.cloud_provider_empty": "No provider.",
  "model.cloud_api_key_label": "API key",
  "model.cloud_api_key_note":
    "Your own key, stored locally on this machine and never logged. For Groq: console.groq.com/keys. Takes effect on the next dictation, no restart needed.",
  "model.llm_cleanup_label": "Local AI cleanup",
  "model.llm_cleanup_tooltip":
    "A small language model runs 100% locally to fix punctuation, accents, and agreement beyond the simple rules. The model (~1.9 GB) isn't bundled with the app: place it manually in %APPDATA%\\com.exponentvalue.vozel\\models\\llm\\ (see PROGRESS.md for the procedure).",
  "model.llm_cleanup_desc": "Advanced correction via a local language model, on top of the rules. Off by default, 100% offline.",
  "model.llm_cleanup_restart_note":
    "This change only takes effect after restarting the app — the model is only loaded at startup.",
  "model.llm_model_missing":
    "Model not found in models\\llm\\. Cleanup falls back to the simple rules until the model is installed there.",
  "model.trigger_label": "Dictation trigger",
  "model.trigger_body":
    "Hold Ctrl + Win while you speak; release to transcribe and insert the text at the cursor. Single, fixed shortcut (not configurable). Note: releasing the Windows key last may open the Start menu.",
  "model.save_error": "Save failed: {error}",
  "model.load_error": "Couldn't load settings: {error}",
  "model.loading": "Loading settings…",
  "model.tooltip_more_info": "More information",
  "model.language_card_title": "Interface language",
  "model.language_card_desc": "Language used throughout the Vozel interface.",
  "model.language_label": "Language",
  "model.language_en": "English",
  "model.language_fr": "Français",

  "dict.title": "Custom dictionary",
  "dict.subtitle": "Replacements applied after transcription, before cleanup — for proper names and jargon.",
  "dict.intro_dismiss": "Hide this card",
  "dict.intro_title": "Vozel writes words the way you say them.",
  "dict.intro_body":
    "When a transcription consistently gets a proper name or industry term wrong, add a correction: the recognized text (left) is replaced by the intended form (right) in every dictation, before cleanup. Case- and spacing-insensitive.",
  "dict.intro_add": "Add a correction",
  "dict.filter_placeholder": "Filter…",
  "dict.filter_aria": "Filter dictionary entries",
  "dict.add": "Add",
  "dict.load_error": "Couldn't load the dictionary: {error}",
  "dict.loading": "Loading…",
  "dict.empty": "No entries. Add one to automatically fix a mistranscribed word.",
  "dict.col_from": "Recognized",
  "dict.col_to": "Corrected to",
  "dict.actions_for": "Actions for “{from}”",
  "dict.edit": "Edit",
  "dict.delete": "Delete",
  "dict.no_match": "No entries match “{filter}”.",
  "dict.dialog_add_title": "Add an entry",
  "dict.dialog_edit_title": "Edit entry",
  "dict.dialog_desc": "The “recognized” text is replaced by the “corrected” text in every dictation (case- and spacing-insensitive).",
  "dict.field_from": "Recognized",
  "dict.field_to": "Corrected to",
  "dict.cancel": "Cancel",
  "dict.save": "Save",
  "dict.delete_confirm_title": "Delete this entry?",
  "dict.delete_confirm_body": "“{from}” → “{to}” will no longer be applied to dictations. This action is permanent.",
  "dict.toast_added": "“{from}” added to the dictionary",
  "dict.toast_updated": "Entry updated",
  "dict.toast_deleted": "“{from}” deleted",

  "history.title": "History",
  "history.subtitle_cloud":
    "Your dictations are saved locally on this device. Since cloud mode is on, audio is sent to a third-party provider at transcription time (the text itself stays local).",
  "history.subtitle_local": "Your dictations are saved locally on this device and never leave it.",
  "history.search_placeholder": "Search history…",
  "history.search_aria": "Search history",
  "history.load_error": "Couldn't load history: {error}",
  "history.loading": "Loading…",
  "history.no_match": "No dictation contains “{query}”.",
  "history.empty": "No dictations yet. Your dictations will appear here as you record them.",
  "history.copy_text": "Copy text",
  "history.more_actions": "More actions",
  "history.delete": "Delete",
  "history.delete_confirm_title": "Delete this dictation?",
  "history.delete_confirm_body": "It will be permanently removed from your local history. This action is irreversible.",
  "history.cancel": "Cancel",
  "history.clear_all": "Clear all history",
  "history.clear_all_confirm_title": "Clear all history?",
  "history.clear_all_confirm_body": "All locally saved dictations will be permanently deleted, including their text. This action is irreversible.",
  "history.clear_all_confirm_action": "Clear everything",
  "history.toast_copied": "Text copied",
  "history.toast_copy_failed": "Couldn't copy",
  "history.toast_deleted": "Dictation deleted",
  "history.toast_cleared": "History cleared",
  "history.group_today": "Today",
  "history.group_yesterday": "Yesterday",
  "history.group_this_week": "This week",
  "history.word_one": "{n} word",
  "history.word_other": "{n} words",

  "settingsModal.title": "Settings",
  "settingsModal.desc": "Vozel configuration: dictation engine and privacy.",
  "settingsModal.section_general": "General",
  "settingsModal.section_privacy": "Data & privacy",
  "settingsModal.privacy_title": "Data & privacy",
  "settingsModal.privacy_body_base":
    "Vozel keeps locally on this machine: your settings, your personal dictionary, and your dictation history (the transcribed text only — never the audio).",
  "settingsModal.privacy_body_cloud":
    " Since cloud mode is enabled, the audio of each dictation is also sent to the chosen provider for transcription.",
  "settingsModal.privacy_body_no_cloud": " Nothing is sent over the Internet as long as cloud stays disabled.",
  "settingsModal.privacy_history_note_before": "History can be viewed, searched, and cleared from the",
  "settingsModal.privacy_history_note_link": "History",
  "settingsModal.privacy_history_note_after":
    "page in the sidebar (each dictation can also be deleted individually there).",

  "settingsModal.section_account": "Account",
  "account.title": "Account",
  "account.subtitle": "Sign in to sync your dictionary and settings across devices (coming soon) and see your plan.",
  "account.loading": "Checking your session…",
  "account.email_label": "Email",
  "account.password_label": "Password",
  "account.sign_in": "Sign in",
  "account.sign_up": "Create account",
  "account.sign_out": "Sign out",
  "account.switch_to_signin": "Already have an account? Sign in",
  "account.switch_to_signup": "No account yet? Create one",
  "account.signup_success": "Account created. Check your email to confirm it, if required.",
  "account.tier_free": "Free plan",
  "account.tier_pro": "Pro plan",
  "account.tier_unknown": "Plan unavailable",
  "account.error_email_taken": "This email is already in use.",
  "account.error_weak_password": "Password must be at least 6 characters.",
  "account.error_invalid_credentials": "Incorrect email or password.",
  "account.error_invalid_email": "This email address doesn't look valid.",
  "account.error_email_not_confirmed":
    "Please confirm your email before signing in — check your inbox for the confirmation link.",
};

const fr: Dict = {
  "brand.name": "Vozel",

  "sidebar.home": "Accueil",
  "sidebar.dictionary": "Dictionnaire",
  "sidebar.history": "Historique",
  "sidebar.settings": "Réglages",
  "sidebar.version": "Vozel v{version}",

  "titlebar.collapse": "Replier la barre latérale",
  "titlebar.expand": "Déplier la barre latérale",
  "titlebar.account": "Compte (bientôt)",
  "titlebar.account_short": "Compte — bientôt",
  "titlebar.notifications": "Notifications (bientôt)",
  "titlebar.notifications_short": "Notifications — bientôt",
  "titlebar.minimize": "Réduire",
  "titlebar.maximize": "Agrandir",
  "titlebar.restore": "Restaurer",
  "titlebar.close": "Fermer",

  "page.home": "Accueil",
  "page.dictionary": "Dictionnaire",
  "page.history": "Historique",

  "home.welcome_title": "Bienvenue dans Vozel",
  "home.welcome_body":
    "Dictée voix-vers-texte, 100 % locale. Placez le curseur où vous voulez écrire, maintenez le raccourci, parlez : à la fin, le texte est inséré à l'endroit du curseur.",
  "home.trigger_title": "Déclencheur de dictée",
  "home.trigger_desc": "Maintenez les deux touches ensemble pendant que vous parlez, relâchez pour transcrire et insérer.",
  "home.trigger_note": "Raccourci unique et fixe. Note : relâcher la touche Windows en dernier peut ouvrir le menu Démarrer.",
  "home.load_error": "Impossible de charger l'historique : {error}",
  "home.loading": "Chargement…",
  "home.your_dictations_title": "Vos dictées",
  "home.empty_body": "Aucune dictée pour l'instant. Vos dictées apparaîtront ici, avec un résumé de votre activité du jour et de la semaine.",
  "home.activity_title": "Votre activité",
  "home.activity_desc": "Compté localement sur cette machine, jamais partagé.",
  "home.stat_today": "Aujourd'hui",
  "home.stat_week": "7 derniers jours",
  "home.recent_title": "Dictées récentes",
  "home.see_all_history": "Voir tout l'historique",
  "home.time_just_now": "à l'instant",
  "home.time_minutes_ago": "il y a {n} min",
  "home.time_hours_ago": "il y a {n} h",
  "home.time_days_ago": "il y a {n} j",
  "home.stat_dictation_one": "{n} dictée",
  "home.stat_dictation_other": "{n} dictées",
  "home.stat_word_one": "{n} mot",
  "home.stat_word_other": "{n} mots",

  "model.card_title": "Moteur de dictée",
  "model.card_desc": "Choix du moteur de reconnaissance vocale local et du raccourci global d'activation.",
  "model.asr_engine_label": "Moteur ASR local",
  "model.asr_engine_tooltip": "Traitement 100 % sur votre machine : l'audio ne quitte jamais l'appareil, la dictée fonctionne hors ligne.",
  "model.asr_parakeet_label": "Parakeet-TDT (recommandé)",
  "model.asr_parakeet_desc":
    "Modèle NVIDIA multilingue (ONNX INT8, ~670 Mo). Le plus rapide et le plus précis en français sur notre benchmark interne — moteur par défaut.",
  "model.asr_whisper_cpp_label": "whisper.cpp (bientôt disponible)",
  "model.asr_whisper_cpp_desc":
    "Alternative plus légère mais plus lente et moins précise en français sur notre benchmark interne. Pas encore branché côté backend.",
  "model.cloud_enabled_label": "Utiliser le cloud",
  "model.cloud_enabled_tooltip":
    "En local, l'audio ne quitte jamais l'appareil. Le cloud peut être plus rapide ou plus précis, mais envoie l'audio à un fournisseur tiers — désactivé par défaut.",
  "model.cloud_enabled_desc": "Désactivé par défaut — Vozel fonctionne entièrement en local.",
  "model.cloud_alert_title": "L'audio quitte votre appareil",
  "model.cloud_alert_desc":
    "Quand le cloud est activé, l'audio dicté est envoyé au fournisseur choisi pour transcription. Vozel reste local par défaut ; n'activez le cloud que si vous l'assumez.",
  "model.cloud_provider_label": "Fournisseur",
  "model.cloud_provider_placeholder": "Choisir un fournisseur",
  "model.cloud_provider_search_placeholder": "Filtrer les fournisseurs…",
  "model.cloud_provider_empty": "Aucun fournisseur.",
  "model.cloud_api_key_label": "Clé API",
  "model.cloud_api_key_note":
    "Votre propre clé, stockée en local sur cette machine et jamais journalisée. Pour Groq : console.groq.com/keys. Prise en compte à la dictée suivante, sans redémarrage.",
  "model.llm_cleanup_label": "Nettoyage IA local",
  "model.llm_cleanup_tooltip":
    "Un petit modèle de langue tourne 100 % en local pour corriger ponctuation, accents et accords au-delà des règles simples. Le modèle (~1,9 Go) n'est pas fourni avec l'app : à placer manuellement dans %APPDATA%\\com.exponentvalue.vozel\\models\\llm\\ (procédure dans la doc du projet, PROGRESS.md).",
  "model.llm_cleanup_desc": "Correction avancée par un modèle de langue local, en plus des règles. Désactivé par défaut, 100 % hors ligne.",
  "model.llm_cleanup_restart_note": "Ce changement ne prend effet qu'au redémarrage de l'app — le modèle n'est chargé qu'au démarrage.",
  "model.llm_model_missing": "Modèle introuvable dans models\\llm\\. Le nettoyage retombe sur les règles simples tant que le modèle n'est pas installé à cet emplacement.",
  "model.trigger_label": "Déclencheur de dictée",
  "model.trigger_body":
    "Maintenez Ctrl + Win pendant que vous parlez ; relâchez pour transcrire et insérer le texte à l'endroit du curseur. Raccourci unique et fixe (non modifiable). Note : relâcher la touche Windows en dernier peut ouvrir le menu Démarrer.",
  "model.save_error": "Échec de la sauvegarde : {error}",
  "model.load_error": "Impossible de charger les réglages : {error}",
  "model.loading": "Chargement des réglages…",
  "model.tooltip_more_info": "Plus d'informations",
  "model.language_card_title": "Langue de l'interface",
  "model.language_card_desc": "Langue utilisée dans toute l'interface de Vozel.",
  "model.language_label": "Langue",
  "model.language_en": "English",
  "model.language_fr": "Français",

  "dict.title": "Dictionnaire personnalisé",
  "dict.subtitle": "Remplacements appliqués après la transcription, avant le nettoyage — pour les noms propres et le jargon.",
  "dict.intro_dismiss": "Masquer cette carte",
  "dict.intro_title": "Vozel écrit les mots comme vous les dites.",
  "dict.intro_body":
    "Quand une transcription se trompe régulièrement sur un nom propre ou un terme métier, ajoutez une correction : le texte reconnu (à gauche) est remplacé par la forme voulue (à droite) dans chaque dictée, avant le nettoyage. Insensible à la casse et aux espaces.",
  "dict.intro_add": "Ajouter une correction",
  "dict.filter_placeholder": "Filtrer…",
  "dict.filter_aria": "Filtrer les entrées du dictionnaire",
  "dict.add": "Ajouter",
  "dict.load_error": "Impossible de charger le dictionnaire : {error}",
  "dict.loading": "Chargement…",
  "dict.empty": "Aucune entrée. Ajoutez-en une pour corriger automatiquement un mot mal transcrit.",
  "dict.col_from": "Reconnu",
  "dict.col_to": "Corrigé en",
  "dict.actions_for": "Actions pour « {from} »",
  "dict.edit": "Modifier",
  "dict.delete": "Supprimer",
  "dict.no_match": "Aucune entrée ne correspond à « {filter} ».",
  "dict.dialog_add_title": "Ajouter une entrée",
  "dict.dialog_edit_title": "Modifier l'entrée",
  "dict.dialog_desc": "Le texte « reconnu » est remplacé par le texte « corrigé » dans chaque dictée (insensible à la casse et aux espaces).",
  "dict.field_from": "Reconnu",
  "dict.field_to": "Corrigé en",
  "dict.cancel": "Annuler",
  "dict.save": "Enregistrer",
  "dict.delete_confirm_title": "Supprimer cette entrée ?",
  "dict.delete_confirm_body": "« {from} » → « {to} » ne sera plus appliqué aux dictées. Cette action est définitive.",
  "dict.toast_added": "« {from} » ajouté au dictionnaire",
  "dict.toast_updated": "Entrée mise à jour",
  "dict.toast_deleted": "« {from} » supprimé",

  "history.title": "Historique",
  "history.subtitle_cloud":
    "Vos dictées sont enregistrées localement sur cet appareil. Le mode cloud étant actif, l'audio est envoyé à un fournisseur tiers au moment de la transcription (le texte, lui, reste local).",
  "history.subtitle_local": "Vos dictées sont enregistrées localement sur cet appareil et n'en sortent pas.",
  "history.search_placeholder": "Rechercher dans l'historique…",
  "history.search_aria": "Rechercher dans l'historique",
  "history.load_error": "Impossible de charger l'historique : {error}",
  "history.loading": "Chargement…",
  "history.no_match": "Aucune dictée ne contient « {query} ».",
  "history.empty": "Aucune dictée pour l'instant. Vos dictées apparaîtront ici au fil de leur enregistrement.",
  "history.copy_text": "Copier le texte",
  "history.more_actions": "Autres actions",
  "history.delete": "Supprimer",
  "history.delete_confirm_title": "Supprimer cette dictée ?",
  "history.delete_confirm_body": "Elle sera retirée définitivement de l'historique local. Cette action est irréversible.",
  "history.cancel": "Annuler",
  "history.clear_all": "Effacer tout l'historique",
  "history.clear_all_confirm_title": "Effacer tout l'historique ?",
  "history.clear_all_confirm_body": "Toutes les dictées enregistrées localement seront supprimées définitivement, y compris leur texte. Cette action est irréversible.",
  "history.clear_all_confirm_action": "Tout effacer",
  "history.toast_copied": "Texte copié",
  "history.toast_copy_failed": "Copie impossible",
  "history.toast_deleted": "Dictée supprimée",
  "history.toast_cleared": "Historique effacé",
  "history.group_today": "Aujourd'hui",
  "history.group_yesterday": "Hier",
  "history.group_this_week": "Cette semaine",
  "history.word_one": "{n} mot",
  "history.word_other": "{n} mots",

  "settingsModal.title": "Réglages",
  "settingsModal.desc": "Configuration de Vozel : moteur de dictée et confidentialité.",
  "settingsModal.section_general": "Général",
  "settingsModal.section_privacy": "Données et confidentialité",
  "settingsModal.privacy_title": "Données et confidentialité",
  "settingsModal.privacy_body_base":
    "Vozel conserve en local, sur cette machine : vos réglages, votre dictionnaire personnel et l'historique de vos dictées (le texte transcrit uniquement — jamais l'audio).",
  "settingsModal.privacy_body_cloud":
    " Le mode cloud étant activé, l'audio de chaque dictée est aussi envoyé au fournisseur choisi pour transcription.",
  "settingsModal.privacy_body_no_cloud": " Rien n'est envoyé sur Internet tant que le cloud reste désactivé.",
  "settingsModal.privacy_history_note_before": "L'historique se consulte, se recherche et s'efface depuis la page",
  "settingsModal.privacy_history_note_link": "Historique",
  "settingsModal.privacy_history_note_after":
    "de la barre latérale (chaque dictée peut aussi y être supprimée individuellement).",

  "settingsModal.section_account": "Compte",
  "account.title": "Compte",
  "account.subtitle": "Connectez-vous pour synchroniser votre dictionnaire et vos réglages entre appareils (bientôt) et voir votre offre.",
  "account.loading": "Vérification de la session…",
  "account.email_label": "Email",
  "account.password_label": "Mot de passe",
  "account.sign_in": "Se connecter",
  "account.sign_up": "Créer un compte",
  "account.sign_out": "Se déconnecter",
  "account.switch_to_signin": "Déjà un compte ? Se connecter",
  "account.switch_to_signup": "Pas encore de compte ? En créer un",
  "account.signup_success": "Compte créé. Vérifiez votre email pour le confirmer, si nécessaire.",
  "account.tier_free": "Offre gratuite",
  "account.tier_pro": "Offre Pro",
  "account.tier_unknown": "Offre indisponible",
  "account.error_email_taken": "Cet email est déjà utilisé.",
  "account.error_weak_password": "Le mot de passe doit contenir au moins 6 caractères.",
  "account.error_invalid_credentials": "Email ou mot de passe incorrect.",
  "account.error_invalid_email": "Cette adresse email ne semble pas valide.",
  "account.error_email_not_confirmed":
    "Confirmez votre email avant de vous connecter — vérifiez votre boîte de réception pour le lien de confirmation.",
};

const DICTS: Record<Lang, Dict> = { en, fr };

/** `lang-XX` pour `toLocaleDateString`/`toLocaleTimeString`. */
export function localeTag(lang: Lang): string {
  return lang === "fr" ? "fr-FR" : "en-US";
}

export function translate(
  lang: Lang,
  key: string,
  vars?: Record<string, string | number>,
): string {
  let str = DICTS[lang][key] ?? DICTS.en[key] ?? key;
  if (vars) {
    for (const [k, v] of Object.entries(vars)) {
      // `.split(x).join(y)` = remplacement global sans dépendre de
      // `String.prototype.replaceAll` (ES2021, hors de la cible ES2020 du
      // projet — voir tsconfig.json).
      str = str.split(`{${k}}`).join(String(v));
    }
  }
  return str;
}

/** Sélectionne `${key}_one` (n === 1) ou `${key}_other`, avec `{n}` résolu. */
export function translatePlural(lang: Lang, n: number, key: string): string {
  return translate(lang, `${key}_${n === 1 ? "one" : "other"}`, { n });
}

interface LanguageContextValue {
  lang: Lang;
  /** Traduction simple, avec interpolation `{var}` optionnelle. */
  t: (key: string, vars?: Record<string, string | number>) => string;
  /** Variante singulier/pluriel — voir `translatePlural`. */
  tp: (n: number, key: string) => string;
  /** Bascule la langue : effet immédiat dans toute la fenêtre (pas besoin de
   *  redémarrer, contrairement au nettoyage IA) + persistance en arrière-plan
   *  via `saveSettings`. */
  setLang: (lang: Lang) => void;
}

const LanguageContext = createContext<LanguageContextValue | null>(null);

/**
 * Charge `ui_language` une fois au montage (valeur par défaut "en" tant que
 * le chargement n'a pas répondu, pour éviter un flash dans une autre langue)
 * et fournit `{lang, t, tp, setLang}` à toute la fenêtre `main`. Monté une
 * seule fois dans `SettingsWindow.tsx` — l'`overlay` et l'ancienne palette
 * Command Mode n'ont pas de texte à traduire.
 */
export function LanguageProvider({ children }: { children: ReactNode }) {
  const [lang, setLangState] = useState<Lang>("en");

  useEffect(() => {
    let cancelled = false;
    getSettings()
      .then((s) => {
        if (!cancelled && (s.ui_language === "en" || s.ui_language === "fr")) {
          setLangState(s.ui_language);
        }
      })
      .catch(() => {
        /* repli sur "en", pas bloquant */
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const setLang = useCallback((next: Lang) => {
    setLangState(next);
    // Persistance en tâche de fond : on ne bloque pas le changement visuel
    // sur l'aller-retour IPC. `getSettings` d'abord pour ne pas écraser
    // d'autres réglages avec un objet partiel.
    void getSettings()
      .then((s) => saveSettings({ ...s, ui_language: next }))
      .catch(() => {
        /* la préférence reste appliquée à l'écran même si la persistance
           échoue ; elle sera simplement redemandée au prochain lancement */
      });
  }, []);

  const value = useMemo<LanguageContextValue>(
    () => ({
      lang,
      t: (key, vars) => translate(lang, key, vars),
      tp: (n, key) => translatePlural(lang, n, key),
      setLang,
    }),
    [lang, setLang],
  );

  return (
    <LanguageContext.Provider value={value}>
      {children}
    </LanguageContext.Provider>
  );
}

export function useTranslation(): LanguageContextValue {
  const ctx = useContext(LanguageContext);
  if (!ctx) {
    throw new Error("useTranslation() must be used within a LanguageProvider");
  }
  return ctx;
}
