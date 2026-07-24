//! Export contract for the static leaderboard document (`membench.leaderboard.v1`).
//!
//! The dashboard serves a live leaderboard from an in-memory registry scan; this
//! module produces the *publishable* counterpart: a single JSON document built
//! from the tracked `records/` tree, stamped with provenance (records root, git
//! sha, generation time), and annotated per row with a verification level so
//! consumers can tell fully-attested scores from partially-attested ones.
//!
//! The `Cohort`/`RankedRow` structs are shared with the live server and stay
//! untouched; the export-only fields (`verification`) are added by augmenting
//! the serialized rows.

use crate::leaderboard::build_cohorts;
use crate::registry::RunSummary;
use serde::Serialize;
use serde_json::{Value, json};

/// Schema id stamped into every exported document.
pub const SCHEMA: &str = "membench.leaderboard.v1";
/// Fixed `generated_at` used in deterministic mode so canary diffs are stable.
pub const DETERMINISTIC_GENERATED_AT: &str = "1970-01-01T00:00:00Z";
/// Fixed `git_sha` used in deterministic mode so canary diffs are stable.
pub const DETERMINISTIC_GIT_SHA: &str = "deterministic";
/// Default methodology pointer carried by the document.
pub const DEFAULT_METHODOLOGY: &str = "docs/longmemeval-methodology.md";

/// Artifact kinds a score cannot be independently reproduced without.
const SCORING_ARTIFACTS: [&str; 3] = ["hypotheses", "verdicts", "scored"];

/// Provenance and mode knobs for one export.
#[derive(Clone, Debug)]
pub struct ExportOptions {
    /// Records root as the caller passed it (kept repo-relative for portability).
    pub records_root: String,
    /// Git sha of the exporting binary. Replaced in deterministic mode.
    pub git_sha: String,
    /// Repo-relative pointer to the scoring methodology doc.
    pub methodology: String,
    /// Replace volatile fields (`generated_at`, `git_sha`, per-row `modified_ms`)
    /// with fixed values so repeated exports diff cleanly.
    pub deterministic: bool,
}

/// A run excluded from ranking, with the reason it was excluded.
///
/// Accuracy fields are kept when the report carries them: meta records can
/// hold a real measured score whose question-level artifacts were omitted on
/// purpose, and consumers may display them as long as the `reason` label
/// travels with the number.
#[derive(Clone, Debug, Serialize)]
pub struct UnrankedRow {
    pub run_id: String,
    pub run_name: String,
    /// `meta-record` (dashboard-safe rollup without question-level data) or
    /// `unscored` (no accuracy metric in the report).
    pub reason: String,
    pub benchmark: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accuracy: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accuracy_correct: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accuracy_total: Option<u64>,
}

/// Build the `membench.leaderboard.v1` document from scanned run summaries.
///
/// `generated_at` is the RFC3339 timestamp the caller wants stamped; it is
/// replaced with [`DETERMINISTIC_GENERATED_AT`] when `options.deterministic`
/// is set. Meta records and unscored runs are excluded from the cohorts and
/// listed under `unranked` instead.
pub fn build_document(
    summaries: &[RunSummary],
    generated_at: &str,
    options: &ExportOptions,
) -> Value {
    let mut ranked = Vec::new();
    let mut unranked = Vec::new();
    for summary in summaries {
        let reason = if summary.is_meta_record {
            Some("meta-record")
        } else if summary.accuracy.is_none() {
            Some("unscored")
        } else {
            None
        };
        match reason {
            Some(reason) => unranked.push(UnrankedRow {
                run_id: summary.run_id.clone(),
                run_name: summary.run_name.clone(),
                reason: reason.to_string(),
                benchmark: summary.benchmark.clone(),
                limit: summary.limit,
                accuracy: summary.accuracy,
                accuracy_correct: summary.accuracy_correct,
                accuracy_total: summary.accuracy_total,
            }),
            None => ranked.push(summary.clone()),
        }
    }
    unranked.sort_by(|left, right| left.run_id.cmp(&right.run_id));

    let mut cohorts =
        serde_json::to_value(build_cohorts(ranked)).expect("cohort serialization is infallible");
    if let Some(cohort_array) = cohorts.as_array_mut() {
        for cohort in cohort_array {
            let Some(rows) = cohort.get_mut("rows").and_then(Value::as_array_mut) else {
                continue;
            };
            for row in rows {
                augment_row(row, options.deterministic);
            }
        }
    }

    json!({
        "schema": SCHEMA,
        "generated_at": if options.deterministic {
            DETERMINISTIC_GENERATED_AT
        } else {
            generated_at
        },
        "source": {
            "records_root": options.records_root,
            "git_sha": if options.deterministic {
                DETERMINISTIC_GIT_SHA
            } else {
                options.git_sha.as_str()
            },
            "run_count": summaries.len(),
        },
        "methodology": options.methodology,
        "cohorts": cohorts,
        "unranked": unranked,
    })
}

/// Add the export-only `verification` object to one serialized ranked row.
/// In deterministic mode the volatile `modified_ms` (a file mtime) is nulled
/// so byte-level canary diffs survive fresh checkouts.
fn augment_row(row: &mut Value, deterministic: bool) {
    let missing_scoring: Vec<String> = row
        .get("artifacts_missing")
        .and_then(Value::as_array)
        .map(|missing| {
            missing
                .iter()
                .filter_map(Value::as_str)
                .filter(|kind| SCORING_ARTIFACTS.contains(kind))
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default();
    let level = if missing_scoring.is_empty() {
        "full"
    } else {
        "partial"
    };

    let Some(object) = row.as_object_mut() else {
        return;
    };
    object.insert(
        "verification".to_string(),
        json!({
            "level": level,
            "missing_artifacts": missing_scoring,
        }),
    );
    if deterministic {
        object.insert("modified_ms".to_string(), Value::Null);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn summary(run_name: &str, accuracy: Option<f64>, artifacts_missing: &[&str]) -> RunSummary {
        RunSummary {
            run_id: format!("records/symbiotic-memory/long-mem-eval/5/{run_name}"),
            origin: "records".to_string(),
            system: "symbiotic-memory".to_string(),
            benchmark: "long-mem-eval".to_string(),
            limit: Some(5),
            run_name: run_name.to_string(),
            display_name: run_name.to_string(),
            run_kind: "native".to_string(),
            registry_section: "benchmarks".to_string(),
            is_meta_record: false,
            tuning_cohort: None,
            tuning_shape: None,
            config_label: run_name.to_string(),
            settings_label: String::new(),
            accuracy,
            accuracy_correct: None,
            accuracy_total: None,
            task_averaged_accuracy: None,
            abstention_accuracy: None,
            cost_micro_usd: None,
            latency_ms_p50: None,
            latency_ms_p95: None,
            config_signature: None,
            cohort_id: "long-mem-eval::5::fp::judge::official".to_string(),
            dataset_fingerprint: Some("fp".to_string()),
            judge_model: Some("judge".to_string()),
            judge_prompt_mode: Some("official".to_string()),
            oracle_gold: false,
            created_at: None,
            modified_ms: Some(123),
            per_question_type: None,
            artifacts_available: vec![],
            artifacts_missing: artifacts_missing
                .iter()
                .map(|kind| kind.to_string())
                .collect(),
            native_state_available: Some(false),
            is_trial_run: false,
            trial_markers: vec![],
        }
    }

    fn options(deterministic: bool) -> ExportOptions {
        ExportOptions {
            records_root: "records".to_string(),
            git_sha: "abc123".to_string(),
            methodology: DEFAULT_METHODOLOGY.to_string(),
            deterministic,
        }
    }

    #[test]
    fn meta_records_and_unscored_runs_are_unranked() {
        let mut meta = summary("meta", Some(0.9), &[]);
        meta.is_meta_record = true;
        let unscored = summary("unscored", None, &[]);
        let scored = summary("scored", Some(0.8), &[]);

        let doc = build_document(
            &[meta, unscored, scored],
            "2026-07-24T00:00:00Z",
            &options(false),
        );

        let cohorts = doc["cohorts"].as_array().unwrap();
        assert_eq!(cohorts.len(), 1);
        assert_eq!(cohorts[0]["rows"].as_array().unwrap().len(), 1);
        assert_eq!(cohorts[0]["rows"][0]["run_name"], "scored");

        let unranked = doc["unranked"].as_array().unwrap();
        assert_eq!(unranked.len(), 2);
        assert_eq!(unranked[0]["run_name"], "meta");
        assert_eq!(unranked[0]["reason"], "meta-record");
        // Meta records keep their measured score so consumers can display it
        // next to the `meta-record` label.
        assert_eq!(unranked[0]["accuracy"], json!(0.9));
        assert_eq!(unranked[0]["benchmark"], "long-mem-eval");
        assert_eq!(unranked[1]["run_name"], "unscored");
        assert_eq!(unranked[1]["reason"], "unscored");
        assert_eq!(unranked[1].get("accuracy"), None);
        // Every scanned run is counted in the source provenance.
        assert_eq!(doc["source"]["run_count"], 3);
    }

    #[test]
    fn verification_level_reflects_missing_scoring_artifacts() {
        let full = summary("full", Some(0.9), &["memory_traces"]);
        let partial = summary("partial", Some(0.8), &["verdicts", "scored"]);

        let doc = build_document(&[full, partial], "2026-07-24T00:00:00Z", &options(false));
        let rows = doc["cohorts"][0]["rows"].as_array().unwrap();

        assert_eq!(rows[0]["run_name"], "full");
        assert_eq!(rows[0]["verification"]["level"], "full");
        assert_eq!(
            rows[0]["verification"]["missing_artifacts"]
                .as_array()
                .unwrap()
                .len(),
            0
        );

        assert_eq!(rows[1]["run_name"], "partial");
        assert_eq!(rows[1]["verification"]["level"], "partial");
        assert_eq!(
            rows[1]["verification"]["missing_artifacts"],
            json!(["verdicts", "scored"])
        );
    }

    #[test]
    fn deterministic_exports_are_byte_identical() {
        let summaries = vec![
            summary("alpha", Some(0.8), &[]),
            summary("beta", Some(0.6), &["scored"]),
        ];
        let first = build_document(&summaries, "2026-07-24T00:00:00Z", &options(true));
        let second = build_document(&summaries, "2030-01-01T12:00:00Z", &options(true));

        assert_eq!(
            serde_json::to_string_pretty(&first).unwrap(),
            serde_json::to_string_pretty(&second).unwrap()
        );
        assert_eq!(first["generated_at"], DETERMINISTIC_GENERATED_AT);
        assert_eq!(first["source"]["git_sha"], DETERMINISTIC_GIT_SHA);
        assert_eq!(first["cohorts"][0]["rows"][0]["modified_ms"], Value::Null);
    }

    #[test]
    fn non_deterministic_export_keeps_provenance() {
        let summaries = vec![summary("alpha", Some(0.8), &[])];
        let doc = build_document(&summaries, "2026-07-24T10:00:00Z", &options(false));

        assert_eq!(doc["schema"], SCHEMA);
        assert_eq!(doc["generated_at"], "2026-07-24T10:00:00Z");
        assert_eq!(doc["source"]["git_sha"], "abc123");
        assert_eq!(doc["methodology"], DEFAULT_METHODOLOGY);
        assert_eq!(doc["cohorts"][0]["rows"][0]["modified_ms"], 123);
    }
}
