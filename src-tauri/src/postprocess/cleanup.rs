//! Nettoyage local du texte transcrit : ponctuation, grammaire légère,
//! suppression des hésitations ("euh", faux départs), application du
//! dictionnaire personnalisé (voir `storage::dictionary`).
//!
//! Phase 1 (Spec_Backend_Desktop.md §1.4, "nettoyage basique uniquement") :
//! implémentation par règles simples, sans dépendance externe — trim,
//! suppression d'une liste statique d'hésitations, capitalisation de la
//! première lettre, ponctuation finale. Le dictionnaire personnalisé et le
//! LLM local (Qwen2.5-1.5B-Instruct ou Phi-3-mini via llama.cpp) restent
//! Phase 2 (§2.2, §2.3) — pas implémentés ici, conformément à la spec
//! ("Ne pas implémenter le LLM local à ce stade").

use regex::Regex;
use std::sync::LazyLock;

/// Hésitations fréquentes retirées par une liste statique (Spec_Backend_Desktop.md
/// §1.4 point 1, qui cite explicitement "euh"/"hum"). Français en priorité
/// (positionnement marché francophone du produit), complété de quelques
/// équivalents anglais courants vu que le moteur ASR retenu (§1.3,
/// Parakeet-TDT) est multilingue et peut transcrire de l'anglais.
const FILLER_WORDS: &[&str] = &["euh", "heu", "hum", "hmm", "uh", "um", "uhh", "umm"];

static FILLER_RE: LazyLock<Regex> = LazyLock::new(|| {
    let alternation = FILLER_WORDS.join("|");
    Regex::new(&format!(r"(?i)\b(?:{alternation})\b[,]?")).expect("regex d'hésitations valide (construite depuis FILLER_WORDS)")
});

static EXTRA_SPACES_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[ \t]{2,}").expect("regex d'espaces valide"));

/// Ponctuation considérée comme "finale" — n'ajoute pas de point si le texte
/// en a déjà une.
fn ends_with_terminal_punctuation(text: &str) -> bool {
    matches!(text.chars().last(), Some('.' | '!' | '?' | '…' | ':' | ';'))
}

pub fn clean(raw_text: &str) -> String {
    let trimmed = raw_text.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let without_fillers = FILLER_RE.replace_all(trimmed, "");
    let mut text = EXTRA_SPACES_RE.replace_all(&without_fillers, " ").trim().to_string();

    if text.is_empty() {
        return text;
    }

    // Capitalisation de la première lettre — Unicode-safe (peut être
    // accentuée, ex. "école" -> "École"), pas une simple maj ASCII.
    let mut chars = text.chars();
    if let Some(first) = chars.next() {
        let capitalized_first: String = first.to_uppercase().collect();
        text = capitalized_first + chars.as_str();
    }

    if !ends_with_terminal_punctuation(&text) {
        text.push('.');
    }

    text
}

#[cfg(test)]
mod tests {
    use super::clean;

    #[test]
    fn trims_and_capitalizes_and_punctuates() {
        assert_eq!(clean("  bonjour tout le monde  "), "Bonjour tout le monde.");
    }

    #[test]
    fn keeps_existing_terminal_punctuation() {
        assert_eq!(clean("comment allez-vous ?"), "Comment allez-vous ?");
        assert_eq!(clean("attention !"), "Attention !");
    }

    #[test]
    fn removes_filler_words_case_insensitive() {
        assert_eq!(clean("je pense que euh c'est bien"), "Je pense que c'est bien.");
        assert_eq!(clean("Euh, on commence"), "On commence.");
        assert_eq!(clean("hum je sais pas"), "Je sais pas.");
    }

    #[test]
    fn does_not_strip_words_containing_filler_as_substring() {
        // "heureux" contient "heu" mais n'est pas un mot d'hésitation isolé.
        assert_eq!(clean("il est heureux"), "Il est heureux.");
    }

    #[test]
    fn removes_english_fillers_too() {
        assert_eq!(clean("so um I think this works"), "So I think this works.");
    }

    #[test]
    fn capitalizes_accented_first_letter() {
        assert_eq!(clean("école obligatoire"), "École obligatoire.");
    }

    #[test]
    fn empty_input_stays_empty() {
        assert_eq!(clean(""), "");
        assert_eq!(clean("   "), "");
    }

    #[test]
    fn only_fillers_collapses_to_empty() {
        assert_eq!(clean("euh euh hum"), "");
    }
}
