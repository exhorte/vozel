//! Backend léger optionnel — uniquement ce qui a réellement besoin d'un
//! serveur : authentification, abonnement, synchronisation optionnelle du
//! dictionnaire/réglages entre appareils, et proxy vers les fournisseurs
//! cloud tiers si l'utilisateur préfère ne pas gérer ses propres clés API.
//! Le calcul (ASR + nettoyage) reste local par défaut (voir analyse 4.6).
//!
//! Implémentation serveur hors de ce dossier `04_Code` (repo séparé
//! recommandé : Rust/Axum ou Cloudflare Workers) — ce module ne contient
//! que le client côté app.

pub mod auth;
pub mod sync;
pub mod proxy;
