// Client Supabase — auth minimal (email/mot de passe), Spec_Backend_Cloud.md
// « phase Auth minimal », Session 21 (prompt de reprise 2026-09-04).
//
// Pas de serveur à écrire pour cette phase : Supabase fournit directement
// l'auth (service hébergé) et le stockage (Postgres managé). Projet déjà
// créé et configuré côté tableau de bord (table `profiles`, RLS, trigger de
// création automatique à l'inscription) — voir PROGRESS.md pour les détails
// de configuration.
//
// La clé ci-dessous est la clé "publishable" (anon key) : publique par
// construction, protégée par les policies RLS côté serveur, pas par le
// secret — comme une clé Stripe publishable ou une clé Firebase côté
// client. Sûre à committer. Ne JAMAIS utiliser une clé `service_role` ici.
import { createClient } from "@supabase/supabase-js";

const SUPABASE_URL = "https://braqfrcbnxthxkbwcpzd.supabase.co";
const SUPABASE_PUBLISHABLE_KEY = "sb_publishable_qhGfss3lnIZ07kfLSBSv3w_2ziIwL9t";

// `persistSession: true` (défaut) utilise `localStorage` pour conserver la
// session entre deux lancements de l'app — vérifié en conditions réelles
// dans la webview Tauri (WebView2 sous Windows), voir PROGRESS.md.
export const supabase = createClient(SUPABASE_URL, SUPABASE_PUBLISHABLE_KEY);

/** Ligne de la table `profiles` (créée automatiquement à l'inscription par
 *  un trigger Postgres — l'app ne la crée jamais elle-même, seulement la
 *  lit). */
export interface Profile {
  id: string;
  subscription_tier: "free" | "pro";
  created_at: string;
  updated_at: string;
}

/** Lit la ligne `profiles` de l'utilisateur connecté. */
export async function fetchProfile(userId: string): Promise<Profile> {
  const { data, error } = await supabase
    .from("profiles")
    .select("*")
    .eq("id", userId)
    .single();
  if (error) throw error;
  return data as Profile;
}

/**
 * `fetchProfile` avec quelques nouvelles tentatives à court délai.
 *
 * Constaté en test réel (Session 21) : juste après restauration d'une
 * session persistée (`localStorage`) au tout premier chargement, le client
 * Supabase peut émettre la toute première requête REST **avant** d'avoir
 * fini d'attacher le jeton de la session restaurée à ses en-têtes internes
 * — la requête part alors sans `Authorization`, RLS ne retourne aucune
 * ligne, et `fetchProfile` échoue (« Offre indisponible » affiché à tort
 * alors que l'utilisateur est bien connecté). Le client se stabilise en
 * pratique en quelques centaines de ms ; on retente donc avant d'abandonner
 * plutôt que d'afficher un état incorrect après un simple redémarrage.
 */
export async function fetchProfileWithRetry(
  userId: string,
  attempts = 3,
  delayMs = 400,
): Promise<Profile> {
  let lastError: unknown;
  for (let i = 0; i < attempts; i++) {
    if (i > 0) {
      await new Promise((resolve) => setTimeout(resolve, delayMs * i));
    }
    try {
      return await fetchProfile(userId);
    } catch (e) {
      lastError = e;
    }
  }
  throw lastError;
}
