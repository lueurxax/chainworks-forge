use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::definition::{self, WorkflowFile};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SimultaneousTransitionFinding {
    pub workflow_path: String,
    pub state_id: String,
    pub left_transition_index: usize,
    pub left_to: String,
    pub left_condition: String,
    pub right_transition_index: usize,
    pub right_to: String,
    pub right_condition: String,
    pub diagnostic: String,
}

pub fn scan_workflow_file_for_simultaneous_transitions(
    workflow_path: &Path,
) -> Result<Vec<SimultaneousTransitionFinding>> {
    let workflow_path_str = workflow_path
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("workflow path is not valid UTF-8"))?;
    if !is_state_machine_workflow_file(workflow_path)? {
        return Ok(Vec::new());
    }
    let workflow = definition::load(workflow_path_str)
        .with_context(|| format!("load workflow {}", workflow_path.display()))?;
    Ok(scan_workflow_definition_for_simultaneous_transitions(
        &workflow,
        &workflow_path.display().to_string(),
    ))
}

fn is_state_machine_workflow_file(workflow_path: &Path) -> Result<bool> {
    let raw = std::fs::read_to_string(workflow_path)
        .with_context(|| format!("read workflow {}", workflow_path.display()))?;
    let value: serde_yaml::Value = serde_yaml::from_str(&raw)
        .with_context(|| format!("parse workflow {}", workflow_path.display()))?;
    let Some(mapping) = value.as_mapping() else {
        return Ok(false);
    };
    Ok(
        mapping.contains_key(serde_yaml::Value::String("initial_state".to_string()))
            && mapping.contains_key(serde_yaml::Value::String("states".to_string())),
    )
}

pub fn scan_workflow_definition_for_simultaneous_transitions(
    workflow: &WorkflowFile,
    workflow_path: &str,
) -> Vec<SimultaneousTransitionFinding> {
    let mut findings = Vec::new();
    let mut state_ids: Vec<_> = workflow.states.keys().cloned().collect();
    state_ids.sort();

    for state_id in state_ids {
        let Some(transitions) = workflow
            .states
            .get(&state_id)
            .and_then(|state| state.transitions.as_ref())
        else {
            continue;
        };

        for left_index in 0..transitions.len() {
            for right_index in (left_index + 1)..transitions.len() {
                let left = &transitions[left_index];
                let right = &transitions[right_index];
                if conditions_can_match_simultaneously(&left.when, &right.when) {
                    findings.push(SimultaneousTransitionFinding {
                        workflow_path: workflow_path.to_string(),
                        state_id: state_id.clone(),
                        left_transition_index: left_index,
                        left_to: left.to.clone(),
                        left_condition: left.when.clone(),
                        right_transition_index: right_index,
                        right_to: right.to.clone(),
                        right_condition: right.when.clone(),
                        diagnostic: "transition conditions can be true at the same time"
                            .to_string(),
                    });
                }
            }
        }
    }

    findings
}

fn conditions_can_match_simultaneously(left: &str, right: &str) -> bool {
    let left_dnf = parse_condition_dnf(left);
    let right_dnf = parse_condition_dnf(right);
    for left_clause in &left_dnf.clauses {
        for right_clause in &right_dnf.clauses {
            if clauses_can_match_simultaneously(left_clause, right_clause) {
                return true;
            }
        }
    }
    false
}

#[derive(Clone, Debug)]
struct Dnf {
    clauses: Vec<Clause>,
}

#[derive(Clone, Debug)]
struct Clause {
    atoms: Vec<Atom>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Atom {
    field: String,
    op: Op,
    value: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Op {
    Eq,
    Ne,
    Gt,
    Ge,
    Lt,
    Le,
}

fn parse_condition_dnf(condition: &str) -> Dnf {
    let trimmed = condition.trim();
    if trimmed.eq_ignore_ascii_case("true") {
        return Dnf {
            clauses: vec![Clause { atoms: Vec::new() }],
        };
    }

    let clauses = split_bool_word(trimmed, "or")
        .into_iter()
        .map(|or_part| {
            let atoms = split_bool_word(&or_part, "and")
                .into_iter()
                .map(|atom| parse_atom(&atom))
                .collect();
            Clause { atoms }
        })
        .collect();
    Dnf { clauses }
}

fn parse_atom(raw: &str) -> Atom {
    let atom = raw.trim();
    if let Some(name) = parse_exists_artifact(atom) {
        return Atom {
            field: format!("exists({name})"),
            op: Op::Eq,
            value: "true".to_string(),
        };
    }

    if let Some((left, op, right)) = split_comparison(atom) {
        return Atom {
            field: left.trim().to_string(),
            op,
            value: normalize_value(right.trim()),
        };
    }

    Atom {
        field: format!("__unsupported__:{atom}"),
        op: Op::Eq,
        value: "true".to_string(),
    }
}

fn parse_exists_artifact(atom: &str) -> Option<String> {
    let inner = atom
        .strip_prefix("exists(")?
        .strip_suffix(')')?
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .trim()
        .to_string();
    (!inner.is_empty()).then_some(inner)
}

fn split_comparison(atom: &str) -> Option<(&str, Op, &str)> {
    for (needle, op) in [
        ("==", Op::Eq),
        ("!=", Op::Ne),
        (">=", Op::Ge),
        ("<=", Op::Le),
        (">", Op::Gt),
        ("<", Op::Lt),
    ] {
        if let Some(index) = find_outside_quotes(atom, needle) {
            return Some((&atom[..index], op, &atom[index + needle.len()..]));
        }
    }
    None
}

fn split_bool_word(input: &str, word: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut quote: Option<char> = None;
    let bytes = input.as_bytes();
    let word_bytes = word.as_bytes();
    let mut index = 0;

    while index < bytes.len() {
        let ch = input[index..].chars().next().unwrap();
        if matches!(ch, '\'' | '"') {
            quote = match quote {
                Some(current) if current == ch => None,
                None => Some(ch),
                other => other,
            };
            index += ch.len_utf8();
            continue;
        }

        if quote.is_none()
            && index + word_bytes.len() <= bytes.len()
            && input[index..index + word_bytes.len()].eq_ignore_ascii_case(word)
            && is_bool_word_start(bytes, index)
            && is_bool_word_end(bytes, index + word_bytes.len())
        {
            parts.push(input[start..index].trim().to_string());
            start = index + word_bytes.len();
            index = start;
            continue;
        }

        index += ch.len_utf8();
    }

    parts.push(input[start..].trim().to_string());
    parts.retain(|part| !part.is_empty());
    parts
}

fn find_outside_quotes(input: &str, needle: &str) -> Option<usize> {
    let mut quote: Option<char> = None;
    let mut index = 0;
    while index < input.len() {
        let ch = input[index..].chars().next().unwrap();
        if matches!(ch, '\'' | '"') {
            quote = match quote {
                Some(current) if current == ch => None,
                None => Some(ch),
                other => other,
            };
            index += ch.len_utf8();
            continue;
        }
        if quote.is_none() && input[index..].starts_with(needle) {
            return Some(index);
        }
        index += ch.len_utf8();
    }
    None
}

fn is_bool_word_start(bytes: &[u8], index: usize) -> bool {
    index == 0 || !is_condition_word_byte(bytes[index - 1])
}

fn is_bool_word_end(bytes: &[u8], index: usize) -> bool {
    index >= bytes.len() || !is_condition_word_byte(bytes[index])
}

fn is_condition_word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.')
}

fn normalize_value(value: &str) -> String {
    value
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .trim()
        .to_string()
}

fn clauses_can_match_simultaneously(left: &Clause, right: &Clause) -> bool {
    for left_atom in &left.atoms {
        for right_atom in &right.atoms {
            if atoms_contradict(left_atom, right_atom) {
                return false;
            }
        }
    }
    true
}

fn atoms_contradict(left: &Atom, right: &Atom) -> bool {
    if left.field != right.field {
        return mutually_exclusive_boolean_fields(left, right);
    }

    match (left.op, right.op) {
        (Op::Eq, Op::Eq) => left.value != right.value,
        (Op::Eq, Op::Ne) | (Op::Ne, Op::Eq) => left.value == right.value,
        (Op::Eq, op) => !value_satisfies(&left.value, op, &right.value),
        (op, Op::Eq) => !value_satisfies(&right.value, op, &left.value),
        (left_op, right_op) => ranges_contradict(left_op, &left.value, right_op, &right.value),
    }
}

fn mutually_exclusive_boolean_fields(left: &Atom, right: &Atom) -> bool {
    let Some((left_parent, left_leaf)) = left.field.rsplit_once('.') else {
        return false;
    };
    let Some((right_parent, right_leaf)) = right.field.rsplit_once('.') else {
        return false;
    };
    left_parent == right_parent
        && matches!(
            (left_leaf, right_leaf),
            ("granted", "rejected") | ("rejected", "granted")
        )
        && left.op == Op::Eq
        && right.op == Op::Eq
        && left.value == "true"
        && right.value == "true"
}

fn value_satisfies(value: &str, op: Op, bound: &str) -> bool {
    if let (Ok(value), Ok(bound)) = (value.parse::<f64>(), bound.parse::<f64>()) {
        return match op {
            Op::Eq => value == bound,
            Op::Ne => value != bound,
            Op::Gt => value > bound,
            Op::Ge => value >= bound,
            Op::Lt => value < bound,
            Op::Le => value <= bound,
        };
    }

    match op {
        Op::Eq => value == bound,
        Op::Ne => value != bound,
        Op::Gt | Op::Lt => value != bound,
        Op::Ge | Op::Le => true,
    }
}

fn ranges_contradict(left_op: Op, left_bound: &str, right_op: Op, right_bound: &str) -> bool {
    if left_bound == right_bound {
        return matches!(
            (left_op, right_op),
            (Op::Ge, Op::Lt)
                | (Op::Lt, Op::Ge)
                | (Op::Gt, Op::Le)
                | (Op::Le, Op::Gt)
                | (Op::Gt, Op::Lt)
                | (Op::Lt, Op::Gt)
        );
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proposal_017_lint_detects_duplicate_unconditional_transitions() {
        assert!(conditions_can_match_simultaneously("true", "true"));
        assert!(conditions_can_match_simultaneously(
            "status == 'complete'",
            "status == 'complete'"
        ));
    }

    #[test]
    fn proposal_017_lint_accepts_common_complementary_transition_pairs() {
        assert!(!conditions_can_match_simultaneously(
            "proposal_review_summary.average_score >= vars.proposal_score_target and proposal_review_summary.blocker_count == 0",
            "proposal_review_summary.average_score < vars.proposal_score_target or proposal_review_summary.blocker_count > 0"
        ));
        assert!(!conditions_can_match_simultaneously(
            "implementation_review_summary.status == vars.implementation_review_target_status or implementation_review_summary.status == 'release_evidence_blocked'",
            "implementation_review_summary.status != vars.implementation_review_target_status and implementation_review_summary.status != 'release_evidence_blocked'"
        ));
    }
}
