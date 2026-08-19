//! Injection du texte transcrit dans l'application active — un adaptateur
//! par OS, avec repli commun sur le presse-papiers (voir analyse section 4.5,
//! le nœud technique le plus sensible du projet).
//!
//! Ordre de portage : Windows (Phase 1) → macOS (Phase 3) → Linux (Phase 3,
//! X11 + Wayland/`ydotool`, point d'attention prioritaire).

pub mod clipboard;

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "linux")]
pub mod linux;

/// Interface commune d'injection, implémentée par chaque adaptateur OS.
pub trait TextInjector {
    fn inject(&self, text: &str) -> Result<(), String>;
}
