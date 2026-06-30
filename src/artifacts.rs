//! Typed readers for the per-run benchmark artifacts and a merged per-question
//! view used by the explorer's question browser and the compare engine.
//!
//! Artifacts live under `<run_root>/artifacts/`. Not every run produces every
//! artifact, so all readers degrade to empty/`None` rather than erroring, and
//! malformed JSONL lines are skipped instead of aborting the whole file.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;

/// One scored hypothesis verdict (`verdicts.jsonl` / `partial-verdicts.jsonl`).
#[derive(Clone, Debug, Deserialize)]
pub struct Verdict {
    pub question_id: String,
    #[serde(default)]
    pub question_type: Option<String>,
    #[serde(default)]
    pub is_abstention: Option<bool>,
    #[serde(default)]
    pub question: Option<String>,
    /// Gold answer from the oracle.
    #[serde(default)]
    pub answer: Option<String>,
    /// Model-produced answer under test.
    #[serde(default)]
    pub hypothesis: Option<String>,
    #[serde(default)]
    pub judge_raw: Option<String>,
    /// Exact judge SYSTEM prompt sent for this question (per-type grader). Absent on older runs.
    #[serde(default)]
    pub judge_system_prompt: Option<String>,
    /// Exact judge USER message sent (question + gold/rubric + model response). Absent on older runs.
    #[serde(default)]
    pub judge_user_prompt: Option<String>,
    #[serde(default)]
    pub autoeval_label: Option<AutoEvalLabel>,
    #[serde(default)]
    pub label: Option<bool>,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct AutoEvalLabel {
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub label: Option<bool>,
}

/// One answer hypothesis (`hypotheses.jsonl`).
#[derive(Clone, Debug, Deserialize)]
pub struct Hypothesis {
    pub question_id: String,
    #[serde(default)]
    pub question_type: Option<String>,
    #[serde(default)]
    pub question: Option<String>,
    #[serde(default)]
    pub hypothesis: Option<String>,
    #[serde(default)]
    pub router_pick: Option<String>,
    #[serde(default)]
    pub debug_artifact: Option<String>,
}

/// One routing provenance record (`provenance.jsonl`).
#[derive(Clone, Debug, Deserialize)]
pub struct Provenance {
    pub question_id: String,
    #[serde(default)]
    pub initial_pick: Option<String>,
    #[serde(default)]
    pub final_pick: Option<String>,
    #[serde(default)]
    pub debug_artifact: Option<String>,
}

/// A flattened per-question row joining verdicts, hypotheses, and provenance.
#[derive(Clone, Debug, Default, Serialize)]
pub struct QuestionRow {
    pub question_id: String,
    pub question_type: Option<String>,
    pub question: Option<String>,
    pub gold_answer: Option<String>,
    pub hypothesis: Option<String>,
    /// `true` when the judge marked the answer correct.
    pub label: Option<bool>,
    pub is_abstention: Option<bool>,
    pub judge_raw: Option<String>,
    pub judge_system_prompt: Option<String>,
    pub judge_user_prompt: Option<String>,
    pub judge_model: Option<String>,
    pub router_pick: Option<String>,
    pub initial_pick: Option<String>,
    pub final_pick: Option<String>,
    pub debug_artifact: Option<String>,
    pub error: Option<String>,
}

fn artifact_path(run_root: &Path, name: &str) -> std::path::PathBuf {
    run_root.join("artifacts").join(name)
}

/// Single source of truth for the (kind, filename, is_jsonl) mapping of artifacts
/// served from a run's `artifacts/` directory. Mirrored by the API layer in
/// `membench-server.rs` so the registry scanner can detect post-hoc artifacts
/// (e.g. `gold-eval.json` written after the original `benchmark-report.json`
/// was sealed) without a report re-write.
///
/// Keep this in sync with `membench_server::artifact_file` (which delegates here).
pub fn artifact_file(kind: &str) -> Option<(&'static str, bool)> {
    Some(match kind {
        "hypotheses" => ("hypotheses.jsonl", true),
        "verdicts" => ("verdicts.jsonl", true),
        "partial_verdicts" => ("partial-verdicts.jsonl", true),
        "provenance" => ("provenance.jsonl", true),
        "memory_traces" => ("memory-traces.jsonl", true),
        "model_traces" => ("model-traces.jsonl", true),
        "step_analytics" => ("step-analytics.json", false),
        "scored" => ("scored.json", false),
        "score_summary" => ("score-summary.json", false),
        "gold_eval" => ("gold-eval.json", false),
        _ => return None,
    })
}

/// All known artifact kinds, in display order.
pub const KNOWN_ARTIFACT_KINDS: &[&str] = &[
    "hypotheses",
    "verdicts",
    "partial_verdicts",
    "provenance",
    "memory_traces",
    "model_traces",
    "step_analytics",
    "scored",
    "score_summary",
    "gold_eval",
];

/// Walk every known artifact kind and return those present on disk for `run_root`.
/// Used to surface post-hoc artifacts (notably `gold_eval`) in the registry when
/// the original `benchmark-report.json`'s `artifact_manifest` is stale.
pub fn discover_artifacts_on_disk(run_root: &Path) -> Vec<String> {
    KNOWN_ARTIFACT_KINDS
        .iter()
        .filter(|kind| {
            artifact_file(kind)
                .map(|(name, _)| artifact_path(run_root, name).exists())
                .unwrap_or(false)
        })
        .map(|kind| kind.to_string())
        .collect()
}

/// Read a JSONL artifact, skipping blank and unparseable lines.
fn read_jsonl<T: for<'de> Deserialize<'de>>(path: &Path) -> Vec<T> {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    raw.lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| serde_json::from_str::<T>(line).ok())
        .collect()
}

/// Read verdicts, falling back to partial verdicts when the final file is absent.
pub fn read_verdicts(run_root: &Path) -> Vec<Verdict> {
    let primary = artifact_path(run_root, "verdicts.jsonl");
    if primary.is_file() {
        return read_jsonl(&primary);
    }
    read_jsonl(&artifact_path(run_root, "partial-verdicts.jsonl"))
}

pub fn read_hypotheses(run_root: &Path) -> Vec<Hypothesis> {
    read_jsonl(&artifact_path(run_root, "hypotheses.jsonl"))
}

pub fn read_provenance(run_root: &Path) -> Vec<Provenance> {
    read_jsonl(&artifact_path(run_root, "provenance.jsonl"))
}

/// Parse `scored.json` as untyped JSON for passthrough to the explorer.
pub fn read_scored(run_root: &Path) -> Option<Value> {
    let raw = std::fs::read_to_string(artifact_path(run_root, "scored.json")).ok()?;
    serde_json::from_str(&raw).ok()
}

/// The judge model recorded in a pre-parsed `scored.json`. Pass the value
/// returned by [`read_scored`] so multiple lookups share a single file read
/// (`scored.json` is large; reading it once per field was the dominant cost on
/// the bulk index path).
pub fn judge_model_from(scored: Option<&Value>) -> Option<String> {
    scored?.get("judge_model")?.as_str().map(ToOwned::to_owned)
}

/// The judge prompt mode recorded in a pre-parsed `scored.json`.
pub fn judge_prompt_mode_from(scored: Option<&Value>) -> Option<String> {
    scored?
        .get("judge_prompt_mode")?
        .as_str()
        .map(ToOwned::to_owned)
}

/// The judge model recorded in `scored.json`, when present.
pub fn judge_model(run_root: &Path) -> Option<String> {
    judge_model_from(read_scored(run_root).as_ref())
}

/// The judge prompt mode recorded in `scored.json`, when present.
pub fn judge_prompt_mode(run_root: &Path) -> Option<String> {
    judge_prompt_mode_from(read_scored(run_root).as_ref())
}

/// The sorted, de-duplicated set of question ids this run covered. Used to
/// fingerprint "the same questions" across runs. Prefers verdicts (which always
/// carry a question id) and falls back to hypotheses.
pub fn question_ids(run_root: &Path) -> Vec<String> {
    let mut ids: Vec<String> = read_verdicts(run_root)
        .into_iter()
        .map(|verdict| verdict.question_id)
        .collect();
    if ids.is_empty() {
        ids = read_hypotheses(run_root)
            .into_iter()
            .map(|hypothesis| hypothesis.question_id)
            .collect();
    }
    ids.sort();
    ids.dedup();
    ids
}

/// Build the merged per-question rows for a run.
pub fn question_rows(run_root: &Path) -> Vec<QuestionRow> {
    let mut rows: BTreeMap<String, QuestionRow> = BTreeMap::new();

    for verdict in read_verdicts(run_root) {
        let row = rows
            .entry(verdict.question_id.clone())
            .or_insert_with(|| QuestionRow {
                question_id: verdict.question_id.clone(),
                ..Default::default()
            });
        row.question_type = verdict.question_type;
        row.question = verdict.question;
        row.gold_answer = verdict.answer;
        row.hypothesis = verdict.hypothesis;
        row.label = verdict.label;
        row.is_abstention = verdict.is_abstention;
        row.judge_raw = verdict.judge_raw;
        row.judge_system_prompt = verdict.judge_system_prompt;
        row.judge_user_prompt = verdict.judge_user_prompt;
        row.judge_model = verdict.autoeval_label.and_then(|label| label.model);
        row.error = verdict.error;
    }

    for hypothesis in read_hypotheses(run_root) {
        let row = rows
            .entry(hypothesis.question_id.clone())
            .or_insert_with(|| QuestionRow {
                question_id: hypothesis.question_id.clone(),
                ..Default::default()
            });
        if row.hypothesis.is_none() {
            row.hypothesis = hypothesis.hypothesis;
        }
        if row.question_type.is_none() {
            row.question_type = hypothesis.question_type;
        }
        if row.question.is_none() {
            row.question = hypothesis.question;
        }
        row.router_pick = hypothesis.router_pick;
        if row.debug_artifact.is_none() {
            row.debug_artifact = hypothesis.debug_artifact;
        }
    }

    for provenance in read_provenance(run_root) {
        let row = rows
            .entry(provenance.question_id.clone())
            .or_insert_with(|| QuestionRow {
                question_id: provenance.question_id.clone(),
                ..Default::default()
            });
        row.initial_pick = provenance.initial_pick;
        row.final_pick = provenance.final_pick;
        if row.debug_artifact.is_none() {
            row.debug_artifact = provenance.debug_artifact;
        }
    }

    rows.into_values().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, contents: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    }

    #[test]
    fn question_rows_join_verdicts_hypotheses_provenance() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(
            &artifact_path(root, "verdicts.jsonl"),
            "{\"question_id\":\"q1\",\"question_type\":\"single-session-user\",\"question\":\"deg?\",\"answer\":\"BA\",\"hypothesis\":\"BA\",\"judge_raw\":\"yes\",\"autoeval_label\":{\"model\":\"deepseek-v4-flash\",\"label\":true},\"label\":true,\"is_abstention\":false}\n",
        );
        write(
            &artifact_path(root, "hypotheses.jsonl"),
            "{\"hypothesis\":\"BA\",\"question_id\":\"q1\",\"question_type\":\"single-session-user\",\"question\":\"degree?\",\"router_pick\":\"x-tgg\",\"debug_artifact\":\"vaults/q1/debug/hypotheses/q1/question-debug.json\"}\n",
        );
        write(
            &artifact_path(root, "provenance.jsonl"),
            "{\"question_id\":\"q1\",\"initial_pick\":\"x-tgg\",\"final_pick\":\"x-tgg\"}\n",
        );

        let rows = question_rows(root);
        assert_eq!(rows.len(), 1);
        let row = &rows[0];
        assert_eq!(row.question_id, "q1");
        assert_eq!(row.question_type.as_deref(), Some("single-session-user"));
        assert_eq!(row.question.as_deref(), Some("deg?"));
        assert_eq!(row.label, Some(true));
        assert_eq!(row.router_pick.as_deref(), Some("x-tgg"));
        assert_eq!(row.final_pick.as_deref(), Some("x-tgg"));
        assert_eq!(row.judge_model.as_deref(), Some("deepseek-v4-flash"));
        assert_eq!(
            row.debug_artifact.as_deref(),
            Some("vaults/q1/debug/hypotheses/q1/question-debug.json")
        );
    }

    #[test]
    fn malformed_lines_are_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(
            &artifact_path(root, "verdicts.jsonl"),
            "not json\n{\"question_id\":\"q2\",\"label\":false}\n\n",
        );
        let ids = question_ids(root);
        assert_eq!(ids, vec!["q2".to_string()]);
    }

    #[test]
    fn unscored_hypotheses_still_populate_question_browser_fields() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(
            &artifact_path(root, "hypotheses.jsonl"),
            "{\"question_id\":\"q1\",\"question_type\":\"temporal-reasoning\",\"question\":\"when?\",\"hypothesis\":\"Tuesday\"}\n",
        );

        let rows = question_rows(root);
        assert_eq!(rows.len(), 1);
        let row = &rows[0];
        assert_eq!(row.question_id, "q1");
        assert_eq!(row.question_type.as_deref(), Some("temporal-reasoning"));
        assert_eq!(row.question.as_deref(), Some("when?"));
        assert_eq!(row.hypothesis.as_deref(), Some("Tuesday"));
        assert_eq!(row.gold_answer, None);
        assert_eq!(row.label, None);
    }
}
