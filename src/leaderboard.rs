//! Group run summaries into comparable cohorts and rank them.
//!
//! Two rules decide what appears on a board, and both are enforced here so the
//! live `/api/leaderboard` and the static `membench.leaderboard.v1` export can
//! never disagree:
//!
//! 1. **Only eligible records are ranked.** [`crate::eligibility`] runs the
//!    machine-checkable half of the published review gate against bytes on
//!    disk. Everything else is listed in [`LeaderboardView::unranked`] with the
//!    gates it failed — visible, but never ranked and never scored against.
//! 2. **A cohort is one comparability class, not one size class.** Two runs
//!    share a cohort only if benchmark, size, question-set fingerprint, judge
//!    model and judge prompt mode all match. Runs judged by different models,
//!    or over different question sets, are different boards — comparing their
//!    accuracies is meaningless, so the code will not put them in one table.

use crate::cohort;
use crate::eligibility::GateFailure;
use crate::registry::RunSummary;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize)]
pub struct Cohort {
    /// Stable, readable identity:
    /// `{benchmark}::{limit}::ds:{fingerprint}::judge:{model}::mode:{prompt_mode}`.
    pub cohort_id: String,
    pub benchmark: String,
    pub limit: Option<u64>,
    pub run_count: usize,
    /// The one question-set fingerprint shared by every row.
    pub dataset_fingerprint: Option<String>,
    /// The one judge model shared by every row.
    pub judge_model: Option<String>,
    /// The one judge prompt mode shared by every row.
    pub judge_prompt_mode: Option<String>,
    /// Distinct question-set fingerprints present. Length ≤ 1 by construction —
    /// kept so a consumer can assert the invariant rather than trust it.
    pub dataset_fingerprints: Vec<String>,
    /// Distinct judge models present (≤ 1 by construction).
    pub judge_models: Vec<String>,
    /// Distinct judge prompt modes present (≤ 1 by construction).
    pub judge_prompt_modes: Vec<String>,
    /// True when the cohort's comparability identity is fully known. Ranked
    /// cohorts are always strictly comparable — the `cohort-identity` gate
    /// rejects records that do not record all three fields.
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

/// A run excluded from ranking, with the reason and the gates it failed.
///
/// Accuracy fields are kept when the report carries them: a meta record can
/// hold a real measured score whose question-level artifacts were omitted on
/// purpose, and consumers may display it as long as the exclusion label travels
/// with the number.
#[derive(Clone, Debug, Serialize)]
pub struct UnrankedRow {
    pub run_id: String,
    pub run_name: String,
    /// `meta-record`, `unscored`, or `gate-failed`.
    pub reason: String,
    /// Every review gate this record failed, with detail.
    pub failed_gates: Vec<GateFailure>,
    pub system: String,
    pub benchmark: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accuracy: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accuracy_correct: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accuracy_total: Option<u64>,
    /// True for synthetic contract fixtures.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub fixture: bool,
}

/// The whole board: ranked cohorts plus everything held back from ranking.
#[derive(Clone, Debug, Serialize)]
pub struct LeaderboardView {
    pub cohorts: Vec<Cohort>,
    pub unranked: Vec<UnrankedRow>,
}

/// Full comparability identity. Anything that would make two accuracies
/// incomparable belongs in this key, which is exactly the id the registry
/// already records on every run (`RunSummary::cohort_id`).
fn cohort_key(summary: &RunSummary) -> String {
    cohort::cohort_id(
        &summary.benchmark,
        summary.limit,
        summary.dataset_fingerprint.as_deref(),
        summary.judge_model.as_deref(),
        summary.judge_prompt_mode.as_deref(),
    )
}

/// Split summaries into the ones that may be ranked and the ones that may not.
/// This is the single eligibility filter; the live server and the exporter both
/// call it, so a row can never be ranked in one surface and excluded in the
/// other.
pub fn partition(summaries: Vec<RunSummary>) -> (Vec<RunSummary>, Vec<UnrankedRow>) {
    let mut ranked = Vec::new();
    let mut unranked = Vec::new();
    for summary in summaries {
        if summary.eligibility.eligible {
            ranked.push(summary);
            continue;
        }
        let reason = summary
            .eligibility
            .reason(summary.is_meta_record, summary.accuracy.is_some());
        unranked.push(UnrankedRow {
            run_id: summary.run_id.clone(),
            run_name: summary.run_name.clone(),
            reason: reason.to_string(),
            failed_gates: summary.eligibility.failures.clone(),
            system: summary.system.clone(),
            benchmark: summary.benchmark.clone(),
            limit: summary.limit,
            accuracy: summary.accuracy,
            accuracy_correct: summary.accuracy_correct,
            accuracy_total: summary.accuracy_total,
            fixture: summary.fixture,
        });
    }
    unranked.sort_by(|left, right| left.run_id.cmp(&right.run_id));
    (ranked, unranked)
}

/// Build the whole board — the entry point for every consumer.
pub fn build_view(summaries: Vec<RunSummary>) -> LeaderboardView {
    let (ranked, unranked) = partition(summaries);
    LeaderboardView {
        cohorts: build_cohorts(ranked),
        unranked,
    }
}

/// Group already-eligible summaries into comparable cohorts and rank them.
///
/// Callers that have not filtered for eligibility should use [`build_view`];
/// this function assumes the partition has already happened.
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
                    // Stable ordering for tied accuracies, so exports diff clean.
                    .then_with(|| left.run_id.cmp(&right.run_id))
            });

            let distinct = |values: Vec<String>| {
                let mut values = values;
                values.sort();
                values.dedup();
                values
            };
            let fingerprints = distinct(
                rows.iter()
                    .filter_map(|row| row.dataset_fingerprint.clone())
                    .collect(),
            );
            let judges = distinct(
                rows.iter()
                    .filter_map(|row| row.judge_model.clone())
                    .collect(),
            );
            let prompt_modes = distinct(
                rows.iter()
                    .filter_map(|row| row.judge_prompt_mode.clone())
                    .collect(),
            );

            let first = rows.first();
            let benchmark = first.map(|row| row.benchmark.clone()).unwrap_or_default();
            let limit = first.and_then(|row| row.limit);
            let dataset_fingerprint = first.and_then(|row| row.dataset_fingerprint.clone());
            let judge_model = first.and_then(|row| row.judge_model.clone());
            let judge_prompt_mode = first.and_then(|row| row.judge_prompt_mode.clone());
            let best_accuracy = rows.iter().filter_map(|row| row.accuracy).next();
            // Every row already shares one identity (that is the group key), so
            // the only way to be non-comparable is for that identity to be
            // partly unknown.
            let strictly_comparable = dataset_fingerprint.is_some()
                && judge_model.is_some()
                && judge_prompt_mode.is_some();
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
                dataset_fingerprint,
                judge_model,
                judge_prompt_mode,
                dataset_fingerprints: fingerprints,
                judge_models: judges,
                judge_prompt_modes: prompt_modes,
                strictly_comparable,
                best_accuracy,
                rows: ranked,
            }
        })
        .collect();

    // Largest, most-populated cohorts first; cohort id breaks ties so the
    // ordering is deterministic across runs and machines.
    cohorts.sort_by(|left, right| {
        right
            .limit
            .unwrap_or(0)
            .cmp(&left.limit.unwrap_or(0))
            .then(right.run_count.cmp(&left.run_count))
            .then_with(|| left.cohort_id.cmp(&right.cohort_id))
    });
    cohorts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eligibility::Eligibility;

    fn eligible() -> Eligibility {
        Eligibility {
            eligible: true,
            level: "verified".to_string(),
            missing_artifacts: vec![],
            failures: vec![],
            review: None,
        }
    }

    fn ineligible(gate: &str) -> Eligibility {
        Eligibility {
            eligible: false,
            level: "unverified".to_string(),
            missing_artifacts: vec![],
            failures: vec![GateFailure {
                gate: gate.to_string(),
                detail: "test".to_string(),
            }],
            review: None,
        }
    }

    fn summary(run_name: &str, limit: u64, accuracy: f64, fingerprint: &str) -> RunSummary {
        RunSummary {
            run_id: format!("runs/x/long-mem-eval/{limit}/{run_name}"),
            origin: "runs".to_string(),
            system: "symbiotic-memory".to_string(),
            benchmark: "long-mem-eval".to_string(),
            limit: Some(limit),
            run_name: run_name.to_string(),
            display_name: run_name.to_string(),
            run_kind: "imported-artifact".to_string(),
            registry_section: "benchmarks".to_string(),
            is_meta_record: false,
            tuning_cohort: None,
            tuning_shape: None,
            config_label: run_name.to_string(),
            settings_label: String::new(),
            accuracy: Some(accuracy),
            accuracy_correct: None,
            accuracy_total: Some(limit),
            task_averaged_accuracy: None,
            abstention_accuracy: None,
            cost_micro_usd: None,
            latency_ms_p50: None,
            latency_ms_p95: None,
            config_signature: None,
            cohort_id: cohort::cohort_id(
                "long-mem-eval",
                Some(limit),
                Some(fingerprint),
                Some("deepseek-v4-flash"),
                Some("official"),
            ),
            dataset_fingerprint: Some(fingerprint.to_string()),
            judge_model: Some("deepseek-v4-flash".to_string()),
            judge_prompt_mode: Some("official".to_string()),
            oracle_gold: false,
            created_at: None,
            modified_ms: None,
            per_question_type: None,
            artifacts_available: vec![],
            artifacts_missing: vec![],
            native_state_available: Some(false),
            is_trial_run: false,
            trial_markers: vec![],
            fixture: false,
            eligibility: eligible(),
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
    fn different_question_sets_are_different_cohorts() {
        // Same benchmark and size, different question-set fingerprint: these
        // accuracies are not comparable, so they must not share a table.
        let cohorts = build_cohorts(vec![
            summary("a", 500, 0.90, "fp1"),
            summary("b", 500, 0.94, "fp2"),
        ]);
        assert_eq!(cohorts.len(), 2);
        for cohort in &cohorts {
            assert_eq!(cohort.run_count, 1);
            assert!(cohort.strictly_comparable);
            assert_eq!(cohort.dataset_fingerprints.len(), 1);
        }
    }

    #[test]
    fn different_judges_are_different_cohorts() {
        let mut other_judge = summary("b", 500, 0.94, "fp1");
        other_judge.judge_model = Some("gemini-3.5-flash".to_string());
        let cohorts = build_cohorts(vec![summary("a", 500, 0.90, "fp1"), other_judge]);
        assert_eq!(cohorts.len(), 2);
        assert!(cohorts.iter().all(|cohort| cohort.run_count == 1));
    }

    #[test]
    fn different_judge_prompt_modes_are_different_cohorts() {
        let mut other_mode = summary("b", 500, 0.94, "fp1");
        other_mode.judge_prompt_mode = Some("lenient".to_string());
        let cohorts = build_cohorts(vec![summary("a", 500, 0.90, "fp1"), other_mode]);
        assert_eq!(cohorts.len(), 2);
    }

    #[test]
    fn cohort_id_states_the_full_identity() {
        let cohorts = build_cohorts(vec![summary("a", 500, 0.90, "fp1")]);
        assert_eq!(
            cohorts[0].cohort_id,
            "long-mem-eval::500::ds:fp1::judge:deepseek-v4-flash::mode:official"
        );
        // A run's recorded cohort id is the id of the cohort it lands in, so
        // the UI can match rows to boards without re-deriving the identity.
        assert_eq!(cohorts[0].rows[0].summary.cohort_id, cohorts[0].cohort_id);
    }

    #[test]
    fn ineligible_rows_are_never_ranked() {
        let mut blocked = summary("blocked", 500, 0.99, "fp1");
        blocked.eligibility = ineligible("independent-review");
        let view = build_view(vec![summary("ok", 500, 0.90, "fp1"), blocked]);

        assert_eq!(view.cohorts.len(), 1);
        assert_eq!(view.cohorts[0].rows.len(), 1);
        assert_eq!(view.cohorts[0].rows[0].summary.run_name, "ok");
        // The highest accuracy on record is excluded, so it must not set the
        // cohort's best score either.
        assert_eq!(view.cohorts[0].best_accuracy, Some(0.90));

        assert_eq!(view.unranked.len(), 1);
        assert_eq!(view.unranked[0].run_name, "blocked");
        assert_eq!(view.unranked[0].reason, "gate-failed");
        assert_eq!(view.unranked[0].failed_gates[0].gate, "independent-review");
        assert_eq!(view.unranked[0].accuracy, Some(0.99));
    }

    #[test]
    fn meta_and_unscored_rows_keep_their_specific_reason() {
        let mut meta = summary("meta", 500, 0.9, "fp1");
        meta.is_meta_record = true;
        meta.eligibility = ineligible("clean-flags");
        let mut unscored = summary("unscored", 500, 0.0, "fp1");
        unscored.accuracy = None;
        unscored.eligibility = ineligible("scored");

        let view = build_view(vec![meta, unscored]);
        assert!(view.cohorts.is_empty());
        assert_eq!(view.unranked[0].reason, "meta-record");
        assert_eq!(view.unranked[1].reason, "unscored");
        assert_eq!(view.unranked[1].accuracy, None);
    }
}
