//! Adaptateur d'injection Linux (Phase 3) — point le plus fragile du projet.
//!
//! TODO : détecter la session au runtime (X11 vs Wayland).
//! - X11 : `XTest`/`xdotool`, fonctionne bien nativement.
//! - Wayland : bloque par design l'injection synthétique pour raisons de
//!   sécurité → passer par `ydotool` (démon `ydotoold` + accès `/dev/uinput`),
//!   avec flux d'installation/permission guidé pour l'utilisateur (à traiter
//!   dès la conception plutôt qu'en portage tardif, voir analyse section 5).

use super::TextInjector;

#[allow(dead_code)]
pub enum LinuxDisplayServer {
    X11,
    Wayland,
}

#[allow(dead_code)]
pub struct LinuxInjector {
    pub display_server: LinuxDisplayServer,
}

impl TextInjector for LinuxInjector {
    fn inject(&self, _text: &str) -> Result<(), String> {
        // TODO: router vers XTest (X11) ou ydotool (Wayland).
        Err("not implemented".into())
    }
}
