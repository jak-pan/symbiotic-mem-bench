//! Trial derivation for typed benchmark improvement ledgers.
//!
//! A trial compares one candidate run to a declared comparison run and
//! optionally to an original baseline stack. It writes appendable JSON/JSONL
//! artifacts that are easy to query without re-opening dashboard debug views.

use crate::artifacts::{self, QuestionRow};
use chrono::Utc;
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

const TRIAL_MIN_QUESTIONS: usize = 25;
const TRIAL_MAX_QUESTIONS: usize = 50;

#[derive(Clone, Debug)]
pub struct TrialDeriveOptions {
    pub stack_id: String,
    pub output_dir: PathBuf,
    pub trial_run_root: PathBuf,
    pub comparison_run_root: PathBuf,
    pub original_baseline_run_root: Option<PathBuf>,
    pub change_id: String,
    pub change_title: String,
    pub reasoning: String,
    pub changed_files: Vec<ChangedFileInput>,
    pub verification: Vec<String>,
    pub risks: Vec<String>,
    pub decision: String,
    pub force: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct ChangedFileInput {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub area: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
struct TrialStack {
    schema: &'static str,
    stack_id: String,
    created_at: String,
    terminology: Value,
    system: Option<String>,
    benchmark: Option<String>,
    baseline_runs: Value,
    failure_buckets: Value,
    sample_policy: Value,
    question_types: BTreeMap<String, u64>,
    question_count: usize,
    rules: Vec<&'static str>,
}

#[derive(Clone, Debug, Serialize)]
struct TrialRow {
    schema: &'static str,
    stack_id: String,
    run_id: String,
    run_path: String,
    change_id: String,
    change_title: String,
    compared_to_run_id: String,
    original_baseline_run_id: String,
    reasoning: String,
    changed_files: Vec<ChangedFileInput>,
    verification: Value,
    sample_policy: Value,
    aggregate: Value,
    outcomes: Value,
    debug_materials: Value,
    risks: Vec<String>,
    decision: String,
    created_at: String,
}

#[derive(Clone, Debug, Serialize)]
struct TrialQuestionDelta {
    schema: &'static str,
    stack_id: String,
    run_id: String,
    change_id: String,
    question_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    question_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    question: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    gold_answer: Option<String>,
    comparison_run_id: String,
    comparison: AnswerSnapshot,
    original_baseline_run_id: String,
    original_baseline: AnswerSnapshot,
    current: AnswerSnapshot,
    outcome: String,
    original_outcome: String,
    debug: Value,
    notes: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
struct AnswerSnapshot {
    label: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    hypothesis: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    judge_raw: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

pub fn derive_trial(options: TrialDeriveOptions) -> anyhow::Result<()> {
    std::fs::create_dir_all(&options.output_dir)?;

    let trial = RunView::load(&options.trial_run_root)?;
    let comparison = RunView::load(&options.comparison_run_root)?;
    let original = RunView::load(
        options
            .original_baseline_run_root
            .as_deref()
            .unwrap_or(&options.comparison_run_root),
    )?;

    let deltas = derive_question_deltas(&options, &trial, &comparison, &original)?;
    let trial_row = derive_trial_row(&options, &trial, &comparison, &original, &deltas);
    let stack = derive_stack(&options, &trial, &comparison, &original);

    write_json(
        &options.output_dir.join("trial-stack.json"),
        &serde_json::to_value(stack)?,
    )?;
    append_or_replace_jsonl(
        &options.output_dir.join("trials.jsonl"),
        "run_id",
        &trial.run_id,
        &serde_json::to_value(trial_row)?,
        options.force,
    )?;
    append_or_replace_question_deltas(
        &options.output_dir.join("trial-question-deltas.jsonl"),
        &trial.run_id,
        &deltas,
        options.force,
    )?;
    Ok(())
}

fn derive_stack(
    options: &TrialDeriveOptions,
    trial: &RunView,
    comparison: &RunView,
    original: &RunView,
) -> TrialStack {
    let ids = trial_question_ids(trial, comparison, original);

    let mut question_types = BTreeMap::<String, u64>::new();
    let mut both_wrong = 0u64;
    let mut comparison_wrong_original_right = 0u64;
    let mut original_wrong_comparison_right = 0u64;
    let mut both_right = 0u64;
    for id in &ids {
        let comparison_label = comparison.rows.get(id).and_then(|row| row.label);
        let original_label = original.rows.get(id).and_then(|row| row.label);
        match (comparison_label, original_label) {
            (Some(false), Some(false)) => both_wrong += 1,
            (Some(false), Some(true)) => comparison_wrong_original_right += 1,
            (Some(true), Some(false)) => original_wrong_comparison_right += 1,
            (Some(true), Some(true)) => both_right += 1,
            _ => {}
        }
        let qtype = trial
            .rows
            .get(id)
            .or_else(|| comparison.rows.get(id))
            .or_else(|| original.rows.get(id))
            .and_then(|row| row.question_type.clone())
            .unwrap_or_else(|| "unknown".to_string());
        *question_types.entry(qtype).or_default() += 1;
    }

    TrialStack {
        schema: "membench.trial_stack.v1",
        stack_id: options.stack_id.clone(),
        created_at: Utc::now().to_rfc3339(),
        terminology: json!({
            "preferred_name": "Trials",
            "avoided_names": ["fine-tuning", "bad-turn improvement"],
            "rationale": "Trials are typed, repeatable benchmark improvement experiments. No model weights are trained, and no question-specific answers are patched."
        }),
        system: trial.system.clone().or_else(|| comparison.system.clone()),
        benchmark: trial
            .benchmark
            .clone()
            .or_else(|| comparison.benchmark.clone()),
        baseline_runs: json!({
            "trial": trial.run_summary(),
            "comparison": comparison.run_summary(),
            "original_baseline": original.run_summary(),
        }),
        failure_buckets: json!({
            "both_wrong": both_wrong,
            "comparison_wrong_original_right": comparison_wrong_original_right,
            "original_wrong_comparison_right": original_wrong_comparison_right,
            "both_right": both_right,
        }),
        sample_policy: sample_policy(ids.len()),
        question_types,
        question_count: ids.len(),
        rules: vec![
            "Use trials for generic system changes only.",
            "Do not use question id, gold answer, verdict, exact residual phrase, or benchmark-topic routing.",
            "Use focused sub-25 question stacks for one failure class; use stratified 25-50 question stacks for broader diagnostic trial evidence.",
            "Record regressions with the same care as improvements.",
            "Do not treat a small diagnostic subset as a benchmark claim.",
            "Keep raw prompts and secret-bearing traces out of tracked records.",
        ],
    }
}

fn derive_trial_row(
    options: &TrialDeriveOptions,
    trial: &RunView,
    comparison: &RunView,
    original: &RunView,
    deltas: &[TrialQuestionDelta],
) -> TrialRow {
    let improvements: Vec<Value> = deltas
        .iter()
        .filter(|delta| delta.outcome == "improved_vs_comparison")
        .map(|delta| {
            json!({
                "question_id": delta.question_id,
                "from": delta.comparison.hypothesis,
                "to": delta.current.hypothesis,
            })
        })
        .collect();
    let regressions: Vec<Value> = deltas
        .iter()
        .filter(|delta| delta.outcome == "regressed_vs_comparison")
        .map(|delta| {
            json!({
                "question_id": delta.question_id,
                "from": delta.comparison.hypothesis,
                "to": delta.current.hypothesis,
            })
        })
        .collect();
    let unchanged_wrong: Vec<String> = deltas
        .iter()
        .filter(|delta| delta.outcome == "unchanged_wrong_vs_comparison")
        .map(|delta| delta.question_id.clone())
        .collect();
    let unchanged_correct: Vec<String> = deltas
        .iter()
        .filter(|delta| delta.outcome == "unchanged_correct_vs_comparison")
        .map(|delta| delta.question_id.clone())
        .collect();
    let original_regressions: Vec<String> = deltas
        .iter()
        .filter(|delta| delta.original_outcome == "regressed_from_original_baseline")
        .map(|delta| delta.question_id.clone())
        .collect();

    TrialRow {
        schema: "membench.trial.v1",
        stack_id: options.stack_id.clone(),
        run_id: trial.run_id.clone(),
        run_path: portable_path(&trial.run_root),
        change_id: options.change_id.clone(),
        change_title: options.change_title.clone(),
        compared_to_run_id: comparison.run_id.clone(),
        original_baseline_run_id: original.run_id.clone(),
        reasoning: options.reasoning.clone(),
        changed_files: options.changed_files.clone(),
        verification: json!({ "commands": options.verification }),
        sample_policy: sample_policy(deltas.len()),
        aggregate: trial.aggregate.clone(),
        outcomes: json!({
            "improvements": improvements,
            "regressions": regressions,
            "unchanged_wrong": unchanged_wrong,
            "unchanged_correct": unchanged_correct,
            "regressions_from_original_baseline": original_regressions,
        }),
        debug_materials: json!({
            "question_delta_rows": deltas.len(),
            "question_debug_rows": deltas.iter().filter(|delta| delta.debug.get("current").is_some()).count(),
            "source": "derived from standard run artifacts: scored/verdicts, hypotheses, provenance, memory traces, model traces, and question-debug bundles when present"
        }),
        risks: options.risks.clone(),
        decision: options.decision.clone(),
        created_at: Utc::now().to_rfc3339(),
    }
}

fn sample_policy(question_count: usize) -> Value {
    let class = if question_count < TRIAL_MIN_QUESTIONS {
        "focused_trial"
    } else if question_count <= TRIAL_MAX_QUESTIONS {
        "diagnostic_trial"
    } else if question_count < 500 {
        "broad_diagnostic"
    } else {
        "benchmark_scale"
    };
    json!({
        "question_count": question_count,
        "classification": class,
        "recommended_trial_range": {
            "min": TRIAL_MIN_QUESTIONS,
            "max": TRIAL_MAX_QUESTIONS
        },
        "focused": question_count < TRIAL_MIN_QUESTIONS,
        "note": if question_count < TRIAL_MIN_QUESTIONS {
            "Focused stack for one failure class or prompt-forensics pass; derive a 25-50 question stratified trial before broad conclusions."
        } else if question_count <= TRIAL_MAX_QUESTIONS {
            "Suitable for diagnostic improvement trials when stratified across failure buckets and question types."
        } else if question_count < 500 {
            "Larger than the normal diagnostic band; useful for confirmation but still not a full benchmark claim."
        } else {
            "Full benchmark scale; publish only with complete artifacts and no-cheating review."
        }
    })
}

fn derive_question_deltas(
    options: &TrialDeriveOptions,
    trial: &RunView,
    comparison: &RunView,
    original: &RunView,
) -> anyhow::Result<Vec<TrialQuestionDelta>> {
    let ids = trial_question_ids(trial, comparison, original);

    let mut deltas = Vec::new();
    for id in ids {
        let current = trial.rows.get(&id);
        let compared = comparison.rows.get(&id);
        let original_row = original.rows.get(&id);
        let question_row = current.or(compared).or(original_row);
        let current_snapshot = snapshot(current);
        let comparison_snapshot = snapshot(compared);
        let original_snapshot = snapshot(original_row);
        deltas.push(TrialQuestionDelta {
            schema: "membench.trial_question_delta.v1",
            stack_id: options.stack_id.clone(),
            run_id: trial.run_id.clone(),
            change_id: options.change_id.clone(),
            question_id: id.clone(),
            question_type: question_row.and_then(|row| row.question_type.clone()),
            question: question_row.and_then(|row| row.question.clone()),
            gold_answer: question_row.and_then(|row| row.gold_answer.clone()),
            comparison_run_id: comparison.run_id.clone(),
            comparison: comparison_snapshot,
            original_baseline_run_id: original.run_id.clone(),
            original_baseline: original_snapshot,
            current: current_snapshot,
            outcome: outcome(
                compared.and_then(|row| row.label),
                current.and_then(|row| row.label),
            ),
            original_outcome: original_outcome(
                original_row.and_then(|row| row.label),
                current.and_then(|row| row.label),
            ),
            debug: json!({
                "current": debug_summary(&trial.run_root, current),
                "comparison": debug_summary(&comparison.run_root, compared),
                "original_baseline": debug_summary(&original.run_root, original_row),
            }),
            notes: Vec::new(),
        });
    }
    Ok(deltas)
}

fn trial_question_ids(
    trial: &RunView,
    comparison: &RunView,
    original: &RunView,
) -> BTreeSet<String> {
    let mut ids = BTreeSet::new();
    ids.extend(trial.rows.keys().cloned());
    if ids.is_empty() {
        ids.extend(comparison.rows.keys().cloned());
        ids.extend(original.rows.keys().cloned());
    }
    ids
}

fn snapshot(row: Option<&QuestionRow>) -> AnswerSnapshot {
    row.map(|row| AnswerSnapshot {
        label: row.label,
        hypothesis: row.hypothesis.clone(),
        judge_raw: row.judge_raw.clone(),
        error: row.error.clone(),
    })
    .unwrap_or_default()
}

fn outcome(before: Option<bool>, after: Option<bool>) -> String {
    match (before, after) {
        (Some(false), Some(true)) => "improved_vs_comparison",
        (Some(true), Some(false)) => "regressed_vs_comparison",
        (_, Some(true)) => "unchanged_correct_vs_comparison",
        _ => "unchanged_wrong_vs_comparison",
    }
    .to_string()
}

fn original_outcome(before: Option<bool>, after: Option<bool>) -> String {
    match (before, after) {
        (Some(false), Some(true)) => "fixed_from_original_baseline",
        (Some(true), Some(false)) => "regressed_from_original_baseline",
        (_, Some(true)) => "still_correct_from_original_baseline",
        _ => "still_wrong_from_original_baseline",
    }
    .to_string()
}

fn debug_summary(run_root: &Path, row: Option<&QuestionRow>) -> Option<Value> {
    let row = row?;
    let debug_artifact = row.debug_artifact.as_ref()?;
    let debug_path = resolve_debug_artifact(run_root, debug_artifact)?;
    let debug_json = std::fs::read_to_string(&debug_path).ok()?;
    let parsed: Value = serde_json::from_str(&debug_json).ok()?;
    let sha256 = Sha256::digest(debug_json.as_bytes());
    Some(json!({
        "path": portable_path(&debug_path),
        "sha256": format!("{sha256:x}"),
        "query_planner_call": hash_if_present(&parsed, &["recall", "query_planner_call"]),
        "retrieval_query_count": nested(&parsed, &["recall", "retrieval_queries"]).and_then(Value::as_array).map(Vec::len),
        "answerer_call_count": nested(&parsed, &["recall", "answerer_calls"]).and_then(Value::as_array).map(Vec::len),
        "answer_system_prompt_hash": nested(&parsed, &["recall", "answerer_calls"])
            .and_then(Value::as_array)
            .and_then(|calls| calls.first())
            .and_then(|call| call.get("system_prompt"))
            .and_then(Value::as_str)
            .map(hash_text),
        "answer_prompt_hash": nested(&parsed, &["recall", "answerer_calls"])
            .and_then(Value::as_array)
            .and_then(|calls| calls.first())
            .and_then(|call| call.get("prompt"))
            .and_then(Value::as_str)
            .map(hash_text),
    }))
}

fn resolve_debug_artifact(run_root: &Path, debug_artifact: &str) -> Option<PathBuf> {
    let direct = run_root.join(debug_artifact);
    if direct.is_file() {
        return Some(direct);
    }
    let fallback = run_root.join("artifacts").join(debug_artifact);
    fallback.is_file().then_some(fallback)
}

fn nested<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut cursor = value;
    for key in path {
        cursor = cursor.get(*key)?;
    }
    Some(cursor)
}

fn hash_if_present(value: &Value, path: &[&str]) -> Option<String> {
    nested(value, path).map(|value| hash_text(&value.to_string()))
}

fn hash_text(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

#[derive(Clone, Debug)]
struct RunView {
    run_root: PathBuf,
    run_id: String,
    system: Option<String>,
    benchmark: Option<String>,
    rows: BTreeMap<String, QuestionRow>,
    aggregate: Value,
}

impl RunView {
    fn load(run_root: &Path) -> anyhow::Result<Self> {
        if !run_root.is_dir() {
            anyhow::bail!("run root does not exist: {}", run_root.display());
        }
        let run_id = run_root
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("unknown-run")
            .to_string();
        let report = read_optional_json(&run_root.join("benchmark-report.json"));
        let params = read_optional_json(&run_root.join("run-params.json"));
        let scored = artifacts::read_scored(run_root);
        let aggregate = aggregate_from(report.as_ref(), scored.as_ref());
        Ok(Self {
            run_root: run_root.to_path_buf(),
            run_id,
            system: string_at(report.as_ref(), &["system"])
                .or_else(|| string_at(params.as_ref(), &["system"])),
            benchmark: string_at(report.as_ref(), &["benchmark"])
                .or_else(|| string_at(params.as_ref(), &["benchmark"])),
            rows: artifacts::question_rows(run_root)
                .into_iter()
                .map(|row| (row.question_id.clone(), row))
                .collect(),
            aggregate,
        })
    }

    fn run_summary(&self) -> Value {
        json!({
            "run_id": self.run_id,
            "run_path": portable_path(&self.run_root),
            "system": self.system,
            "benchmark": self.benchmark,
            "aggregate": self.aggregate,
        })
    }
}

fn aggregate_from(report: Option<&Value>, scored: Option<&Value>) -> Value {
    if let Some(metrics) = report.and_then(|report| report.get("metrics")) {
        return metrics.clone();
    }
    json!({
        "accuracy": {
            "correct": scored.and_then(|s| nested(s, &["counts", "total_correct"])).and_then(Value::as_u64),
            "total": scored.and_then(|s| nested(s, &["counts", "scored"])).and_then(Value::as_u64),
            "value": scored.and_then(|s| s.get("overall_accuracy")).and_then(Value::as_f64),
        },
        "task_averaged_accuracy": scored.and_then(|s| s.get("task_averaged_accuracy")).and_then(Value::as_f64),
        "abstention_accuracy": scored.and_then(|s| s.get("abstention_accuracy")).and_then(Value::as_f64),
    })
}

fn string_at(value: Option<&Value>, path: &[&str]) -> Option<String> {
    nested(value?, path)?.as_str().map(ToOwned::to_owned)
}

fn read_optional_json(path: &Path) -> Option<Value> {
    let raw = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

fn write_json(path: &Path, value: &Value) -> anyhow::Result<()> {
    std::fs::write(path, serde_json::to_string_pretty(value)? + "\n")?;
    Ok(())
}

fn append_or_replace_jsonl(
    path: &Path,
    identity_key: &str,
    identity_value: &str,
    row: &Value,
    force: bool,
) -> anyhow::Result<()> {
    let mut rows = read_jsonl_values(path)?;
    let before = rows.len();
    rows.retain(|existing| {
        existing.get(identity_key).and_then(Value::as_str) != Some(identity_value)
    });
    if before != rows.len() && !force {
        anyhow::bail!(
            "{} already has a row with {identity_key}={identity_value}; pass --force to replace it",
            path.display()
        );
    }
    rows.push(row.clone());
    write_jsonl_values(path, &rows)
}

fn append_or_replace_question_deltas(
    path: &Path,
    run_id: &str,
    deltas: &[TrialQuestionDelta],
    force: bool,
) -> anyhow::Result<()> {
    let mut rows = read_jsonl_values(path)?;
    let before = rows.len();
    rows.retain(|existing| existing.get("run_id").and_then(Value::as_str) != Some(run_id));
    if before != rows.len() && !force {
        anyhow::bail!(
            "{} already has rows for run_id={run_id}; pass --force to replace them",
            path.display()
        );
    }
    for delta in deltas {
        rows.push(serde_json::to_value(delta)?);
    }
    write_jsonl_values(path, &rows)
}

fn read_jsonl_values(path: &Path) -> anyhow::Result<Vec<Value>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = std::fs::read_to_string(path)?;
    raw.lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(idx, line)| {
            serde_json::from_str(line).map_err(|err| {
                anyhow::anyhow!(
                    "invalid JSONL line {} in {}: {err}",
                    idx + 1,
                    path.display()
                )
            })
        })
        .collect()
}

fn write_jsonl_values(path: &Path, rows: &[Value]) -> anyhow::Result<()> {
    let mut out = String::new();
    for row in rows {
        out.push_str(&serde_json::to_string(row)?);
        out.push('\n');
    }
    std::fs::write(path, out)?;
    Ok(())
}

fn portable_path(path: &Path) -> String {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.strip_prefix(&root)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, contents: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    }

    #[test]
    fn derive_trial_records_wins_and_regressions() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().join("base");
        let trial = dir.path().join("trial");
        let out = dir.path().join("analysis");
        write(
            &base.join("artifacts/verdicts.jsonl"),
            "{\"question_id\":\"q1\",\"question\":\"one\",\"answer\":\"a\",\"hypothesis\":\"x\",\"label\":false}\n{\"question_id\":\"q2\",\"question\":\"two\",\"answer\":\"b\",\"hypothesis\":\"b\",\"label\":true}\n",
        );
        write(
            &trial.join("artifacts/verdicts.jsonl"),
            "{\"question_id\":\"q1\",\"question\":\"one\",\"answer\":\"a\",\"hypothesis\":\"a\",\"label\":true}\n{\"question_id\":\"q2\",\"question\":\"two\",\"answer\":\"b\",\"hypothesis\":\"x\",\"label\":false}\n",
        );

        derive_trial(TrialDeriveOptions {
            stack_id: "stack".to_string(),
            output_dir: out.clone(),
            trial_run_root: trial,
            comparison_run_root: base.clone(),
            original_baseline_run_root: Some(base),
            change_id: "change".to_string(),
            change_title: "Change".to_string(),
            reasoning: "Test".to_string(),
            changed_files: vec![ChangedFileInput {
                path: "../symbiotic-memory/src/recall/prompt_policy.rs".to_string(),
                line: Some(10),
                area: Some("answer prompt".to_string()),
                summary: Some("tighten evidence handling".to_string()),
            }],
            verification: vec!["cargo test".to_string()],
            risks: vec![],
            decision: "diagnostic".to_string(),
            force: false,
        })
        .unwrap();

        let trial_row: Value = serde_json::from_str(
            std::fs::read_to_string(out.join("trials.jsonl"))
                .unwrap()
                .trim(),
        )
        .unwrap();
        assert_eq!(
            trial_row["outcomes"]["improvements"][0]["question_id"],
            "q1"
        );
        assert_eq!(trial_row["outcomes"]["regressions"][0]["question_id"], "q2");
        assert_eq!(trial_row["sample_policy"]["question_count"], 2);
        assert_eq!(
            trial_row["sample_policy"]["classification"],
            "focused_trial"
        );
        assert_eq!(trial_row["sample_policy"]["focused"], true);

        let stack: Value =
            serde_json::from_str(&std::fs::read_to_string(out.join("trial-stack.json")).unwrap())
                .unwrap();
        assert_eq!(stack["sample_policy"]["recommended_trial_range"]["min"], 25);
        assert_eq!(stack["sample_policy"]["recommended_trial_range"]["max"], 50);

        let deltas = std::fs::read_to_string(out.join("trial-question-deltas.jsonl")).unwrap();
        assert!(deltas.contains("improved_vs_comparison"));
        assert!(deltas.contains("regressed_vs_comparison"));
    }
}
