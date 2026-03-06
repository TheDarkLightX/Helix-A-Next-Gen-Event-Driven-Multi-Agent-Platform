// Copyright 2026 DarkLightX
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use super::{
    normalize_non_empty, KrrTriple, ReasoningContradiction, ReasoningSupportKind,
    ReasoningSupportNode, SymbolicRule,
};
use crate::HelixError;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SymbolicEvaluationScope {
    Full,
    QueryDirected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SymbolicClosureStatus {
    Saturated,
    Truncated,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct FactId(usize);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
enum SymbolicTerm {
    Atom(String),
}

impl SymbolicTerm {
    fn canonical_string(&self) -> &str {
        match self {
            Self::Atom(value) => value.as_str(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
enum SymbolicAtom {
    Atom(String),
    Predicate {
        name: String,
        args: Vec<SymbolicTerm>,
    },
}

impl SymbolicAtom {
    fn canonical_string(&self) -> String {
        match self {
            Self::Atom(value) => value.clone(),
            Self::Predicate { name, args } => {
                let args = args
                    .iter()
                    .map(SymbolicTerm::canonical_string)
                    .collect::<Vec<_>>()
                    .join(",");
                format!("{name}({args})")
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
enum LiteralPolarity {
    Positive,
    Negative,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
struct SymbolicLiteral {
    polarity: LiteralPolarity,
    atom: SymbolicAtom,
}

impl SymbolicLiteral {
    fn canonical_string(&self) -> String {
        match self.polarity {
            LiteralPolarity::Positive => self.atom.canonical_string(),
            LiteralPolarity::Negative => format!("not {}", self.atom.canonical_string()),
        }
    }

    fn contradiction_key(&self) -> String {
        self.atom.canonical_string()
    }
}

#[derive(Debug, Clone, Default)]
struct SymbolTable {
    ids: BTreeMap<SymbolicLiteral, FactId>,
    facts: Vec<SymbolicLiteral>,
}

impl SymbolTable {
    fn intern(&mut self, fact: SymbolicLiteral) -> FactId {
        if let Some(id) = self.ids.get(&fact) {
            return id.clone();
        }

        let id = FactId(self.facts.len());
        self.facts.push(fact.clone());
        self.ids.insert(fact, id.clone());
        id
    }

    fn resolve(&self, id: &FactId) -> &SymbolicLiteral {
        &self.facts[id.0]
    }
}

#[derive(Debug, Clone)]
struct CompiledSymbolicRule {
    id: String,
    antecedents: Vec<FactId>,
    consequent: FactId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SupportRecord {
    Seed,
    Derived {
        rule_id: String,
        supports: Vec<FactId>,
    },
}

#[derive(Debug, Clone)]
pub struct CompiledSymbolicProgram {
    symbols: SymbolTable,
    base_facts: BTreeSet<FactId>,
    rules: Vec<CompiledSymbolicRule>,
    dependent_rules: BTreeMap<FactId, Vec<usize>>,
    rules_by_consequent: BTreeMap<FactId, Vec<usize>>,
    fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SymbolicEvaluation {
    pub(crate) derived_facts: Vec<String>,
    pub(crate) matched_rules: Vec<String>,
    pub(crate) entailed: bool,
    pub(crate) support_graph: Vec<ReasoningSupportNode>,
    pub(crate) contradictions: Vec<ReasoningContradiction>,
    pub(crate) query_support: Vec<String>,
    pub(crate) closure_status: SymbolicClosureStatus,
    pub(crate) rounds_executed: usize,
    pub(crate) pending_rule_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct CanonicalRuleFingerprint {
    id: String,
    antecedents: Vec<String>,
    consequent: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct CanonicalProgramFingerprint {
    rules: Vec<CanonicalRuleFingerprint>,
    triples: Vec<String>,
}

pub(crate) fn evaluate_compiled_symbolic(
    program: &CompiledSymbolicProgram,
    query: String,
    facts: Vec<String>,
    max_rounds: usize,
    scope: SymbolicEvaluationScope,
) -> Result<SymbolicEvaluation, HelixError> {
    program.evaluate(query, facts, max_rounds, scope)
}

pub fn fingerprint_symbolic_program(
    rules: &[SymbolicRule],
    triples: &[KrrTriple],
) -> Result<String, HelixError> {
    let descriptor = build_canonical_program_descriptor(rules, triples)?;
    hash_canonical_program(&descriptor)
}

impl CompiledSymbolicProgram {
    pub fn compile(rules: Vec<SymbolicRule>, triples: Vec<KrrTriple>) -> Result<Self, HelixError> {
        let fingerprint = fingerprint_symbolic_program(&rules, &triples)?;
        let mut symbols = SymbolTable::default();
        let mut base_facts = BTreeSet::new();
        let mut rules_by_consequent: BTreeMap<FactId, Vec<usize>> = BTreeMap::new();

        for triple in triples {
            let fact = parse_triple_fact(&triple)?;
            base_facts.insert(symbols.intern(fact));
        }

        let mut compiled_rules = Vec::with_capacity(rules.len());
        let mut dependent_rules: BTreeMap<FactId, Vec<usize>> = BTreeMap::new();

        for rule in rules {
            let id = normalize_non_empty(&rule.id, "reasoning.rules", "rule id")?;
            let consequent = parse_fact(&rule.consequent, "reasoning.rules", "rule consequent")?;
            let consequent_id = symbols.intern(consequent);
            let antecedents = dedupe_antecedents(rule.antecedents, &mut symbols)?;
            let rule_index = compiled_rules.len();
            rules_by_consequent
                .entry(consequent_id.clone())
                .or_default()
                .push(rule_index);
            for antecedent in &antecedents {
                dependent_rules
                    .entry(antecedent.clone())
                    .or_default()
                    .push(rule_index);
            }
            compiled_rules.push(CompiledSymbolicRule {
                id,
                antecedents,
                consequent: consequent_id,
            });
        }

        Ok(Self {
            symbols,
            base_facts,
            rules: compiled_rules,
            dependent_rules,
            rules_by_consequent,
            fingerprint,
        })
    }

    pub fn fingerprint(&self) -> &str {
        self.fingerprint.as_str()
    }

    pub(crate) fn evaluate(
        &self,
        query: String,
        facts: Vec<String>,
        max_rounds: usize,
        scope: SymbolicEvaluationScope,
    ) -> Result<SymbolicEvaluation, HelixError> {
        let mut symbols = self.symbols.clone();
        let mut closure = self.base_facts.clone();
        let mut support_records: BTreeMap<FactId, SupportRecord> = closure
            .iter()
            .cloned()
            .map(|fact| (fact, SupportRecord::Seed))
            .collect();
        let query = symbols.intern(parse_fact(&query, "reasoning.query", "query")?);
        let active_rules = self.active_rules(&query, scope);
        let mut missing_counts: Vec<usize> = self
            .rules
            .iter()
            .enumerate()
            .map(|(rule_index, rule)| {
                if active_rules[rule_index] {
                    rule.antecedents.len()
                } else {
                    usize::MAX
                }
            })
            .collect();
        let mut matched_rules = Vec::new();

        for fact in facts {
            let parsed = parse_fact(&fact, "reasoning.facts", "fact entries")?;
            let fact_id = symbols.intern(parsed);
            closure.insert(fact_id.clone());
            support_records
                .entry(fact_id.clone())
                .or_insert(SupportRecord::Seed);
        }

        for fact in &closure {
            self.satisfy_fact(fact, &active_rules, &mut missing_counts);
        }

        let mut current_ready = self.initial_ready_rules(&closure, &active_rules, &missing_counts);
        let mut rounds_executed = 0usize;

        while rounds_executed < max_rounds {
            if current_ready.is_empty() {
                break;
            }

            rounds_executed += 1;
            let mut changed = false;
            let mut next_round_ready = BTreeSet::new();
            let mut cursor = 0usize;
            while let Some(rule_index) = pop_next_ready_rule(&mut current_ready, cursor) {
                cursor = rule_index.saturating_add(1);
                let rule = &self.rules[rule_index];
                if !active_rules[rule_index]
                    || missing_counts[rule_index] != 0
                    || closure.contains(&rule.consequent)
                {
                    continue;
                }

                changed = true;
                closure.insert(rule.consequent.clone());
                matched_rules.push(rule.id.clone());
                support_records.insert(
                    rule.consequent.clone(),
                    SupportRecord::Derived {
                        rule_id: rule.id.clone(),
                        supports: rule.antecedents.clone(),
                    },
                );
                self.propagate_consequent(
                    rule_index,
                    &rule.consequent,
                    &closure,
                    &active_rules,
                    &mut missing_counts,
                    &mut next_round_ready,
                );
            }

            if !changed {
                current_ready.clear();
                break;
            }
            current_ready = next_round_ready;
        }

        let pending_rule_count = current_ready.len();
        let closure_status = if pending_rule_count == 0 {
            SymbolicClosureStatus::Saturated
        } else {
            SymbolicClosureStatus::Truncated
        };

        Ok(SymbolicEvaluation {
            derived_facts: self.sorted_facts(&symbols, &closure),
            matched_rules,
            entailed: closure.contains(&query),
            query_support: build_query_support(&symbols, &support_records, &query),
            support_graph: build_support_graph(&symbols, &support_records),
            contradictions: detect_contradictions(&symbols, &closure),
            closure_status,
            rounds_executed,
            pending_rule_count,
        })
    }

    fn satisfy_fact(&self, fact: &FactId, active_rules: &[bool], missing_counts: &mut [usize]) {
        if let Some(rule_indexes) = self.dependent_rules.get(fact) {
            for &rule_index in rule_indexes {
                if active_rules[rule_index] && missing_counts[rule_index] > 0 {
                    missing_counts[rule_index] -= 1;
                }
            }
        }
    }

    fn initial_ready_rules(
        &self,
        closure: &BTreeSet<FactId>,
        active_rules: &[bool],
        missing_counts: &[usize],
    ) -> BTreeSet<usize> {
        self.rules
            .iter()
            .enumerate()
            .filter_map(|(rule_index, rule)| {
                if active_rules[rule_index]
                    && missing_counts[rule_index] == 0
                    && !closure.contains(&rule.consequent)
                {
                    Some(rule_index)
                } else {
                    None
                }
            })
            .collect()
    }

    fn propagate_consequent(
        &self,
        _source_rule_index: usize,
        consequent: &FactId,
        closure: &BTreeSet<FactId>,
        active_rules: &[bool],
        missing_counts: &mut [usize],
        next_round_ready: &mut BTreeSet<usize>,
    ) {
        if let Some(rule_indexes) = self.dependent_rules.get(consequent) {
            for &rule_index in rule_indexes {
                if !active_rules[rule_index] {
                    continue;
                }
                if missing_counts[rule_index] > 0 {
                    missing_counts[rule_index] -= 1;
                }

                if missing_counts[rule_index] == 0
                    && !closure.contains(&self.rules[rule_index].consequent)
                {
                    next_round_ready.insert(rule_index);
                }
            }
        }
    }

    fn active_rules(&self, query: &FactId, scope: SymbolicEvaluationScope) -> Vec<bool> {
        match scope {
            SymbolicEvaluationScope::Full => vec![true; self.rules.len()],
            SymbolicEvaluationScope::QueryDirected => {
                let relevant = self.relevant_rules_for_query(query);
                (0..self.rules.len())
                    .map(|rule_index| relevant.contains(&rule_index))
                    .collect()
            }
        }
    }

    fn relevant_rules_for_query(&self, query: &FactId) -> BTreeSet<usize> {
        let mut relevant_rules = BTreeSet::new();
        let mut pending_facts = vec![query.clone()];
        let mut visited_facts = BTreeSet::new();

        while let Some(fact) = pending_facts.pop() {
            if !visited_facts.insert(fact.clone()) {
                continue;
            }
            if let Some(rule_indexes) = self.rules_by_consequent.get(&fact) {
                for &rule_index in rule_indexes {
                    if relevant_rules.insert(rule_index) {
                        pending_facts.extend(self.rules[rule_index].antecedents.iter().cloned());
                    }
                }
            }
        }

        relevant_rules
    }

    fn sorted_facts(&self, symbols: &SymbolTable, closure: &BTreeSet<FactId>) -> Vec<String> {
        let mut facts: Vec<String> = closure
            .iter()
            .map(|fact| symbols.resolve(fact).canonical_string())
            .collect();
        facts.sort();
        facts
    }
}

fn parse_fact(value: &str, context: &str, field: &str) -> Result<SymbolicLiteral, HelixError> {
    let normalized = normalize_non_empty(value, context, field)?;
    let (polarity, atom_source) = parse_literal_polarity(&normalized);
    Ok(SymbolicLiteral {
        polarity,
        atom: parse_atom(atom_source, context, field)?,
    })
}

fn parse_atom(value: &str, context: &str, _field: &str) -> Result<SymbolicAtom, HelixError> {
    if !looks_like_predicate(value) {
        return Ok(SymbolicAtom::Atom(value.to_string()));
    }

    let Some(open_idx) = value.find('(') else {
        return Ok(SymbolicAtom::Atom(value.to_string()));
    };

    if !value.ends_with(')') || value[..open_idx].contains('(') {
        return Ok(SymbolicAtom::Atom(value.to_string()));
    }

    let name = normalize_non_empty(&value[..open_idx], context, "fact predicate name")?;
    let inner = &value[open_idx + 1..value.len() - 1];
    if inner.is_empty() {
        return Ok(SymbolicAtom::Predicate {
            name,
            args: Vec::new(),
        });
    }

    if inner.contains('(') || inner.contains(')') {
        return Ok(SymbolicAtom::Atom(value.to_string()));
    }

    let mut args = Vec::new();
    for term in inner.split(',') {
        let normalized_term = normalize_non_empty(term, context, "fact argument")?;
        args.push(SymbolicTerm::Atom(normalized_term));
    }

    Ok(SymbolicAtom::Predicate { name, args })
}

fn parse_triple_fact(triple: &KrrTriple) -> Result<SymbolicLiteral, HelixError> {
    let predicate =
        normalize_non_empty(&triple.predicate, "reasoning.triples", "triple predicate")?;
    let subject = normalize_non_empty(&triple.subject, "reasoning.triples", "triple subject")?;
    let object = normalize_non_empty(&triple.object, "reasoning.triples", "triple object")?;
    Ok(SymbolicLiteral {
        polarity: LiteralPolarity::Positive,
        atom: SymbolicAtom::Predicate {
            name: predicate,
            args: vec![SymbolicTerm::Atom(subject), SymbolicTerm::Atom(object)],
        },
    })
}

fn parse_literal_polarity(value: &str) -> (LiteralPolarity, &str) {
    if let Some(rest) = value.strip_prefix('!') {
        (LiteralPolarity::Negative, rest.trim())
    } else if let Some(rest) = value.strip_prefix("not ") {
        (LiteralPolarity::Negative, rest.trim())
    } else {
        (LiteralPolarity::Positive, value)
    }
}

fn looks_like_predicate(value: &str) -> bool {
    value.contains('(') || value.ends_with(')')
}

fn dedupe_antecedents(
    antecedents: Vec<String>,
    symbols: &mut SymbolTable,
) -> Result<Vec<FactId>, HelixError> {
    let mut seen = BTreeSet::new();
    let mut normalized = Vec::new();
    for antecedent in antecedents {
        let fact = parse_fact(&antecedent, "reasoning.rules", "rule antecedent")?;
        let fact_id = symbols.intern(fact);
        if seen.insert(fact_id.clone()) {
            normalized.push(fact_id);
        }
    }
    Ok(normalized)
}

fn build_support_graph(
    symbols: &SymbolTable,
    support_records: &BTreeMap<FactId, SupportRecord>,
) -> Vec<ReasoningSupportNode> {
    let mut graph: Vec<ReasoningSupportNode> = support_records
        .iter()
        .map(|(fact, record)| match record {
            SupportRecord::Seed => ReasoningSupportNode {
                fact: symbols.resolve(fact).canonical_string(),
                kind: ReasoningSupportKind::Seed,
                rule_id: None,
                supports: Vec::new(),
            },
            SupportRecord::Derived { rule_id, supports } => ReasoningSupportNode {
                fact: symbols.resolve(fact).canonical_string(),
                kind: ReasoningSupportKind::Derived,
                rule_id: Some(rule_id.clone()),
                supports: supports
                    .iter()
                    .map(|support| symbols.resolve(support).canonical_string())
                    .collect(),
            },
        })
        .collect();

    graph.sort_by(|left, right| left.fact.cmp(&right.fact));
    graph
}

fn build_query_support(
    symbols: &SymbolTable,
    support_records: &BTreeMap<FactId, SupportRecord>,
    query: &FactId,
) -> Vec<String> {
    if !support_records.contains_key(query) {
        return Vec::new();
    }

    let mut stack = vec![query.clone()];
    let mut visited = BTreeSet::new();
    while let Some(fact) = stack.pop() {
        if !visited.insert(fact.clone()) {
            continue;
        }

        if let Some(SupportRecord::Derived { supports, .. }) = support_records.get(&fact) {
            stack.extend(supports.iter().cloned());
        }
    }

    let mut support: Vec<String> = visited
        .into_iter()
        .map(|fact| symbols.resolve(&fact).canonical_string())
        .collect();
    support.sort();
    support
}

fn detect_contradictions(
    symbols: &SymbolTable,
    closure: &BTreeSet<FactId>,
) -> Vec<ReasoningContradiction> {
    let mut grouped: BTreeMap<String, (Option<String>, Option<String>)> = BTreeMap::new();
    for fact_id in closure {
        let literal = symbols.resolve(fact_id);
        let entry = grouped
            .entry(literal.contradiction_key())
            .or_insert((None, None));
        match literal.polarity {
            LiteralPolarity::Positive => entry.0 = Some(literal.canonical_string()),
            LiteralPolarity::Negative => entry.1 = Some(literal.canonical_string()),
        }
    }

    grouped
        .into_iter()
        .filter_map(|(_, (positive, negative))| match (positive, negative) {
            (Some(positive), Some(negative)) => Some(ReasoningContradiction { positive, negative }),
            _ => None,
        })
        .collect()
}

fn build_canonical_program_descriptor(
    rules: &[SymbolicRule],
    triples: &[KrrTriple],
) -> Result<CanonicalProgramFingerprint, HelixError> {
    let canonical_rules = rules
        .iter()
        .map(|rule| {
            Ok::<_, HelixError>(CanonicalRuleFingerprint {
                id: normalize_non_empty(&rule.id, "reasoning.rules", "rule id")?,
                antecedents: rule
                    .antecedents
                    .iter()
                    .map(|antecedent| {
                        parse_fact(antecedent, "reasoning.rules", "rule antecedent")
                            .map(|fact| fact.canonical_string())
                    })
                    .collect::<Result<Vec<_>, _>>()?,
                consequent: parse_fact(&rule.consequent, "reasoning.rules", "rule consequent")?
                    .canonical_string(),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    let canonical_triples = triples
        .iter()
        .map(parse_triple_fact)
        .map(|result| result.map(|fact| fact.canonical_string()))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(CanonicalProgramFingerprint {
        rules: canonical_rules,
        triples: canonical_triples,
    })
}

fn hash_canonical_program(descriptor: &CanonicalProgramFingerprint) -> Result<String, HelixError> {
    let bytes = serde_json::to_vec(descriptor)?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

fn pop_next_ready_rule(ready: &mut BTreeSet<usize>, cursor: usize) -> Option<usize> {
    let next = ready.range(cursor..).next().copied()?;
    ready.remove(&next);
    Some(next)
}
