//! Ranking eligibility — the machine-checkable half of the published review gate.
//!
//! `docs/longmemeval-methodology.md` states six conditions a record must meet
//! before it may be *ranked* on a published leaderboard. Before this module the
//! export trusted the record's own `artifact_manifest`: a report that declared
//! `available: [hypotheses, verdicts, scored]` was labeled fully verified even
//! when none of those files existed on disk. That is self-attestation, not
//! verification.
//!
//! Here every condition that can be checked from the record itself is checked
//! against bytes on disk:
//!
//! | gate | doc rule | check |
//! |------|----------|-------|
//! | `clean-flags` | 4 | not a meta record, not `oracle_gold`, not `TRIAL`-flagged, and no non-promotable protocol identity |
//! | `scored` | 2 | the report carries an accuracy metric |
//! | `scoring-artifacts` | 2 | `hypotheses`/`verdicts`/`scored` files exist and are non-empty |
//! | `cohort-identity` | 2 | `dataset_fingerprint`, `judge_model`, `judge_prompt_mode` recorded |
//! | `full-scale` | 1 | scored question count equals the cohort's declared size |
//! | `provenance-traces` | 3 | `model-traces.jsonl` present and non-empty |
//! | `score-summary-hashes` | 2 | the hashes the *scorer* wrote still match the artifacts |
//! | `independent-review` | 5, 6 | a `review.json` attestation, verdict `pass` |
//! | `artifact-hashes` | 5, 6 | the attested SHA-256s match the files on disk |
//!
//! Gate 5's *judgement* (did a second reviewer actually sample verdicts?) cannot
//! be derived from a directory; what the tool can enforce is that a named
//! reviewer committed an attestation whose hashes still match the artifacts
//! being ranked, so a later edit to any scoring artifact invalidates the review
//! instead of silently inheriting it. `score-summary-hashes` is the same idea
//! one step earlier and with no human in it at all: the scorer records what it
//! judged, so tampering is visible even on a record nobody has reviewed.
//!
//! Hashing only happens when an attestation or a score summary exists, so the
//! common case — an unreviewed record — costs a few `metadata()` calls, keeping
//! the registry scan cheap.

use crate::artifacts;
use crate::stable_hash;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

/// Schema id of the per-record review attestation (`review.json`).
pub const REVIEW_SCHEMA: &str = "membench.record_review.v1";
pub const LONGMEMEVAL_V2_TEXT_ID: &str = "longmemeval-v2-text";
pub const NON_PROMOTABLE_BENCHMARKS: &[&str] = &[LONGMEMEVAL_V2_TEXT_ID];

/// Artifacts a score cannot be independently reproduced without.
pub const SCORING_ARTIFACTS: [&str; 3] = ["hypotheses", "verdicts", "scored"];

/// Trace artifacts that prove provider usage (gate 3). Any one is enough.
const PROVENANCE_ARTIFACTS: [&str; 2] = ["model_traces", "memory_traces"];

/// One failed gate, with enough detail to fix the record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct GateFailure {
    /// Stable gate id, e.g. `scoring-artifacts`.
    pub gate: String,
    /// Human-readable reason this record failed the gate.
    pub detail: String,
}

impl GateFailure {
    fn new(gate: &str, detail: impl Into<String>) -> Self {
        Self {
            gate: gate.to_string(),
            detail: detail.into(),
        }
    }
}

/// The committed review attestation, as summarized into the export.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ReviewSummary {
    pub reviewer: String,
    pub reviewed_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reviewed_commit: Option<String>,
    pub verdict: String,
}

/// The `review.json` document a promoted record must carry.
#[derive(Clone, Debug, Deserialize)]
struct ReviewAttestation {
    schema: String,
    /// Who reviewed it — a person or an independent agent, named.
    reviewer: String,
    reviewed_at: String,
    #[serde(default)]
    reviewed_commit: Option<String>,
    /// `pass` is the only verdict that admits a record to ranking.
    verdict: String,
    /// `artifact kind -> sha256 of the file as reviewed`.
    #[serde(default)]
    artifact_sha256: std::collections::BTreeMap<String, String>,
}

/// Verdict for one record: may it be ranked, and if not, why not.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Eligibility {
    /// True only when every gate below passed.
    pub eligible: bool,
    /// `verified` when eligible, otherwise `unverified`. This is the label the
    /// dashboard shows next to a score.
    pub level: String,
    /// Scoring artifacts (gate 2) that are absent or empty on disk.
    pub missing_artifacts: Vec<String>,
    /// Every gate that failed, in gate order.
    pub failures: Vec<GateFailure>,
    /// The attestation, when one was present and parseable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub review: Option<ReviewSummary>,
}

impl Eligibility {
    /// Coarse exclusion label for the export's `unranked` list.
    pub fn reason(&self, is_meta_record: bool, has_accuracy: bool) -> &'static str {
        if is_meta_record {
            "meta-record"
        } else if !has_accuracy {
            "unscored"
        } else {
            "gate-failed"
        }
    }
}

/// Everything the gates need about one record. Borrowed so evaluation can run
/// inside `summarize` without cloning the summary being built.
#[derive(Clone, Copy, Debug)]
pub struct RecordFacts<'a> {
    pub run_root: &'a Path,
    pub declared_benchmark: &'a str,
    pub params_benchmark: Option<&'a str>,
    pub path_benchmark: Option<&'a str>,
    pub promotion_prohibited: bool,
    pub is_meta_record: bool,
    pub oracle_gold: bool,
    pub is_trial_run: bool,
    pub leaderboard_eligible: bool,
    pub accuracy: Option<f64>,
    pub accuracy_total: Option<u64>,
    /// Declared cohort size (the benchmark's question count for this record).
    pub limit: Option<u64>,
    pub dataset_fingerprint: Option<&'a str>,
    pub judge_model: Option<&'a str>,
    pub judge_prompt_mode: Option<&'a str>,
}

/// Run every gate against one record.
pub fn evaluate(facts: &RecordFacts) -> Eligibility {
    let mut failures = Vec::new();

    // Gate 4 — clean flags.
    if facts.is_meta_record {
        failures.push(GateFailure::new(
            "clean-flags",
            "meta record: question-level scoring artifacts were deliberately omitted",
        ));
    }
    if facts.oracle_gold {
        failures.push(GateFailure::new(
            "clean-flags",
            "oracle_gold run: gold evidence was fed to the answerer",
        ));
    }
    if facts.is_trial_run {
        failures.push(GateFailure::new(
            "clean-flags",
            "TRIAL-flagged diagnostic run, not a benchmark claim",
        ));
    }
    let benchmark_witnesses = [
        Some(facts.declared_benchmark),
        facts.params_benchmark,
        facts.path_benchmark,
    ];
    let categorically_prohibited = facts.promotion_prohibited
        || benchmark_witnesses
            .into_iter()
            .flatten()
            .any(|benchmark| NON_PROMOTABLE_BENCHMARKS.contains(&benchmark));
    if !facts.leaderboard_eligible || categorically_prohibited {
        failures.push(GateFailure::new(
            "clean-flags",
            if categorically_prohibited {
                "run protocol or benchmark identity categorically prohibits leaderboard promotion"
            } else {
                "run protocol explicitly prohibits leaderboard promotion"
            },
        ));
    } else if [facts.params_benchmark, facts.path_benchmark]
        .into_iter()
        .flatten()
        .any(|benchmark| benchmark != facts.declared_benchmark)
    {
        failures.push(GateFailure::new(
            "clean-flags",
            "declared, parameter, and registry-path benchmark identities do not match",
        ));
    }

    // Gate 2 — a score at all.
    if facts.accuracy.is_none() {
        failures.push(GateFailure::new(
            "scored",
            "no accuracy metric in benchmark-report.json",
        ));
    }

    // Gate 2 — scoring artifacts, verified against disk rather than the manifest.
    let mut missing_artifacts = Vec::new();
    for kind in SCORING_ARTIFACTS {
        if !artifact_is_present(facts.run_root, kind) {
            missing_artifacts.push(kind.to_string());
        }
    }
    if !missing_artifacts.is_empty() {
        failures.push(GateFailure::new(
            "scoring-artifacts",
            format!("absent or empty on disk: {}", missing_artifacts.join(", ")),
        ));
    }

    // Gate 2 — recorded cohort identity.
    let mut identity_gaps = Vec::new();
    if facts.dataset_fingerprint.is_none() {
        identity_gaps.push("dataset_fingerprint");
    }
    if facts.judge_model.is_none() {
        identity_gaps.push("judge_model");
    }
    if facts.judge_prompt_mode.is_none() {
        identity_gaps.push("judge_prompt_mode");
    }
    if !identity_gaps.is_empty() {
        failures.push(GateFailure::new(
            "cohort-identity",
            format!("not recorded: {}", identity_gaps.join(", ")),
        ));
    }

    // Gate 1 — full-scale run: the scored question count must equal the size
    // class the record is being ranked in, so a 50-question subset can never
    // land in the 500-question cohort.
    match (facts.accuracy_total, facts.limit) {
        (Some(total), Some(limit)) if total != limit => {
            failures.push(GateFailure::new(
                "full-scale",
                format!("scored {total} questions but the cohort size is {limit}"),
            ));
        }
        (Some(0), _) => {
            failures.push(GateFailure::new("full-scale", "zero questions scored"));
        }
        (None, _) => {
            failures.push(GateFailure::new(
                "full-scale",
                "no scored question count in the report",
            ));
        }
        _ => {}
    }

    // Gate 3 — provenance: provider usage proven by traces, not intent.
    if !PROVENANCE_ARTIFACTS
        .iter()
        .any(|kind| artifact_is_present(facts.run_root, kind))
    {
        failures.push(GateFailure::new(
            "provenance-traces",
            "no model or memory traces on disk; configured_models alone proves nothing",
        ));
    }

    // Gate 2, continued — the scorer's own hash chain. `score-summary.json` is
    // written at scoring time and records what each artifact hashed to *then*.
    // Checking it needs no reviewer at all, so an artifact edited after scoring
    // is caught even on a record nobody has reviewed yet.
    for mismatch in score_summary_mismatches(facts.run_root) {
        failures.push(GateFailure::new("score-summary-hashes", mismatch));
    }

    // Gates 5 and 6 — independent review, still bound to these exact bytes.
    let review = read_review(facts.run_root);
    match &review {
        None => failures.push(GateFailure::new(
            "independent-review",
            format!("no {REVIEW_SCHEMA} attestation (review.json) in the record"),
        )),
        Some(review) if review.schema != REVIEW_SCHEMA => failures.push(GateFailure::new(
            "independent-review",
            format!(
                "review.json declares schema {} (expected {REVIEW_SCHEMA})",
                review.schema
            ),
        )),
        Some(review) if review.verdict != "pass" => failures.push(GateFailure::new(
            "independent-review",
            format!("review verdict is {} (expected pass)", review.verdict),
        )),
        Some(review)
            if review.reviewer.trim().is_empty() || review.reviewed_at.trim().is_empty() =>
        {
            failures.push(GateFailure::new(
                "independent-review",
                "review.json must name a reviewer and a review date",
            ))
        }
        Some(review) => {
            for mismatch in hash_mismatches(facts.run_root, review) {
                failures.push(GateFailure::new("artifact-hashes", mismatch));
            }
        }
    }

    let eligible = failures.is_empty();
    Eligibility {
        eligible,
        level: if eligible { "verified" } else { "unverified" }.to_string(),
        missing_artifacts,
        failures,
        review: review.map(|review| ReviewSummary {
            reviewer: review.reviewer,
            reviewed_at: review.reviewed_at,
            reviewed_commit: review.reviewed_commit,
            verdict: review.verdict,
        }),
    }
}

/// An artifact counts as present only when its file exists and has bytes: a
/// zero-length `verdicts.jsonl` proves nothing.
fn artifact_is_present(run_root: &Path, kind: &str) -> bool {
    let Some((file_name, _)) = artifacts::artifact_file(kind) else {
        return false;
    };
    std::fs::metadata(run_root.join("artifacts").join(file_name))
        .map(|meta| meta.is_file() && meta.len() > 0)
        .unwrap_or(false)
}

fn read_review(run_root: &Path) -> Option<ReviewAttestation> {
    let raw = std::fs::read_to_string(run_root.join("review.json")).ok()?;
    serde_json::from_str::<ReviewAttestation>(&raw).ok()
}

/// Recompute the SHA-256 of every attested artifact and report every gap: an
/// unattested scoring artifact, a file that no longer hashes to what was
/// reviewed, or an attested file that has since disappeared.
fn hash_mismatches(run_root: &Path, review: &ReviewAttestation) -> Vec<String> {
    let mut problems = Vec::new();
    for kind in SCORING_ARTIFACTS {
        let Some(expected) = review.artifact_sha256.get(kind) else {
            problems.push(format!("review.json does not attest a sha256 for {kind}"));
            continue;
        };
        let Some((file_name, _)) = artifacts::artifact_file(kind) else {
            continue;
        };
        match std::fs::read(run_root.join("artifacts").join(file_name)) {
            Ok(bytes) => {
                let actual = stable_hash(&bytes);
                if &actual != expected {
                    problems.push(format!(
                        "{kind} hashes to {actual} but the review attested {expected}"
                    ));
                }
            }
            Err(_) => problems.push(format!("{kind} attested by the review is not readable")),
        }
    }
    problems
}

/// Verify the hashes the scorer recorded in `artifacts/score-summary.json`
/// against the files on disk.
///
/// The scorer writes `hypotheses_hash` plus an `artifact_hashes` map of
/// run-relative path → SHA-256 when it produces the verdicts. That is evidence
/// from a different moment and a different actor than the review attestation,
/// so a record whose artifacts were touched after scoring fails here regardless
/// of what any later review says. Absent or hash-less summaries produce no
/// failures — older records simply do not carry the chain.
fn score_summary_mismatches(run_root: &Path) -> Vec<String> {
    let Ok(raw) = std::fs::read_to_string(run_root.join("artifacts/score-summary.json")) else {
        return Vec::new();
    };
    let Ok(summary) = serde_json::from_str::<Value>(&raw) else {
        return vec!["artifacts/score-summary.json is not readable JSON".to_string()];
    };

    let mut expected: Vec<(String, String)> = Vec::new();
    if let Some(hash) = summary.get("hypotheses_hash").and_then(Value::as_str) {
        expected.push(("artifacts/hypotheses.jsonl".to_string(), hash.to_string()));
    }
    if let Some(map) = summary.get("artifact_hashes").and_then(Value::as_object) {
        for (path, hash) in map {
            if let Some(hash) = hash.as_str() {
                expected.push((path.clone(), hash.to_string()));
            }
        }
    }

    let mut problems = Vec::new();
    for (relative, hash) in expected {
        // The summary names paths inside its own record; anything climbing out
        // of it is malformed provenance, not something to chase.
        if relative.contains("..") || Path::new(&relative).is_absolute() {
            problems.push(format!(
                "score-summary.json names a path outside the record: {relative}"
            ));
            continue;
        }
        match std::fs::read(run_root.join(&relative)) {
            Ok(bytes) => {
                let actual = stable_hash(&bytes);
                if actual != hash {
                    problems.push(format!(
                        "{relative} hashes to {actual} but the scorer recorded {hash}"
                    ));
                }
            }
            Err(_) => problems.push(format!(
                "{relative} is recorded in score-summary.json but is not readable"
            )),
        }
    }
    problems
}

/// Serialize an attestation for the given record. Used by tests and by the
/// documented promotion flow to produce a `review.json` skeleton.
pub fn artifact_hashes(run_root: &Path) -> std::collections::BTreeMap<String, String> {
    let mut hashes = std::collections::BTreeMap::new();
    for kind in SCORING_ARTIFACTS {
        let Some((file_name, _)) = artifacts::artifact_file(kind) else {
            continue;
        };
        if let Ok(bytes) = std::fs::read(run_root.join("artifacts").join(file_name)) {
            hashes.insert(kind.to_string(), stable_hash(&bytes));
        }
    }
    hashes
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    struct Fixture {
        dir: tempfile::TempDir,
    }

    impl Fixture {
        /// A record that passes every gate.
        fn complete() -> Self {
            let dir = tempfile::tempdir().unwrap();
            let artifacts = dir.path().join("artifacts");
            std::fs::create_dir_all(&artifacts).unwrap();
            std::fs::write(
                artifacts.join("hypotheses.jsonl"),
                "{\"question_id\":\"q1\"}\n",
            )
            .unwrap();
            std::fs::write(
                artifacts.join("verdicts.jsonl"),
                "{\"question_id\":\"q1\"}\n",
            )
            .unwrap();
            std::fs::write(artifacts.join("scored.json"), "{\"judge_model\":\"j\"}\n").unwrap();
            std::fs::write(artifacts.join("model-traces.jsonl"), "{\"model\":\"m\"}\n").unwrap();
            let fixture = Self { dir };
            fixture.write_review("pass");
            fixture
        }

        fn root(&self) -> &Path {
            self.dir.path()
        }

        fn artifact(&self, name: &str) -> PathBuf {
            self.dir.path().join("artifacts").join(name)
        }

        /// Write the scorer's hash chain exactly as `score-summary.json` records
        /// it: `hypotheses_hash` plus run-relative `artifact_hashes`.
        fn write_score_summary(&self) {
            let hash = |relative: &str| {
                stable_hash(&std::fs::read(self.dir.path().join(relative)).unwrap())
            };
            let doc = serde_json::json!({
                "schema_version": 1,
                "scorer": "canary-judge",
                "hypotheses_hash": hash("artifacts/hypotheses.jsonl"),
                "artifact_hashes": {
                    "artifacts/scored.json": hash("artifacts/scored.json"),
                    "artifacts/verdicts.jsonl": hash("artifacts/verdicts.jsonl"),
                },
            });
            std::fs::write(
                self.artifact("score-summary.json"),
                serde_json::to_string_pretty(&doc).unwrap(),
            )
            .unwrap();
        }

        fn write_review(&self, verdict: &str) {
            let hashes = artifact_hashes(self.root());
            let doc = serde_json::json!({
                "schema": REVIEW_SCHEMA,
                "reviewer": "second-reviewer",
                "reviewed_at": "2026-07-24",
                "reviewed_commit": "abc1234",
                "verdict": verdict,
                "artifact_sha256": hashes,
            });
            std::fs::write(
                self.dir.path().join("review.json"),
                serde_json::to_string_pretty(&doc).unwrap(),
            )
            .unwrap();
        }
    }

    fn facts(run_root: &Path) -> RecordFacts<'_> {
        RecordFacts {
            run_root,
            declared_benchmark: "long-mem-eval",
            params_benchmark: Some("long-mem-eval"),
            path_benchmark: Some("long-mem-eval"),
            promotion_prohibited: false,
            is_meta_record: false,
            oracle_gold: false,
            is_trial_run: false,
            leaderboard_eligible: true,
            accuracy: Some(0.9),
            accuracy_total: Some(500),
            limit: Some(500),
            dataset_fingerprint: Some("fp"),
            judge_model: Some("judge"),
            judge_prompt_mode: Some("official"),
        }
    }

    #[test]
    fn benchmark_identity_mismatch_fails_closed_without_rejecting_a_missing_path_witness() {
        let root = tempfile::tempdir().unwrap();
        let matching = facts(root.path());

        let mut mismatch = matching;
        mismatch.path_benchmark = Some("other-benchmark");
        assert!(
            evaluate(&mismatch)
                .failures
                .iter()
                .any(|failure| failure.detail.contains("identities do not match"))
        );

        let mut missing = matching;
        missing.path_benchmark = None;
        assert!(
            !evaluate(&missing)
                .failures
                .iter()
                .any(|failure| failure.detail.contains("identities do not match"))
        );
    }

    fn gates(eligibility: &Eligibility) -> Vec<&str> {
        eligibility
            .failures
            .iter()
            .map(|failure| failure.gate.as_str())
            .collect()
    }

    #[test]
    fn complete_reviewed_record_is_eligible() {
        let fixture = Fixture::complete();
        let verdict = evaluate(&facts(fixture.root()));
        assert!(
            verdict.eligible,
            "unexpected failures: {:?}",
            verdict.failures
        );
        assert_eq!(verdict.level, "verified");
        assert_eq!(verdict.review.unwrap().reviewer, "second-reviewer");
    }

    #[test]
    fn manifest_claims_do_not_substitute_for_files_on_disk() {
        // The canary-alpha shape: a report declaring the scoring artifacts
        // available while the record directory holds none of them.
        let dir = tempfile::tempdir().unwrap();
        let verdict = evaluate(&facts(dir.path()));
        assert!(!verdict.eligible);
        assert_eq!(
            verdict.missing_artifacts,
            vec!["hypotheses", "verdicts", "scored"]
        );
        assert!(gates(&verdict).contains(&"scoring-artifacts"));
    }

    #[test]
    fn empty_artifact_files_do_not_count_as_present() {
        let fixture = Fixture::complete();
        std::fs::write(fixture.artifact("verdicts.jsonl"), "").unwrap();
        let verdict = evaluate(&facts(fixture.root()));
        assert!(!verdict.eligible);
        assert_eq!(verdict.missing_artifacts, vec!["verdicts"]);
    }

    #[test]
    fn unreviewed_record_is_not_eligible() {
        let fixture = Fixture::complete();
        std::fs::remove_file(fixture.root().join("review.json")).unwrap();
        let verdict = evaluate(&facts(fixture.root()));
        assert!(!verdict.eligible);
        assert_eq!(gates(&verdict), vec!["independent-review"]);
        assert!(verdict.review.is_none());
    }

    #[test]
    fn failed_review_verdict_is_not_eligible() {
        let fixture = Fixture::complete();
        fixture.write_review("fail");
        let verdict = evaluate(&facts(fixture.root()));
        assert!(!verdict.eligible);
        assert_eq!(gates(&verdict), vec!["independent-review"]);
    }

    #[test]
    fn editing_an_artifact_after_review_invalidates_it() {
        let fixture = Fixture::complete();
        std::fs::write(
            fixture.artifact("verdicts.jsonl"),
            "{\"question_id\":\"q1\",\"correct\":true}\n",
        )
        .unwrap();
        let verdict = evaluate(&facts(fixture.root()));
        assert!(!verdict.eligible);
        assert_eq!(gates(&verdict), vec!["artifact-hashes"]);
        assert!(verdict.failures[0].detail.contains("verdicts hashes to"));
    }

    #[test]
    fn subset_run_cannot_be_ranked_in_a_full_size_cohort() {
        let fixture = Fixture::complete();
        let mut facts = facts(fixture.root());
        facts.accuracy_total = Some(50);
        let verdict = evaluate(&facts);
        assert!(!verdict.eligible);
        assert_eq!(gates(&verdict), vec!["full-scale"]);
    }

    #[test]
    fn oracle_gold_and_trial_runs_are_excluded() {
        let fixture = Fixture::complete();
        let mut facts = facts(fixture.root());
        facts.oracle_gold = true;
        facts.is_trial_run = true;
        let verdict = evaluate(&facts);
        assert!(!verdict.eligible);
        assert_eq!(gates(&verdict), vec!["clean-flags", "clean-flags"]);
    }

    #[test]
    fn protocol_can_prohibit_leaderboard_promotion() {
        let fixture = Fixture::complete();
        let mut facts = facts(fixture.dir.path());
        facts.leaderboard_eligible = false;
        let verdict = evaluate(&facts);
        assert!(!verdict.eligible);
        assert!(verdict.failures.iter().any(|failure| {
            failure.gate == "clean-flags"
                && failure.detail.contains("prohibits leaderboard promotion")
        }));
    }

    #[test]
    fn benchmark_protocol_prohibition_is_an_independent_clean_flag() {
        let fixture = Fixture::complete();
        let mut facts = facts(fixture.dir.path());
        facts.promotion_prohibited = true;
        let verdict = evaluate(&facts);
        assert!(!verdict.eligible);
        assert!(verdict.failures.iter().any(|failure| {
            failure.gate == "clean-flags" && failure.detail.contains("categorically prohibits")
        }));
    }

    #[test]
    fn missing_cohort_identity_blocks_ranking() {
        let fixture = Fixture::complete();
        let mut facts = facts(fixture.root());
        facts.judge_model = None;
        facts.judge_prompt_mode = None;
        let verdict = evaluate(&facts);
        assert!(!verdict.eligible);
        assert_eq!(gates(&verdict), vec!["cohort-identity"]);
        assert!(verdict.failures[0].detail.contains("judge_model"));
    }

    #[test]
    fn scorer_recorded_hashes_are_checked_without_a_reviewer() {
        let fixture = Fixture::complete();
        fixture.write_score_summary();
        assert!(evaluate(&facts(fixture.root())).eligible);

        // Edit an artifact and re-attest it: the review now agrees with disk,
        // but the scorer's own record of what it judged does not.
        std::fs::write(
            fixture.artifact("verdicts.jsonl"),
            "{\"question_id\":\"q1\",\"correct\":true}\n",
        )
        .unwrap();
        fixture.write_review("pass");

        let verdict = evaluate(&facts(fixture.root()));
        assert!(!verdict.eligible);
        assert_eq!(gates(&verdict), vec!["score-summary-hashes"]);
        assert!(verdict.failures[0].detail.contains("the scorer recorded"));
    }

    #[test]
    fn a_record_without_a_score_summary_is_not_penalised() {
        // Older records predate the scorer hash chain; absence is not evidence
        // of tampering, so the gate stays silent.
        let fixture = Fixture::complete();
        assert!(!fixture.artifact("score-summary.json").exists());
        assert!(evaluate(&facts(fixture.root())).eligible);
    }

    #[test]
    fn partially_published_score_bundle_is_not_eligible() {
        // The torn-publish shape: verdicts renamed into place, scored.json never landed.
        let fixture = Fixture::complete();
        std::fs::remove_file(fixture.artifact("scored.json")).unwrap();
        let verdict = evaluate(&facts(fixture.root()));
        assert!(!verdict.eligible);
        assert_eq!(verdict.missing_artifacts, vec!["scored"]);
        assert!(gates(&verdict).contains(&"scoring-artifacts"));
    }

    #[test]
    fn stale_summary_from_a_torn_republish_is_not_eligible() {
        // A republish that died between renames: verdicts and scored are new, but the
        // hash-binding summary still describes the previous bundle.
        let fixture = Fixture::complete();
        fixture.write_score_summary();
        std::fs::write(
            fixture.artifact("verdicts.jsonl"),
            "{\"question_id\":\"q1\",\"correct\":true}\n",
        )
        .unwrap();
        std::fs::write(
            fixture.artifact("scored.json"),
            "{\"judge_model\":\"new\"}\n",
        )
        .unwrap();
        fixture.write_review("pass");

        let verdict = evaluate(&facts(fixture.root()));
        assert!(!verdict.eligible);
        assert!(gates(&verdict).contains(&"score-summary-hashes"));
    }

    #[test]
    fn missing_provider_traces_block_ranking() {
        let fixture = Fixture::complete();
        std::fs::remove_file(fixture.artifact("model-traces.jsonl")).unwrap();
        let verdict = evaluate(&facts(fixture.root()));
        assert!(!verdict.eligible);
        assert_eq!(gates(&verdict), vec!["provenance-traces"]);
    }
}
