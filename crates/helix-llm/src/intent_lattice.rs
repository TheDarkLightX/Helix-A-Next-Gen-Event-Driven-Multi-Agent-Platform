use regex::Regex;

/// Different types of guard conditions that can modify intent execution.
#[derive(Debug, Clone, PartialEq)]
pub enum GuardFacet {
    /// Trigger only if a condition is met.
    If,
    /// Trigger when a condition occurs.
    When,
    /// Prevent action unless a condition is met.
    Unless,
    /// Trigger only if specific criteria are satisfied.
    OnlyIf,
}

/// Detect guard facets in the provided text using word-boundary regexes.
pub fn detect_guard_facets(input: &str) -> Vec<GuardFacet> {
    let lower = input.to_lowercase();
    let mut facets = Vec::new();

    let only_if_re = Regex::new(r"\bonly\s+if\b").unwrap();
    if only_if_re.is_match(&lower) {
        facets.push(GuardFacet::OnlyIf);
    }

    let if_re = Regex::new(r"\bif\b").unwrap();
    if if_re.is_match(&lower) && !only_if_re.is_match(&lower) {
        facets.push(GuardFacet::If);
    }

    let when_re = Regex::new(r"\bwhen\b").unwrap();
    if when_re.is_match(&lower) {
        facets.push(GuardFacet::When);
    }

    let unless_re = Regex::new(r"\bunless\b").unwrap();
    if unless_re.is_match(&lower) {
        facets.push(GuardFacet::Unless);
    }

    facets
}

/// Generate clarifying questions for detected guard facets.
pub fn clarifying_questions(facets: &[GuardFacet]) -> Vec<String> {
    facets
        .iter()
        .map(|facet| match facet {
            GuardFacet::If => "What conditions must be met for this to proceed?".to_string(),
            GuardFacet::When => "When should this action be triggered?".to_string(),
            GuardFacet::Unless => "Under what circumstances should this be skipped?".to_string(),
            GuardFacet::OnlyIf => "Which criteria must hold true before this runs?".to_string(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_if_and_when() {
        let facets = detect_guard_facets("Notify me if it rains and when the sun rises");
        assert!(facets.contains(&GuardFacet::If));
        assert!(facets.contains(&GuardFacet::When));
    }

    #[test]
    fn detects_unless() {
        let facets = detect_guard_facets("Start the process unless the system is locked");
        assert_eq!(facets, vec![GuardFacet::Unless]);
    }

    #[test]
    fn detects_only_if() {
        let facets = detect_guard_facets("Deploy only if all tests pass");
        assert_eq!(facets, vec![GuardFacet::OnlyIf]);
    }

    #[test]
    fn generates_clarifying_questions() {
        let facets = vec![
            GuardFacet::If,
            GuardFacet::When,
            GuardFacet::Unless,
            GuardFacet::OnlyIf,
        ];
        let questions = clarifying_questions(&facets);
        assert!(questions.iter().any(|q| q.contains("conditions")));
        assert!(questions.iter().any(|q| q.contains("When")));
        assert!(questions.iter().any(|q| q.contains("skipped")));
        assert!(questions.iter().any(|q| q.contains("criteria")));
    }
}
