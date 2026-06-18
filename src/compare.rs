//! Verdict diff between a baseline run and a candidate run.
//!
//! Both runs are reduced to `question_id -> QuestionRow`, then every shared
//! question is bucketed by how its correctness transitioned. This powers the
//! explorer's "what did this change actually do?" view: newly fixed answers,
//! regressions, still-broken questions, and per-category accuracy deltas.

use crate::artifacts::{self, QuestionRow};
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct CompareCounts {
    /// Questions present in both runs.
    pub common: u64,
    /// Wrong in base, correct in candidate.
    pub newly_correct: u64,
    /// Correct in base, wrong in candidate (regressions).
    pub newly_wrong: u64,
    /// Correct in both.
    pub unchanged_correct: u64,
    /// Wrong in both.
    pub unchanged_wrong: u64,
    /// Abstention flag flipped between runs.
    pub abstention_changes: u64,
    /// Only in base.
    pub only_in_base: u64,
    /// Only in candidate.
    pub only_in_candidate: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct TypeDelta {
    pub question_type: String,
    pub n: u64,
    pub base_accuracy: f64,
    pub candidate_accuracy: f64,
    pub delta: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct ChangedRow {
    pub question_id: String,
    pub question_type: Option<String>,
    pub question: Option<String>,
    pub gold_answer: Option<String>,
    pub base_hypothesis: Option<String>,
    pub base_label: Option<bool>,
    pub candidate_hypothesis: Option<String>,
    pub candidate_label: Option<bool>,
    /// One of `newly_correct`, `newly_wrong`.
    pub transition: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct CompareResult {
    pub base_accuracy: Option<f64>,
    pub candidate_accuracy: Option<f64>,
    pub accuracy_delta: Option<f64>,
    pub counts: CompareCounts,
    pub per_type: Vec<TypeDelta>,
    pub changed: Vec<ChangedRow>,
}

fn index(rows: Vec<QuestionRow>) -> BTreeMap<String, QuestionRow> {
    rows.into_iter()
        .map(|row| (row.question_id.clone(), row))
        .collect()
}

fn accuracy(rows: &BTreeMap<String, QuestionRow>) -> Option<f64> {
    let labeled: Vec<bool> = rows.values().filter_map(|row| row.label).collect();
    if labeled.is_empty() {
        return None;
    }
    let correct = labeled.iter().filter(|label| **label).count();
    Some(correct as f64 / labeled.len() as f64)
}

/// Compare two runs by their on-disk artifacts.
pub fn compare_runs(base_root: &Path, candidate_root: &Path) -> CompareResult {
    let base = index(artifacts::question_rows(base_root));
    let candidate = index(artifacts::question_rows(candidate_root));

    let mut counts = CompareCounts::default();
    let mut changed = Vec::new();
    // type -> (base_correct, base_n, cand_correct, cand_n)
    let mut per_type: BTreeMap<String, [u64; 4]> = BTreeMap::new();

    for (qid, base_row) in &base {
        let Some(cand_row) = candidate.get(qid) else {
            counts.only_in_base += 1;
            continue;
        };
        counts.common += 1;

        if base_row.is_abstention != cand_row.is_abstention {
            counts.abstention_changes += 1;
        }

        let question_type = cand_row
            .question_type
            .clone()
            .or_else(|| base_row.question_type.clone())
            .unwrap_or_else(|| "unknown".to_string());
        let bucket = per_type.entry(question_type).or_default();
        if let Some(label) = base_row.label {
            bucket[1] += 1;
            if label {
                bucket[0] += 1;
            }
        }
        if let Some(label) = cand_row.label {
            bucket[3] += 1;
            if label {
                bucket[2] += 1;
            }
        }

        match (base_row.label, cand_row.label) {
            (Some(false), Some(true)) => {
                counts.newly_correct += 1;
                changed.push(changed_row(qid, base_row, cand_row, "newly_correct"));
            }
            (Some(true), Some(false)) => {
                counts.newly_wrong += 1;
                changed.push(changed_row(qid, base_row, cand_row, "newly_wrong"));
            }
            (Some(true), Some(true)) => counts.unchanged_correct += 1,
            (Some(false), Some(false)) => counts.unchanged_wrong += 1,
            _ => {}
        }
    }
    for qid in candidate.keys() {
        if !base.contains_key(qid) {
            counts.only_in_candidate += 1;
        }
    }

    let mut per_type_deltas: Vec<TypeDelta> = per_type
        .into_iter()
        .map(|(question_type, [bc, bn, cc, cn])| {
            let base_accuracy = ratio(bc, bn);
            let candidate_accuracy = ratio(cc, cn);
            TypeDelta {
                question_type,
                n: bn.max(cn),
                base_accuracy,
                candidate_accuracy,
                delta: candidate_accuracy - base_accuracy,
            }
        })
        .collect();
    per_type_deltas.sort_by(|left, right| {
        left.delta
            .partial_cmp(&right.delta)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Surface regressions first, then fixes.
    changed.sort_by(|left, right| left.transition.cmp(&right.transition));

    let base_accuracy = accuracy(&base);
    let candidate_accuracy = accuracy(&candidate);
    CompareResult {
        base_accuracy,
        candidate_accuracy,
        accuracy_delta: match (base_accuracy, candidate_accuracy) {
            (Some(base), Some(cand)) => Some(cand - base),
            _ => None,
        },
        counts,
        per_type: per_type_deltas,
        changed,
    }
}

fn ratio(correct: u64, total: u64) -> f64 {
    if total == 0 {
        0.0
    } else {
        correct as f64 / total as f64
    }
}

fn changed_row(
    qid: &str,
    base: &QuestionRow,
    candidate: &QuestionRow,
    transition: &str,
) -> ChangedRow {
    ChangedRow {
        question_id: qid.to_string(),
        question_type: candidate
            .question_type
            .clone()
            .or_else(|| base.question_type.clone()),
        question: candidate.question.clone().or_else(|| base.question.clone()),
        gold_answer: candidate
            .gold_answer
            .clone()
            .or_else(|| base.gold_answer.clone()),
        base_hypothesis: base.hypothesis.clone(),
        base_label: base.label,
        candidate_hypothesis: candidate.hypothesis.clone(),
        candidate_label: candidate.label,
        transition: transition.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_verdicts(root: &Path, lines: &str) {
        let path = root.join("artifacts").join("verdicts.jsonl");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, lines).unwrap();
    }

    #[test]
    fn buckets_transitions_and_deltas() {
        let base = tempfile::tempdir().unwrap();
        let cand = tempfile::tempdir().unwrap();
        write_verdicts(
            base.path(),
            "{\"question_id\":\"q1\",\"question_type\":\"t\",\"label\":false,\"hypothesis\":\"a\"}\n\
             {\"question_id\":\"q2\",\"question_type\":\"t\",\"label\":true,\"hypothesis\":\"b\"}\n\
             {\"question_id\":\"q3\",\"question_type\":\"t\",\"label\":false,\"hypothesis\":\"c\"}\n",
        );
        write_verdicts(
            cand.path(),
            "{\"question_id\":\"q1\",\"question_type\":\"t\",\"label\":true,\"hypothesis\":\"a2\"}\n\
             {\"question_id\":\"q2\",\"question_type\":\"t\",\"label\":false,\"hypothesis\":\"b2\"}\n\
             {\"question_id\":\"q3\",\"question_type\":\"t\",\"label\":false,\"hypothesis\":\"c2\"}\n",
        );

        let result = compare_runs(base.path(), cand.path());
        assert_eq!(result.counts.common, 3);
        assert_eq!(result.counts.newly_correct, 1);
        assert_eq!(result.counts.newly_wrong, 1);
        assert_eq!(result.counts.unchanged_wrong, 1);
        assert_eq!(result.changed.len(), 2);
        assert_eq!(result.accuracy_delta, Some(0.0));
        assert_eq!(result.per_type.len(), 1);
    }
}
