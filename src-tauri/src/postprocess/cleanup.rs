//! Nettoyage local du texte transcrit : application du dictionnaire
//! personnalisé, ponctuation, grammaire légère, suppression des hésitations
//! ("euh", faux départs).
//!
//! Phase 1 (Spec_Backend_Desktop.md §1.4, "nettoyage basique uniquement") :
//! règles simples sans dépendance externe — trim, suppression d'une liste
//! statique d'hésitations, capitalisation de la première lettre, ponctuation
//! finale.
//!
//! Phase 2 §2.2 : le dictionnaire personnalisé (`storage::dictionary`) est
//! appliqué **en premier**, avant les règles ci-dessus. `clean` reçoit les
//! paires `(from, to)` en paramètre (chargées depuis SQLite par
//! `commands::run_pipeline`) — la fonction reste synchrone, pure et
//! testable. Le LLM local (§2.3) reste à venir.

use regex::{NoExpand, Regex};
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

/// Applique les remplacements du dictionnaire personnalisé (§2.2). Pour
/// chaque paire `(from, to)` : correspondance insensible à la casse, aux
/// frontières de mot, tolérante aux variations d'espacement / de trait
/// d'union entre les mots de `from` (« type script », « type-script »,
/// « type  script » → tous remplacés). C'est le "fuzzy-matching léger"
/// demandé par la spec ; un rapprochement par distance d'édition reste une
/// amélioration ultérieure possible sans changer cette interface.
fn apply_replacements(text: &str, replacements: &[(String, String)]) -> String {
    let mut out = text.to_string();
    for (from, to) in replacements {
        let tokens: Vec<String> = from.split_whitespace().map(regex::escape).collect();
        if tokens.is_empty() {
            continue;
        }
        let pattern = format!(r"(?i)\b{}\b", tokens.join(r"[\s\-]+"));
        match Regex::new(&pattern) {
            Ok(re) => out = re.replace_all(&out, NoExpand(to)).into_owned(),
            // Une entrée pathologique (ex. `from` = "c++") ne doit pas
            // casser tout le nettoyage — on la saute en le signalant.
            Err(e) => eprintln!("[postprocess] entrée de dictionnaire ignorée (« {from} ») : {e}"),
        }
    }
    out
}

pub fn clean(raw_text: &str, replacements: &[(String, String)]) -> String {
    let trimmed = raw_text.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    // Dictionnaire personnalisé d'abord (§2.2 : "avant les autres règles").
    let substituted = if replacements.is_empty() {
        trimmed.to_string()
    } else {
        apply_replacements(trimmed, replacements)
    };

    let without_fillers = FILLER_RE.replace_all(&substituted, "");
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

    /// Raccourci : `clean` sans dictionnaire (comportement des règles seules).
    fn clean_rules(raw: &str) -> String {
        clean(raw, &[])
    }

    fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
        list.iter().map(|(f, t)| (f.to_string(), t.to_string())).collect()
    }

    #[test]
    fn trims_and_capitalizes_and_punctuates() {
        assert_eq!(clean_rules("  bonjour tout le monde  "), "Bonjour tout le monde.");
    }

    #[test]
    fn keeps_existing_terminal_punctuation() {
        assert_eq!(clean_rules("comment allez-vous ?"), "Comment allez-vous ?");
        assert_eq!(clean_rules("attention !"), "Attention !");
    }

    #[test]
    fn removes_filler_words_case_insensitive() {
        assert_eq!(clean_rules("je pense que euh c'est bien"), "Je pense que c'est bien.");
        assert_eq!(clean_rules("Euh, on commence"), "On commence.");
        assert_eq!(clean_rules("hum je sais pas"), "Je sais pas.");
    }

    #[test]
    fn does_not_strip_words_containing_filler_as_substring() {
        // "heureux" contient "heu" mais n'est pas un mot d'hésitation isolé.
        assert_eq!(clean_rules("il est heureux"), "Il est heureux.");
    }

    #[test]
    fn removes_english_fillers_too() {
        assert_eq!(clean_rules("so um I think this works"), "So I think this works.");
    }

    #[test]
    fn capitalizes_accented_first_letter() {
        assert_eq!(clean_rules("école obligatoire"), "École obligatoire.");
    }

    #[test]
    fn empty_input_stays_empty() {
        assert_eq!(clean_rules(""), "");
        assert_eq!(clean_rules("   "), "");
    }

    #[test]
    fn only_fillers_collapses_to_empty() {
        assert_eq!(clean_rules("euh euh hum"), "");
    }

    // --- Dictionnaire personnalisé (§2.2) ---

    #[test]
    fn applies_exact_dictionary_replacement() {
        // Le critère d'acceptation littéral de la spec.
        let dict = pairs(&[("type script", "TypeScript")]);
        assert_eq!(
            clean("j'écris du type script au quotidien", &dict),
            "J'écris du TypeScript au quotidien."
        );
    }

    #[test]
    fn dictionary_match_is_case_insensitive_and_spacing_tolerant() {
        let dict = pairs(&[("type script", "TypeScript")]);
        assert_eq!(clean("Type Script c'est bien", &dict), "TypeScript c'est bien.");
        assert_eq!(clean("du type-script partout", &dict), "Du TypeScript partout.");
        assert_eq!(clean("du type  script partout", &dict), "Du TypeScript partout.");
    }

    #[test]
    fn dictionary_respects_word_boundaries() {
        let dict = pairs(&[("js", "JavaScript")]);
        // "js" isolé -> remplacé ; "jsp" ne l'est pas.
        assert_eq!(clean("le js moderne", &dict), "Le JavaScript moderne.");
        assert_eq!(clean("jsp ce que c'est", &dict), "Jsp ce que c'est.");
    }

    #[test]
    fn dictionary_applies_before_rules() {
        // Remplacement, puis suppression d'hésitation, puis capitalisation.
        let dict = pairs(&[("vozel", "Vozel")]);
        assert_eq!(clean("euh vozel est prêt", &dict), "Vozel est prêt.");
    }

    #[test]
    fn dictionary_replacement_value_is_literal() {
        // Un `$` dans la cible ne doit pas être interprété comme une
        // référence de capture regex.
        let dict = pairs(&[("prix", "$5")]);
        assert_eq!(clean("le prix affiché", &dict), "Le $5 affiché.");
    }

    #[test]
    fn no_matching_entry_leaves_text_unchanged() {
        let dict = pairs(&[("golang", "Go")]);
        assert_eq!(clean("je fais du python", &dict), "Je fais du python.");
    }
}
