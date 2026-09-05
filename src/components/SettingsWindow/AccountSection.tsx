// Section « Compte » de la fenêtre modale Réglages — auth minimal Supabase
// (Spec_Backend_Cloud.md « phase Auth minimal », Session 21, prompt de
// reprise 2026-09-04). Email/mot de passe uniquement — pas d'OAuth (hors
// périmètre explicite du prompt, éviterait la complexité des deep links en
// environnement desktop Tauri).
//
// Pas de logique métier au-delà de l'auth : `subscription_tier` est lu tel
// quel depuis la ligne `profiles` (créée automatiquement à l'inscription
// par un trigger Postgres, jamais par l'app) — aucun système de paiement
// n'existe encore, `'free'` reste la valeur pour tout le monde.
//
// Design volontairement simple à ce stade (demande explicite du prompt de
// reprise) : la fonction prime sur la forme.
//
// La lecture du profil passe par `fetchProfileWithRetry` (`lib/supabase.ts`)
// et non `fetchProfile` directement — voir sa doc : juste après un
// redémarrage complet de l'app, la toute première requête REST peut partir
// avant que le client Supabase ait fini d'attacher le jeton de la session
// restaurée, et échoue silencieusement (RLS ne retourne rien). Constaté en
// test réel Session 21, corrigé par quelques tentatives à court délai.

import { useEffect, useState, type FormEvent } from "react";
import type { AuthError, Session } from "@supabase/supabase-js";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { supabase, fetchProfileWithRetry, type Profile } from "@/lib/supabase";
import { useTranslation } from "@/lib/i18n";

type Mode = "signin" | "signup";

// Codes d'erreur stables exposés par supabase-js v2 (`AuthError.code`) —
// préférés à un `.includes()` sur `error.message` (texte anglais, pas
// garanti stable). Repli sur le message brut si le code est absent/inconnu
// (ex. erreur réseau) : imparfait mais jamais muet.
function authErrorMessage(t: (key: string) => string, error: AuthError): string {
  switch (error.code) {
    case "user_already_exists":
      return t("account.error_email_taken");
    case "weak_password":
      return t("account.error_weak_password");
    case "invalid_credentials":
      return t("account.error_invalid_credentials");
    case "email_address_invalid":
      return t("account.error_invalid_email");
    case "email_not_confirmed":
      return t("account.error_email_not_confirmed");
    default:
      return error.message;
  }
}

export function AccountSection() {
  const { t } = useTranslation();
  const [session, setSession] = useState<Session | null>(null);
  const [checkingSession, setCheckingSession] = useState(true);
  const [profile, setProfile] = useState<Profile | null>(null);

  const [mode, setMode] = useState<Mode>("signin");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [info, setInfo] = useState<string | null>(null);

  // Session initiale (persistée en localStorage par le client Supabase,
  // vérifié fonctionnel dans la webview Tauri/WebView2 — voir PROGRESS.md)
  // + abonnement aux changements (connexion/déconnexion/rafraîchissement de
  // token) pendant toute la durée de vie du composant.
  useEffect(() => {
    let cancelled = false;
    supabase.auth.getSession().then(({ data }) => {
      if (!cancelled) {
        setSession(data.session);
        setCheckingSession(false);
      }
    });
    const {
      data: { subscription },
    } = supabase.auth.onAuthStateChange((_event, s) => {
      if (!cancelled) setSession(s);
    });
    return () => {
      cancelled = true;
      subscription.unsubscribe();
    };
  }, []);

  useEffect(() => {
    let cancelled = false;
    if (!session?.user) {
      setProfile(null);
      return;
    }
    fetchProfileWithRetry(session.user.id)
      .then((p) => {
        if (!cancelled) setProfile(p);
      })
      .catch(() => {
        if (!cancelled) setProfile(null);
      });
    return () => {
      cancelled = true;
    };
  }, [session]);

  async function submit(e: FormEvent) {
    e.preventDefault();
    setError(null);
    setInfo(null);
    setSubmitting(true);
    try {
      if (mode === "signup") {
        const { error: err } = await supabase.auth.signUp({ email, password });
        if (err) throw err;
        setInfo(t("account.signup_success"));
      } else {
        const { error: err } = await supabase.auth.signInWithPassword({
          email,
          password,
        });
        if (err) throw err;
      }
      setPassword("");
    } catch (err) {
      setError(authErrorMessage(t, err as AuthError));
    } finally {
      setSubmitting(false);
    }
  }

  async function signOut() {
    setSubmitting(true);
    try {
      await supabase.auth.signOut();
    } finally {
      setSubmitting(false);
    }
  }

  if (checkingSession) {
    return (
      <section className="flex flex-col gap-4">
        <p className="text-sm text-muted-foreground">{t("account.loading")}</p>
      </section>
    );
  }

  if (session?.user) {
    return (
      <section className="flex flex-col gap-4">
        <h2 className="text-base font-semibold">{t("account.title")}</h2>
        <div className="flex items-center justify-between gap-4 rounded-md border border-border/60 p-3">
          <div className="flex flex-col gap-0.5">
            <span className="text-sm font-medium text-foreground">
              {session.user.email ?? "—"}
            </span>
            <span className="inline-flex w-fit items-center rounded-full border border-border/60 bg-muted/50 px-2 py-0.5 text-xs text-muted-foreground">
              {profile
                ? t(
                    profile.subscription_tier === "pro"
                      ? "account.tier_pro"
                      : "account.tier_free",
                  )
                : t("account.tier_unknown")}
            </span>
          </div>
          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={() => void signOut()}
            disabled={submitting}
          >
            {t("account.sign_out")}
          </Button>
        </div>
      </section>
    );
  }

  return (
    <section className="flex flex-col gap-4">
      <div className="flex flex-col gap-1">
        <h2 className="text-base font-semibold">{t("account.title")}</h2>
        <p className="text-sm text-muted-foreground">{t("account.subtitle")}</p>
      </div>

      <form className="flex flex-col gap-4" onSubmit={(e) => void submit(e)}>
        <div className="flex flex-col gap-2">
          <Label htmlFor="account-email">{t("account.email_label")}</Label>
          <Input
            id="account-email"
            type="email"
            autoComplete="email"
            required
            value={email}
            onChange={(e) => setEmail(e.target.value)}
          />
        </div>
        <div className="flex flex-col gap-2">
          <Label htmlFor="account-password">{t("account.password_label")}</Label>
          <Input
            id="account-password"
            type="password"
            autoComplete={mode === "signup" ? "new-password" : "current-password"}
            required
            minLength={6}
            value={password}
            onChange={(e) => setPassword(e.target.value)}
          />
        </div>

        {error && <p className="text-sm text-destructive">{error}</p>}
        {info && <p className="text-sm text-muted-foreground">{info}</p>}

        <div className="flex items-center gap-2">
          <Button type="submit" disabled={submitting}>
            {mode === "signup" ? t("account.sign_up") : t("account.sign_in")}
          </Button>
          <Button
            type="button"
            variant="ghost"
            size="sm"
            onClick={() => {
              setMode((m) => (m === "signup" ? "signin" : "signup"));
              setError(null);
              setInfo(null);
            }}
          >
            {mode === "signup"
              ? t("account.switch_to_signin")
              : t("account.switch_to_signup")}
          </Button>
        </div>
      </form>
    </section>
  );
}
