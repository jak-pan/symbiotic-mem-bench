//! Group run summaries into comparable cohorts and rank them.
//!
//! A cohort is the Geekbench-style "size class": one benchmark at one size.
//! Runs inside a cohort are ranked by accuracy. Because runs *should* share the
//! same questions and judge but might not, each cohort also reports the distinct
//! `dataset_fingerprint`s, `judge_model`s, and `judge_prompt_mode`s it contains,
//! so the UI can flag a cohort whose rows are not strictly comparable.

use crate::registry::RunSummary;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize)]
pub struct Cohort {
    /// Stable id: `{benchmark}::{limit}`.
    pub cohort_id: String,
    pub benchmark: String,
    pub limit: Option<u64>,
    pub run_count: usize,
    /// Distinct question-set fingerprints present (1 == strictly same questions).
    pub dataset_fingerprints: Vec<String>,
    /// Distinct judge models present (1 == strictly same judge).
    pub judge_models: Vec<String>,
    /// Distinct judge prompt modes present (1 == strictly same rubric).
    pub judge_prompt_modes: Vec<String>,
    /// True when every run shares one fingerprint, judge, and judge prompt mode.
    pub strictly_comparable: bool,
    pub best_accuracy: Option<f64>,
    /// Runs ranked by accuracy, descending.
    pub rows: Vec<RankedRow>,
}

#[derive(Clone, Debug, Serialize)]
pub struct RankedRow {
    pub rank: usize,
    #[serde(flatten)]
    pub summary: RunSummary,
}

fn cohort_key(summary: &RunSummary) -> String {
    format!(
        "{}::{}",
        summary.benchmark,
        summary
            .limit
            .map(|value| value.to_string())
            .unwrap_or_else(|| "?".to_string())
    )
}

/// Build cohorts from a flat list of summaries.
pub fn build_cohorts(summaries: Vec<RunSummary>) -> Vec<Cohort> {
    let mut groups: BTreeMap<String, Vec<RunSummary>> = BTreeMap::new();
    for summary in summaries {
        groups
            .entry(cohort_key(&summary))
            .or_default()
            .push(summary);
    }

    let mut cohorts: Vec<Cohort> = groups
        .into_iter()
        .map(|(cohort_id, mut rows)| {
            rows.sort_by(|left, right| {
                right
                    .accuracy
                    .unwrap_or(f64::NEG_INFINITY)
                    .partial_cmp(&left.accuracy.unwrap_or(f64::NEG_INFINITY))
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            let mut fingerprints: Vec<String> = rows
                .iter()
                .filter_map(|row| row.dataset_fingerprint.clone())
                .collect();
            fingerprints.sort();
            fingerprints.dedup();
            let mut judges: Vec<String> = rows
                .iter()
                .filter_map(|row| row.judge_model.clone())
                .collect();
            judges.sort();
            judges.dedup();
            let mut prompt_modes: Vec<String> = rows
                .iter()
                .filter_map(|row| row.judge_prompt_mode.clone())
                .collect();
            prompt_modes.sort();
            prompt_modes.dedup();

            let benchmark = rows
                .first()
                .map(|row| row.benchmark.clone())
                .unwrap_or_default();
            let limit = rows.first().and_then(|row| row.limit);
            let best_accuracy = rows.iter().filter_map(|row| row.accuracy).next();
            let strictly_comparable =
                fingerprints.len() <= 1 && judges.len() <= 1 && prompt_modes.len() <= 1;
            let run_count = rows.len();

            let ranked = rows
                .into_iter()
                .enumerate()
                .map(|(idx, summary)| RankedRow {
                    rank: idx + 1,
                    summary,
                })
                .collect();

            Cohort {
                cohort_id,
                benchmark,
                limit,
                run_count,
                dataset_fingerprints: fingerprints,
                judge_models: judges,
                judge_prompt_modes: prompt_modes,
                strictly_comparable,
                best_accuracy,
                rows: ranked,
            }
        })
        .collect();

    // Largest, most-populated cohorts first.
    cohorts.sort_by(|left, right| {
        right
            .limit
            .unwrap_or(0)
            .cmp(&left.limit.unwrap_or(0))
            .then(right.run_count.cmp(&left.run_count))
    });
    cohorts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn summary(run_name: &str, limit: u64, accuracy: f64, fingerprint: &str) -> RunSummary {
        RunSummary {
            run_id: format!("runs/x/long-mem-eval/{limit}/{run_name}"),
            origin: "runs".to_string(),
            system: "symbiotic-memory".to_string(),
            benchmark: "long-mem-eval".to_string(),
            limit: Some(limit),
            run_name: run_name.to_string(),
            run_kind: "imported-artifact".to_string(),
            config_label: run_name.to_string(),
            accuracy: Some(accuracy),
            accuracy_correct: None,
            accuracy_total: None,
            task_averaged_accuracy: None,
            abstention_accuracy: None,
            cost_micro_usd: None,
            latency_ms_p50: None,
            latency_ms_p95: None,
            config_signature: None,
            cohort_id: "x".to_string(),
            dataset_fingerprint: Some(fingerprint.to_string()),
            judge_model: Some("deepseek-v4-flash".to_string()),
            judge_prompt_mode: Some("semantic-shared-compact".to_string()),
            created_at: None,
            modified_ms: None,
            per_question_type: None,
            artifacts_available: vec![],
            artifacts_missing: vec![],
            native_state_available: Some(false),
        }
    }

    #[test]
    fn ranks_within_cohort_and_flags_comparability() {
        let cohorts = build_cohorts(vec![
            summary("a", 500, 0.90, "fp1"),
            summary("b", 500, 0.94, "fp1"),
            summary("c", 50, 0.80, "fp2"),
        ]);
        assert_eq!(cohorts.len(), 2);
        let big = cohorts.iter().find(|c| c.limit == Some(500)).unwrap();
        assert_eq!(big.rows[0].rank, 1);
        assert_eq!(big.rows[0].summary.run_name, "b");
        assert!(big.strictly_comparable);
    }

    #[test]
    fn mixed_fingerprints_break_strict_comparability() {
        let cohorts = build_cohorts(vec![
            summary("a", 500, 0.90, "fp1"),
            summary("b", 500, 0.94, "fp2"),
        ]);
        assert!(!cohorts[0].strictly_comparable);
        assert_eq!(cohorts[0].dataset_fingerprints.len(), 2);
    }
}
