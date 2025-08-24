use once_cell::sync::Lazy;
use regex::Regex;

/// Temporal relationship between condition and outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemporalFacet {
    /// Always true.
    Always,
    /// Eventually becomes true.
    Eventually,
    /// Happens immediately on next step.
    Immediate,
}

/// Quantifier scope of the statement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuantifierFacet {
    /// Applies to all cases.
    Universal,
    /// Applies to some cases.
    Existential,
}

/// Guard/condition relationship.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuardFacet {
    /// Implication/if-then relationship.
    IfThen,
    /// Negated guard expressed with "unless".
    Unless,
    /// Restrictive guard expressed with "only if".
    OnlyIf,
}

/// Facets extracted from natural language.
#[derive(Debug, Default, Clone)]
pub struct IntentFacets {
    /// Temporal aspect.
    pub temporal: Option<TemporalFacet>,
    /// Quantifier aspect.
    pub quantifier: Option<QuantifierFacet>,
    /// Guard/condition aspect.
    pub guard: Option<GuardFacet>,
}

impl IntentFacets {
    /// Parse a prompt into intent facets using simple keyword heuristics.
    pub fn parse(prompt: &str) -> Self {
        let mut facets = IntentFacets::default();
        let lower = prompt.to_lowercase();

        if lower.contains("always") {
            facets.temporal = Some(TemporalFacet::Always);
        } else if lower.contains("eventually") {
            facets.temporal = Some(TemporalFacet::Eventually);
        } else if lower.contains("immediately") || lower.contains("next") {
            facets.temporal = Some(TemporalFacet::Immediate);
        }

        static UNIVERSAL: Lazy<Regex> = Lazy::new(|| Regex::new(r"\b(all|every|each)\b").unwrap());
        static EXISTENTIAL: Lazy<Regex> =
            Lazy::new(|| Regex::new(r"\b(some|any|there exists|exists)\b").unwrap());
        if UNIVERSAL.is_match(&lower) {
            facets.quantifier = Some(QuantifierFacet::Universal);
        } else if EXISTENTIAL.is_match(&lower) {
            facets.quantifier = Some(QuantifierFacet::Existential);
        }

        static IF_THEN: Lazy<Regex> = Lazy::new(|| Regex::new(r"\b(if|when)\b").unwrap());
        static UNLESS: Lazy<Regex> = Lazy::new(|| Regex::new(r"\bunless\b").unwrap());
        static ONLY_IF: Lazy<Regex> = Lazy::new(|| Regex::new(r"\bonly if\b").unwrap());

        if ONLY_IF.is_match(&lower) {
            facets.guard = Some(GuardFacet::OnlyIf);
        } else if UNLESS.is_match(&lower) {
            facets.guard = Some(GuardFacet::Unless);
        } else if IF_THEN.is_match(&lower) {
            facets.guard = Some(GuardFacet::IfThen);
        }

        facets
    }

    /// Generate clarifying questions for missing facets.
    pub fn clarifying_questions(&self) -> Vec<String> {
        let mut qs = Vec::new();
        if self.temporal.is_none() {
            qs.push(
                "Is this rule always true, eventually true, or immediately after the condition?"
                    .to_string(),
            );
        }
        if self.quantifier.is_none() {
            qs.push("Should the rule apply to all cases or only to some?".to_string());
        }
        if self.guard.is_none() {
            qs.push(
                "Does the statement include a condition such as 'if', 'when', 'unless', or 'only if'?".to_string(),
            );
        }
        qs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_facets_and_questions() {
        let facets = IntentFacets::parse("If a button is pressed, the light eventually turns on");
        assert_eq!(facets.guard, Some(GuardFacet::IfThen));
        assert_eq!(facets.temporal, Some(TemporalFacet::Eventually));
        assert_eq!(facets.quantifier, None);

        let qs = facets.clarifying_questions();
        assert!(qs.iter().any(|q| q.contains("apply to all")));
    }

    #[test]
    fn recognizes_guard_keywords() {
        let unless = IntentFacets::parse("Turn on the alarm unless the system is in maintenance");
        assert_eq!(unless.guard, Some(GuardFacet::Unless));

        let only_if = IntentFacets::parse("Alert only if the sensor fails");
        assert_eq!(only_if.guard, Some(GuardFacet::OnlyIf));
    }
}
