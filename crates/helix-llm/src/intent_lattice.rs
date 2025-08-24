use once_cell::sync::Lazy;
use regex::Regex;

/// Cached regex for universal quantifiers.
///
/// Matches common universal quantifier words such as "all", "every", "each" and "any".
static UNIVERSAL_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\b(all|every|each|any)\b").expect("valid universal regex"));

/// Cached regex for existential quantifiers.
///
/// Matches common existential quantifier words such as "some", "any", "exists", "a" and "an".
static EXISTENTIAL_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\b(some|any|exists|a|an)\b").expect("valid existential regex"));

/// Types of quantifiers that can be detected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quantifier {
    /// Represents universal quantifiers like "all" or "every".
    Universal,
    /// Represents existential quantifiers like "some" or "exists".
    Existential,
}

/// Determine whether the provided text contains a universal or existential quantifier.
///
/// Returns `Quantifier::Universal` if a universal quantifier is found,
/// `Quantifier::Existential` if an existential quantifier is found,
/// and `None` otherwise.
pub fn detect_quantifier(text: &str) -> Option<Quantifier> {
    if UNIVERSAL_REGEX.is_match(text) {
        Some(Quantifier::Universal)
    } else if EXISTENTIAL_REGEX.is_match(text) {
        Some(Quantifier::Existential)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_universal_quantifier() {
        let text = "all agents must comply";
        assert_eq!(detect_quantifier(text), Some(Quantifier::Universal));
    }

    #[test]
    fn detects_existential_quantifier() {
        let text = "some agents may comply";
        assert_eq!(detect_quantifier(text), Some(Quantifier::Existential));
    }

    #[test]
    fn detects_none_when_no_quantifier_present() {
        let text = "agents comply";
        assert_eq!(detect_quantifier(text), None);
    }
}
