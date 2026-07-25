//! Export contract for the static leaderboard document (`membench.leaderboard.v1`).
//!
//! The dashboard serves a live leaderboard from an in-memory registry scan; this
//! module produces the *publishable* counterpart: a single JSON document built
//! from the tracked `records/` tree, stamped with provenance, and annotated per
//! row with a verification level.
//!
//! Two properties matter here.
//!
//! **Same filter as live.** Ranking eligibility is decided once, in
//! [`crate::eligibility`], and applied through [`leaderboard::build_view`] — the
//! exporter has no filter of its own, so the published board and the live board
//! rank exactly the same records.
//!
//! **Provenance you can recompute.** The exporting commit's sha is a weak
//! witness: the snapshot is committed *after* the sha it names, so it is stale
//! by construction and says nothing about what was exported. The document
//! therefore also carries `records_digest` — a content hash over every durable
//! file in the records tree — which anyone can recompute from a checkout, and which CI
//! uses to prove the committed snapshot still matches the records it claims to
//! describe (`scripts/check-leaderboard-snapshot.sh`).

use crate::leaderboard;
use crate::registry::RunSummary;
use crate::stable_hash;
use serde_json::{Value, json};
use std::path::Path;

/// Schema id stamped into every exported document.
pub const SCHEMA: &str = "membench.leaderboard.v1";
/// Fixed `generated_at` used in deterministic mode so canary diffs are stable.
pub const DETERMINISTIC_GENERATED_AT: &str = "1970-01-01T00:00:00Z";
/// Fixed `git_sha` used in deterministic mode so canary diffs are stable.
pub const DETERMINISTIC_GIT_SHA: &str = "deterministic";
/// Default methodology pointer carried by the document.
pub const DEFAULT_METHODOLOGY: &str = "docs/longmemeval-methodology.md";

/// Provenance and mode knobs for one export.
#[derive(Clone, Debug)]
pub struct ExportOptions {
    /// Records root as the caller passed it (kept repo-relative for portability).
    pub records_root: String,
    /// Content digest of that tree — see [`records_digest`]. `None` when the
    /// tree could not be read.
    pub records_digest: Option<String>,
    /// Git sha of the exporting binary. Replaced in deterministic mode.
    pub git_sha: String,
    /// Repo-relative pointer to the scoring methodology doc.
    pub methodology: String,
    /// Replace volatile fields (`generated_at`, `git_sha`, per-row `modified_ms`)
    /// with fixed values so repeated exports diff cleanly. The records digest is
    /// content-derived and stays real in this mode.
    pub deterministic: bool,
}

/// Content digest over a records tree: SHA-256 of the sorted durable
/// `{repo-relative path}\0{sha256 of file bytes}` lines.
///
/// Machine-independent and recomputable, so a consumer can check that a
/// published document describes the tree in front of them rather than trusting
/// a commit sha that was stale the moment it was committed. Ephemeral SQLite
/// sidecars are excluded: they are ignored runtime state, not release records.
pub fn records_digest(root: &Path) -> Option<String> {
    let mut files = Vec::new();
    collect_files(root, root, &mut files)?;
    files.sort();
    let mut seed = String::new();
    for (relative, digest) in files {
        seed.push_str(&relative);
        seed.push('\0');
        seed.push_str(&digest);
        seed.push('\n');
    }
    Some(stable_hash(seed.as_bytes()))
}

fn collect_files(root: &Path, dir: &Path, out: &mut Vec<(String, String)>) -> Option<()> {
    let mut entries: Vec<_> = std::fs::read_dir(dir).ok()?.flatten().collect();
    entries.sort_by_key(std::fs::DirEntry::path);
    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            collect_files(root, &path, out)?;
        } else if path.is_file() {
            let file_name = path.file_name()?.to_string_lossy();
            if file_name.ends_with(".sqlite-wal")
                || file_name.ends_with(".sqlite-shm")
                || file_name.ends_with(".sqlite-journal")
            {
                continue;
            }
            let relative = path
                .strip_prefix(root)
                .ok()?
                .to_string_lossy()
                .replace('\\', "/");
            let bytes = std::fs::read(&path).ok()?;
            out.push((relative, stable_hash(&bytes)));
        }
    }
    Some(())
}

/// Build the `membench.leaderboard.v1` document from scanned run summaries.
///
/// `generated_at` is the RFC3339 timestamp the caller wants stamped; it is
/// replaced with [`DETERMINISTIC_GENERATED_AT`] when `options.deterministic` is
/// set. Records that fail the review gate are excluded from every cohort and
/// listed under `unranked` with the gates they failed.
pub fn build_document(
    summaries: &[RunSummary],
    generated_at: &str,
    options: &ExportOptions,
) -> Value {
    let view = leaderboard::build_view(summaries.to_vec());
    let ranked_count: usize = view.cohorts.iter().map(|cohort| cohort.run_count).sum();

    let mut cohorts =
        serde_json::to_value(&view.cohorts).expect("cohort serialization is infallible");
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
            "records_digest": options.records_digest,
            "git_sha": if options.deterministic {
                DETERMINISTIC_GIT_SHA
            } else {
                options.git_sha.as_str()
            },
            "run_count": summaries.len(),
            "ranked_count": ranked_count,
            "unranked_count": view.unranked.len(),
            "contains_fixtures": summaries.iter().any(|summary| summary.fixture),
        },
        "methodology": options.methodology,
        "cohorts": cohorts,
        "unranked": view.unranked,
    })
}

/// Restate the row's gate verdict as the export-only `verification` object.
///
/// Every ranked row passed the gate, so the level is always `verified` here —
/// the object exists so a consumer reading a single row (rather than inferring
/// from its position in `cohorts`) sees the claim explicitly. In deterministic
/// mode the volatile `modified_ms` (a file mtime) is nulled so byte-level canary
/// diffs survive fresh checkouts.
fn augment_row(row: &mut Value, deterministic: bool) {
    let eligibility = row.get("eligibility").cloned().unwrap_or(Value::Null);
    let level = eligibility
        .get("level")
        .and_then(Value::as_str)
        .unwrap_or("unverified")
        .to_string();
    let missing = eligibility
        .get("missing_artifacts")
        .cloned()
        .unwrap_or_else(|| json!([]));
    let review = eligibility.get("review").cloned().unwrap_or(Value::Null);

    let Some(object) = row.as_object_mut() else {
        return;
    };
    object.insert(
        "verification".to_string(),
        json!({
            "level": level,
            "missing_artifacts": missing,
            "review": review,
        }),
    );
    if deterministic {
        object.insert("modified_ms".to_string(), Value::Null);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eligibility::{Eligibility, GateFailure, ReviewSummary};

    fn verified() -> Eligibility {
        Eligibility {
            eligible: true,
            level: "verified".to_string(),
            missing_artifacts: vec![],
            failures: vec![],
            review: Some(ReviewSummary {
                reviewer: "second-reviewer".to_string(),
                reviewed_at: "2026-07-24".to_string(),
                reviewed_commit: Some("abc1234".to_string()),
                verdict: "pass".to_string(),
            }),
        }
    }

    fn blocked(gate: &str, missing: &[&str]) -> Eligibility {
        Eligibility {
            eligible: false,
            level: "unverified".to_string(),
            missing_artifacts: missing.iter().map(|kind| kind.to_string()).collect(),
            failures: vec![GateFailure {
                gate: gate.to_string(),
                detail: "test".to_string(),
            }],
            review: None,
        }
    }

    fn summary(run_name: &str, accuracy: Option<f64>, eligibility: Eligibility) -> RunSummary {
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
            accuracy_total: Some(5),
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
            artifacts_missing: vec![],
            native_state_available: Some(false),
            is_trial_run: false,
            trial_markers: vec![],
            fixture: false,
            eligibility,
        }
    }

    fn options(deterministic: bool) -> ExportOptions {
        ExportOptions {
            records_root: "records".to_string(),
            records_digest: Some("digest".to_string()),
            git_sha: "abc123".to_string(),
            methodology: DEFAULT_METHODOLOGY.to_string(),
            deterministic,
        }
    }

    #[test]
    fn gate_failures_are_unranked_with_their_reasons() {
        let mut meta = summary("meta", Some(0.9), blocked("clean-flags", &[]));
        meta.is_meta_record = true;
        let unscored = summary("unscored", None, blocked("scored", &[]));
        let scored = summary("scored", Some(0.8), verified());

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
        assert_eq!(unranked[0]["failed_gates"][0]["gate"], "clean-flags");
        // Meta records keep their measured score so consumers can display it
        // next to the exclusion label.
        assert_eq!(unranked[0]["accuracy"], json!(0.9));
        assert_eq!(unranked[1]["run_name"], "unscored");
        assert_eq!(unranked[1]["reason"], "unscored");
        assert_eq!(unranked[1].get("accuracy"), None);

        assert_eq!(doc["source"]["run_count"], 3);
        assert_eq!(doc["source"]["ranked_count"], 1);
        assert_eq!(doc["source"]["unranked_count"], 2);
    }

    #[test]
    fn a_record_that_only_claims_artifacts_is_never_ranked() {
        // The self-attestation case: a report whose manifest lists every
        // scoring artifact, but whose files are not on disk. The gate sees the
        // disk, not the claim.
        let claimed = summary(
            "claims-everything",
            Some(0.99),
            blocked("scoring-artifacts", &["hypotheses", "verdicts", "scored"]),
        );
        let doc = build_document(&[claimed], "2026-07-24T00:00:00Z", &options(false));

        assert!(doc["cohorts"].as_array().unwrap().is_empty());
        let unranked = &doc["unranked"][0];
        assert_eq!(unranked["reason"], "gate-failed");
        assert_eq!(unranked["failed_gates"][0]["gate"], "scoring-artifacts");
    }

    #[test]
    fn verified_rows_carry_the_review_attestation() {
        let doc = build_document(
            &[summary("scored", Some(0.8), verified())],
            "2026-07-24T00:00:00Z",
            &options(false),
        );
        let row = &doc["cohorts"][0]["rows"][0];
        assert_eq!(row["verification"]["level"], "verified");
        assert_eq!(row["verification"]["review"]["reviewer"], "second-reviewer");
        assert_eq!(row["verification"]["review"]["verdict"], "pass");
    }

    #[test]
    fn deterministic_exports_are_byte_identical() {
        let summaries = vec![
            summary("alpha", Some(0.8), verified()),
            summary("beta", Some(0.6), verified()),
        ];
        let first = build_document(&summaries, "2026-07-24T00:00:00Z", &options(true));
        let second = build_document(&summaries, "2030-01-01T12:00:00Z", &options(true));

        assert_eq!(
            serde_json::to_string_pretty(&first).unwrap(),
            serde_json::to_string_pretty(&second).unwrap()
        );
        assert_eq!(first["generated_at"], DETERMINISTIC_GENERATED_AT);
        assert_eq!(first["source"]["git_sha"], DETERMINISTIC_GIT_SHA);
        // The digest is content-derived, so it stays real even here.
        assert_eq!(first["source"]["records_digest"], "digest");
        assert_eq!(first["cohorts"][0]["rows"][0]["modified_ms"], Value::Null);
    }

    #[test]
    fn non_deterministic_export_keeps_provenance() {
        let summaries = vec![summary("alpha", Some(0.8), verified())];
        let doc = build_document(&summaries, "2026-07-24T10:00:00Z", &options(false));

        assert_eq!(doc["schema"], SCHEMA);
        assert_eq!(doc["generated_at"], "2026-07-24T10:00:00Z");
        assert_eq!(doc["source"]["git_sha"], "abc123");
        assert_eq!(doc["methodology"], DEFAULT_METHODOLOGY);
        assert_eq!(doc["cohorts"][0]["rows"][0]["modified_ms"], 123);
    }

    #[test]
    fn records_digest_tracks_content_not_paths() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("a")).unwrap();
        std::fs::write(dir.path().join("a/report.json"), "{\"x\":1}").unwrap();
        let before = records_digest(dir.path()).unwrap();

        // Re-running over an untouched tree reproduces the digest.
        assert_eq!(records_digest(dir.path()).unwrap(), before);

        // Editing any byte of any record changes it.
        std::fs::write(dir.path().join("a/report.json"), "{\"x\":2}").unwrap();
        assert_ne!(records_digest(dir.path()).unwrap(), before);
    }

    #[test]
    fn records_digest_ignores_ephemeral_sqlite_sidecars() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("queue.sqlite"), "durable").unwrap();
        let before = records_digest(dir.path()).unwrap();

        std::fs::write(dir.path().join("queue.sqlite-wal"), "transient wal").unwrap();
        std::fs::write(dir.path().join("queue.sqlite-shm"), "transient shm").unwrap();
        std::fs::write(dir.path().join("queue.sqlite-journal"), "transient journal").unwrap();

        assert_eq!(records_digest(dir.path()).unwrap(), before);
    }

    #[test]
    fn fixture_records_are_flagged_in_the_source_block() {
        let mut fixture = summary("canary", Some(0.8), verified());
        fixture.fixture = true;
        let doc = build_document(&[fixture], "2026-07-24T00:00:00Z", &options(false));
        assert_eq!(doc["source"]["contains_fixtures"], json!(true));
    }
}
