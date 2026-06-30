//! Scan the run/record registry into a typed index.
//!
//! `membench explore` and the dashboard both need the same thing: walk
//! `runs/` and `records/` for `benchmark-report.json` files and reduce each to a
//! compact, comparable [`RunSummary`]. The heavy cohort/cost derivation
//! ([`compute_cohort_fields`]) is shared with the report writer so that the
//! values surfaced live by the explorer are identical to the ones persisted into
//! reports at finalize time.

use crate::cohort::{self, Models};
use crate::cost;
use crate::jsonutil::{nested_bool, nested_f64, nested_str, nested_string, nested_u64};
use crate::{artifacts, stable_hash};
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// A run discovered on disk: its report, params, and location.
#[derive(Clone, Debug, Serialize)]
pub struct RunRecord {
    /// Repo-relative path to the run folder; also the stable id used in URLs.
    pub run_id: String,
    /// First path segment, e.g. `runs` or `records`.
    pub origin: String,
    #[serde(skip)]
    pub run_root: PathBuf,
    pub report: Value,
    pub params: Value,
    pub modified_ms: Option<i64>,
}

/// Compact, comparable index row for one run.
#[derive(Clone, Debug, Serialize)]
pub struct RunSummary {
    pub run_id: String,
    pub origin: String,
    pub system: String,
    pub benchmark: String,
    pub limit: Option<u64>,
    pub run_name: String,
    pub display_name: String,
    pub run_kind: String,
    pub registry_section: String,
    pub is_meta_record: bool,
    pub tuning_cohort: Option<String>,
    pub tuning_shape: Option<String>,
    pub config_label: String,
    pub settings_label: String,
    pub accuracy: Option<f64>,
    pub accuracy_correct: Option<u64>,
    pub accuracy_total: Option<u64>,
    pub task_averaged_accuracy: Option<f64>,
    pub abstention_accuracy: Option<f64>,
    pub cost_micro_usd: Option<u64>,
    pub latency_ms_p50: Option<f64>,
    pub latency_ms_p95: Option<f64>,
    pub config_signature: Option<String>,
    pub cohort_id: String,
    pub dataset_fingerprint: Option<String>,
    pub judge_model: Option<String>,
    pub judge_prompt_mode: Option<String>,
    /// Oracle-gold run: gold evidence fed straight to the answerer (reader-ceiling method).
    /// Read from `run-params.json` (`oracle_gold: bool`); absent → false.
    pub oracle_gold: bool,
    pub created_at: Option<String>,
    pub modified_ms: Option<i64>,
    pub per_question_type: Option<Value>,
    pub artifacts_available: Vec<String>,
    pub artifacts_missing: Vec<String>,
    pub native_state_available: Option<bool>,
    pub is_trial_run: bool,
    pub trial_markers: Vec<TrialMarker>,
}

/// Compact marker attached to benchmark runs that are referenced by a typed
/// Trial ledger under `runs/analysis/**/trials.jsonl`.
#[derive(Clone, Debug, Serialize)]
pub struct TrialMarker {
    pub stack_id: String,
    pub change_id: String,
    pub change_title: String,
    pub decision: String,
    pub analysis_path: String,
    pub compared_to_run_id: Option<String>,
    pub original_baseline_run_id: Option<String>,
    pub improvements: u64,
    pub regressions: u64,
    pub unchanged_wrong: u64,
    pub unchanged_correct: u64,
    pub question_count: u64,
    pub sample_classification: String,
    pub focused: bool,
    pub aggregate_accuracy: Option<f64>,
    pub aggregate_correct: Option<u64>,
    pub aggregate_total: Option<u64>,
}

/// Cohort/cost fields derived from a run's artifacts. Shared by the report
/// writer (persist once at finalize) and the server (compute lazily for detail).
#[derive(Clone, Debug, Default, Serialize)]
pub struct CohortFields {
    pub dataset_fingerprint: Option<String>,
    pub judge_model: Option<String>,
    pub judge_prompt_mode: Option<String>,
    #[serde(skip_serializing_if = "Models::is_empty")]
    pub models: Models,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub role_stats: Vec<cost::RoleStat>,
    pub config_signature: String,
    pub cost_micro_usd: Option<u64>,
    pub latency_ms_p50: Option<f64>,
    pub latency_ms_p95: Option<f64>,
    pub cached_input_tokens: Option<u64>,
    pub uncached_input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub response_cache_hits: Option<u64>,
    pub prompt_cache_hits: Option<u64>,
    pub prompt_cache_partial_hits: Option<u64>,
    pub prompt_cache_misses: Option<u64>,
}

/// Heavy or irrelevant state directories that never contain a run report.
/// Pruning them keeps a registry scan fast even when native runs leave large
/// `vaults/` or `provider-queue/` trees behind, and avoids following symlink
/// cycles into them.
const PRUNE_DIRS: &[&str] = &[
    "vaults",
    "workflow",
    "provider-queue",
    "raw",
    "artifacts",
    "archive",
    ".git",
    "target",
    "node_modules",
    "dist",
    ".debug-session",
];

/// Recursively collect `benchmark-report.json` paths beneath a directory.
/// Reports live at the run-root level only, so heavy state subdirectories are
/// pruned and symlinks are not followed.
pub fn collect_report_paths(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            let name = entry.file_name();
            if PRUNE_DIRS.contains(&name.to_string_lossy().as_ref()) {
                continue;
            }
            collect_report_paths(&entry.path(), out);
        } else if entry.file_name() == "benchmark-report.json" {
            out.push(entry.path());
        }
    }
}

/// Scan one or more registry roots into run records. `repo_root` is used to make
/// run ids repo-relative.
pub fn scan_registry(roots: &[PathBuf], repo_root: &Path) -> Vec<RunRecord> {
    let mut report_paths = Vec::new();
    for root in roots {
        collect_report_paths(root, &mut report_paths);
    }
    report_paths.sort();

    let mut records = Vec::new();
    for report_path in report_paths {
        let Ok(raw) = std::fs::read_to_string(&report_path) else {
            continue;
        };
        let Ok(report) = serde_json::from_str::<Value>(&raw) else {
            continue;
        };
        let Some(run_root) = report_path.parent().map(Path::to_path_buf) else {
            continue;
        };
        let params = std::fs::read_to_string(run_root.join("run-params.json"))
            .ok()
            .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
            .unwrap_or(Value::Null);
        let run_id = run_root
            .strip_prefix(repo_root)
            .unwrap_or(&run_root)
            .to_string_lossy()
            .replace('\\', "/");
        let origin = run_id.split('/').next().unwrap_or("runs").to_string();
        let modified_ms = std::fs::metadata(&report_path)
            .and_then(|meta| meta.modified())
            .ok()
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|delta| delta.as_millis() as i64);

        records.push(RunRecord {
            run_id,
            origin,
            run_root,
            report,
            params,
            modified_ms,
        });
    }
    records
}

/// Scan typed trial ledgers and return markers keyed by repo-relative run id.
pub fn scan_trial_markers(repo_root: &Path) -> BTreeMap<String, Vec<TrialMarker>> {
    let mut paths = Vec::new();
    collect_trial_paths(&repo_root.join("runs").join("analysis"), &mut paths);
    paths.sort();

    let mut markers: BTreeMap<String, Vec<TrialMarker>> = BTreeMap::new();
    for path in paths {
        let Ok(raw) = std::fs::read_to_string(&path) else {
            continue;
        };
        let analysis_path = path
            .parent()
            .unwrap_or(&path)
            .strip_prefix(repo_root)
            .unwrap_or_else(|_| path.parent().unwrap_or(&path))
            .to_string_lossy()
            .replace('\\', "/");
        for line in raw.lines().filter(|line| !line.trim().is_empty()) {
            let Ok(value) = serde_json::from_str::<Value>(line) else {
                continue;
            };
            let run_path =
                nested_string(&value, &["run_path"]).or_else(|| nested_string(&value, &["run_id"]));
            let Some(run_path) = run_path else {
                continue;
            };
            let run_id = normalize_run_path(&run_path, repo_root);
            let question_count = nested_u64(&value, &["sample_policy", "question_count"])
                .or_else(|| nested_u64(&value, &["aggregate", "accuracy", "total"]))
                .unwrap_or_else(|| count_trial_outcomes(&value));
            let sample_classification = nested_string(&value, &["sample_policy", "classification"])
                .map(|value| normalize_trial_sample_class(&value).to_string())
                .unwrap_or_else(|| classify_trial_sample(question_count).to_string());
            let focused = nested_bool(&value, &["sample_policy", "focused"])
                .or_else(|| nested_bool(&value, &["sample_policy", "underpowered"]))
                .unwrap_or(question_count < 25);

            let marker = TrialMarker {
                stack_id: nested_string(&value, &["stack_id"]).unwrap_or_default(),
                change_id: nested_string(&value, &["change_id"]).unwrap_or_default(),
                change_title: nested_string(&value, &["change_title"]).unwrap_or_default(),
                decision: nested_string(&value, &["decision"]).unwrap_or_default(),
                analysis_path: analysis_path.clone(),
                compared_to_run_id: nested_string(&value, &["compared_to_run_id"]),
                original_baseline_run_id: nested_string(&value, &["original_baseline_run_id"]),
                improvements: count_array(&value, &["outcomes", "improvements"]),
                regressions: count_array(&value, &["outcomes", "regressions"]),
                unchanged_wrong: count_array(&value, &["outcomes", "unchanged_wrong"]),
                unchanged_correct: count_array(&value, &["outcomes", "unchanged_correct"]),
                question_count,
                sample_classification,
                focused,
                aggregate_accuracy: nested_f64(&value, &["aggregate", "accuracy", "value"]),
                aggregate_correct: nested_u64(&value, &["aggregate", "accuracy", "correct"]),
                aggregate_total: nested_u64(&value, &["aggregate", "accuracy", "total"]),
            };
            markers.entry(run_id).or_default().push(marker);
        }
    }
    markers
}

/// Recursively collect `trials.jsonl` paths beneath a directory. Used by
/// [`scan_trial_markers`] and by the server's cache signature probe.
pub fn collect_trial_paths(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            collect_trial_paths(&entry.path(), out);
        } else if entry.file_name() == "trials.jsonl" {
            out.push(entry.path());
        }
    }
}

fn normalize_run_path(raw: &str, repo_root: &Path) -> String {
    let path = PathBuf::from(raw);
    if path.is_absolute() {
        path.strip_prefix(repo_root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/")
    } else {
        raw.trim_start_matches("./").replace('\\', "/")
    }
}

fn count_array(value: &Value, path: &[&str]) -> u64 {
    path.iter()
        .try_fold(value, |value, key| value.get(*key))
        .and_then(Value::as_array)
        .map(|items| items.len() as u64)
        .unwrap_or(0)
}

fn count_trial_outcomes(value: &Value) -> u64 {
    count_array(value, &["outcomes", "improvements"])
        + count_array(value, &["outcomes", "regressions"])
        + count_array(value, &["outcomes", "unchanged_wrong"])
        + count_array(value, &["outcomes", "unchanged_correct"])
}

fn classify_trial_sample(question_count: u64) -> &'static str {
    match question_count {
        0..=24 => "focused_trial",
        25..=50 => "diagnostic_trial",
        51..=499 => "broad_diagnostic",
        _ => "benchmark_scale",
    }
}

fn normalize_trial_sample_class(raw: &str) -> &str {
    match raw {
        "smoke" => "focused_trial",
        other => other,
    }
}

/// Reduce a record to its index row. This is the cheap path: it reads `scored`
/// and `verdicts` (≈ `limit` lines) for cohort identity but never the large
/// trace files — cost/latency come from the report when persisted.
pub fn summarize(record: &RunRecord) -> RunSummary {
    summarize_with_trials(record, &BTreeMap::new())
}

/// Reduce a record to its index row and attach trial metadata discovered from
/// `runs/analysis/**/trials.jsonl`.
pub fn summarize_with_trials(
    record: &RunRecord,
    trial_index: &BTreeMap<String, Vec<TrialMarker>>,
) -> RunSummary {
    let report = &record.report;
    let params = &record.params;
    let field = |key: &str| nested_string(report, &[key]).or_else(|| nested_string(params, &[key]));

    let system = field("system").unwrap_or_else(|| "unknown".to_string());
    let benchmark = field("benchmark").unwrap_or_else(|| "unknown".to_string());
    let run_name = field("run_name").unwrap_or_else(|| "unnamed".to_string());
    let run_kind = field("run_kind").unwrap_or_else(|| "unknown".to_string());
    let limit = nested_u64(report, &["run_params", "limit"])
        .or_else(|| nested_u64(params, &["limit"]))
        .or_else(|| nested_u64(report, &["metrics", "accuracy", "total"]));

    let dataset_fingerprint = nested_string(report, &["cohort", "dataset_fingerprint"])
        .or_else(|| cohort::dataset_fingerprint(&record.run_root));
    // `scored.json` is the largest per-run artifact after the trace files; read
    // it once and let judge/per-question-type lookups share the parse.
    let scored = artifacts::read_scored(&record.run_root);
    let judge_model = nested_string(report, &["cohort", "judge_model"])
        .or_else(|| artifacts::judge_model_from(scored.as_ref()));
    let judge_prompt_mode = nested_string(report, &["cohort", "judge_prompt_mode"])
        .or_else(|| nested_string(report, &["judge_prompt_mode"]))
        .or_else(|| artifacts::judge_prompt_mode_from(scored.as_ref()));
    let models = report
        .get("models")
        .and_then(|value| serde_json::from_value::<Models>(value.clone()).ok())
        .unwrap_or_default();
    let models = models_with_param_fallback(models, params, judge_model.as_deref());
    let config_signature = nested_string(report, &["config_signature"])
        .or_else(|| Some(cohort::config_signature(params, &models)));

    let cohort_id = cohort::cohort_id(
        &benchmark,
        limit,
        dataset_fingerprint.as_deref(),
        judge_model.as_deref(),
        judge_prompt_mode.as_deref(),
    );

    let manifest = report.get("artifact_manifest");
    let string_list = |key: &str| -> Vec<String> {
        manifest
            .and_then(|manifest| manifest.get(key))
            .and_then(|value| value.as_array())
            .map(|values| {
                values
                    .iter()
                    .filter_map(|value| value.as_str().map(ToOwned::to_owned))
                    .collect()
            })
            .unwrap_or_default()
    };

    let per_question_type = scored
        .as_ref()
        .and_then(|scored| scored.get("per_question_type").cloned());

    let trial_markers = trial_index.get(&record.run_id).cloned().unwrap_or_default();
    let is_trial_run = !trial_markers.is_empty();
    let is_meta_record = report
        .get("meta_record")
        .or_else(|| params.get("meta_record"))
        .is_some();
    let tuning_cohort = tuning_cohort(report, params, &run_name);
    let tuning_shape = tuning_shape(report, params, &run_name);
    let registry_section = if tuning_cohort.is_some() {
        "tuning"
    } else if is_trial_run {
        "trials"
    } else {
        "benchmarks"
    }
    .to_string();
    let display_name = display_name(&run_name, params, tuning_shape.as_deref());

    // Union the report's static artifact manifest with what is actually on disk
    // so post-hoc artifacts (notably `gold_eval`, written after the report was
    // sealed) are surfaced in the registry without a report re-write.
    let manifest_available = string_list("available");
    let disk_available = artifacts::discover_artifacts_on_disk(&record.run_root);
    let mut artifacts_available = Vec::<String>::new();
    for kind in artifacts::KNOWN_ARTIFACT_KINDS {
        if manifest_available.iter().any(|x| x == *kind)
            || disk_available.iter().any(|x| x == *kind)
        {
            artifacts_available.push((*kind).to_string());
        }
    }
    for kind in &manifest_available {
        if !artifacts_available.contains(kind) {
            artifacts_available.push(kind.clone());
        }
    }
    // A post-hoc artifact present on disk can no longer be "missing" — drop it.
    let artifacts_missing = string_list("missing")
        .into_iter()
        .filter(|kind| !artifacts_available.contains(kind))
        .collect::<Vec<_>>();

    RunSummary {
        run_id: record.run_id.clone(),
        origin: record.origin.clone(),
        config_label: config_label(&run_name, params, &models),
        settings_label: settings_label(params),
        system,
        benchmark,
        limit,
        run_name,
        display_name,
        run_kind,
        registry_section,
        is_meta_record,
        tuning_cohort,
        tuning_shape,
        accuracy: nested_f64(report, &["metrics", "accuracy", "value"]),
        accuracy_correct: nested_u64(report, &["metrics", "accuracy", "correct"]),
        accuracy_total: nested_u64(report, &["metrics", "accuracy", "total"]),
        task_averaged_accuracy: nested_f64(report, &["metrics", "task_averaged_accuracy"]),
        abstention_accuracy: nested_f64(report, &["metrics", "abstention_accuracy", "value"]),
        cost_micro_usd: nested_u64(report, &["metrics", "cost_micro_usd"]),
        latency_ms_p50: nested_f64(report, &["metrics", "latency_ms_p50"]),
        latency_ms_p95: nested_f64(report, &["metrics", "latency_ms_p95"]),
        config_signature,
        cohort_id,
        dataset_fingerprint,
        judge_model,
        judge_prompt_mode,
        oracle_gold: nested_bool(params, &["oracle_gold"]).unwrap_or(false),
        created_at: nested_string(report, &["created_at"]),
        modified_ms: record.modified_ms,
        per_question_type,
        artifacts_available,
        artifacts_missing,
        native_state_available: nested_bool(
            report,
            &["artifact_manifest", "native_state_available"],
        ),
        is_trial_run,
        trial_markers,
    }
}

/// Build a human-readable configuration label.
fn config_label(run_name: &str, params: &Value, models: &Models) -> String {
    let mut parts: Vec<String> = Vec::new();
    for key in ["distiller", "embedder", "store"] {
        if let Some(value) = nested_str(params, &[key]) {
            parts.push(value.to_string());
        }
    }
    if let Some(planner) = nested_str(params, &["query_planner"]) {
        parts.push(format!("plan:{planner}"));
    }
    if nested_bool(params, &["routed"]).unwrap_or(false) {
        parts.push("routed".to_string());
    }
    if let Some(answer) = &models.answer {
        parts.push(format!("ans:{answer}"));
    }
    if parts.is_empty() {
        run_name.to_string()
    } else {
        parts.join("·")
    }
}

fn tuning_cohort(report: &Value, params: &Value, run_name: &str) -> Option<String> {
    nested_string(report, &["tuning", "cohort"])
        .or_else(|| nested_string(params, &["tuning", "cohort"]))
        .or_else(|| {
            if is_openrouter_qwen_raw_embed(params, run_name) {
                Some("embed-transport/openrouter-qwen3-8b-1024-32k".to_string())
            } else if is_deepseek_chat_distill_transport(params, run_name) {
                Some("chat-transport/deepseek-v4-flash-distill".to_string())
            } else {
                None
            }
        })
}

fn tuning_shape(report: &Value, params: &Value, run_name: &str) -> Option<String> {
    nested_string(report, &["tuning", "shape"])
        .or_else(|| nested_string(params, &["tuning", "shape"]))
        .or_else(|| tuning_shape_from_params(params))
        .or_else(|| chat_tuning_shape_from_params(params))
        .or_else(|| tuning_shape_from_name(run_name))
}

fn display_name(run_name: &str, params: &Value, tuning_shape: Option<&str>) -> String {
    let Some(shape) = tuning_shape else {
        return run_name.to_string();
    };
    let is_chat_tuning = is_deepseek_chat_distill_transport(params, run_name);
    let model_role = if is_chat_tuning { "distill" } else { "embed" };
    let model = configured_model(params, model_role)
        .or_else(|| runtime_model(params, model_role))
        .map(|model| short_model_name(&model))
        .unwrap_or_else(|| {
            if is_chat_tuning {
                "deepseek-chat".to_string()
            } else {
                "embedding".to_string()
            }
        });
    if is_chat_tuning {
        return format!("{shape} · {model}");
    }
    let dims = nested_string(params, &["embed_request_dims"])
        .or_else(|| nested_string(params, &["embed_dims"]))
        .or_else(|| {
            if run_name.contains("qwen1024") {
                Some("1024".to_string())
            } else {
                None
            }
        })
        .map(|dims| format!(" {dims}d"))
        .unwrap_or_default();
    format!("{shape} · {model}{dims}")
}

fn short_model_name(model: &str) -> String {
    match model {
        value if value.contains("qwen3-embedding-8b") => "qwen3-emb-8b".to_string(),
        value if value.contains("deepseek-v4-flash") => "deepseek-v4-flash".to_string(),
        value => value
            .rsplit('/')
            .next()
            .filter(|part| !part.is_empty())
            .unwrap_or(value)
            .to_string(),
    }
}

fn is_openrouter_qwen_raw_embed(params: &Value, run_name: &str) -> bool {
    let raw_embed = nested_bool(params, &["stop_after_raw_embed"])
        .or_else(|| nested_bool(params, &["ingest_stop_after_raw_embed"]))
        .unwrap_or(false)
        || run_name.contains("rawembed");
    let model = configured_model(params, "embed")
        .or_else(|| runtime_model(params, "embed"))
        .unwrap_or_default();
    let openrouter = nested_string(params, &["embedder"]).as_deref() == Some("openrouter")
        || nested_string(params, &["role_settings", "embed", "operator"]).as_deref()
            == Some("openrouter")
        || model.contains("openrouter");
    raw_embed
        && openrouter
        && (model.contains("qwen3-embedding-8b") || run_name.contains("qwen1024"))
}

fn is_deepseek_chat_distill_transport(params: &Value, run_name: &str) -> bool {
    let lower = run_name.to_ascii_lowercase();
    let distill_only = nested_string(params, &["ingest_diagnostic"])
        .or_else(|| nested_string(params, &["ingest_diagnostic_mode"]))
        .as_deref()
        == Some("distill")
        || lower.contains("tune-chat")
        || lower.contains("ds-chat");
    let model = configured_model(params, "distill")
        .or_else(|| runtime_model(params, "distill"))
        .unwrap_or_default();
    let operator = nested_string(params, &["role_settings", "distill", "operator"])
        .or_else(|| nested_string(params, &["configured_models", "distill", "operator"]))
        .unwrap_or_default();
    distill_only
        && (operator == "deepseek" || model.contains("deepseek") || lower.contains("deepseek"))
        && (model.contains("deepseek-v4-flash") || lower.contains("deepseek-v4-flash"))
}

fn tuning_shape_from_params(params: &Value) -> Option<String> {
    let pool = nested_string(params, &["openrouter_http_client_pool_size"])?;
    let idle = nested_string(params, &["openrouter_http_pool_max_idle_per_host"])?;
    let protocol = match nested_bool(params, &["openrouter_http1_only"]) {
        Some(true) => "h1",
        Some(false) => "h2",
        None => "h?",
    };
    Some(format!("{protocol} {pool}x{idle}"))
}

fn chat_tuning_shape_from_params(params: &Value) -> Option<String> {
    let pool = nested_string(params, &["chat_http_client_pool_size"])?;
    let idle = nested_string(params, &["chat_http_pool_max_idle_per_host"])?;
    let protocol = match nested_bool(params, &["chat_http1_only"]) {
        Some(true) => "h1",
        Some(false) => "h2",
        None => "h?",
    };
    Some(format!("{protocol} {pool}x{idle}"))
}

fn tuning_shape_from_name(run_name: &str) -> Option<String> {
    let lower = run_name.to_ascii_lowercase();
    for prefix in ["http1-pool", "h2pool"] {
        let Some(start) = lower.find(prefix) else {
            continue;
        };
        let rest = &lower[start + prefix.len()..];
        let shape: String = rest
            .chars()
            .take_while(|ch| ch.is_ascii_digit() || *ch == 'x')
            .collect();
        if shape.contains('x') {
            let protocol = if prefix.starts_with("http1") {
                "h1"
            } else {
                "h2"
            };
            return Some(format!("{protocol} {shape}"));
        }
    }
    None
}

/// Derive cohort + cost fields for a run, reading trace files. Used at report
/// finalize (persisted) and by the server's run-detail endpoint.
pub fn compute_cohort_fields(run_root: &Path, params: &Value) -> CohortFields {
    let rollup = cost::rollup_model_traces(run_root);
    compute_cohort_fields_with_rollup(run_root, params, rollup.as_ref())
}

/// Same as [`compute_cohort_fields`] but accepts a precomputed model-trace
/// rollup, so callers that already need the rollup for their own payload (the
/// server's run-detail endpoint) don't parse `model-traces.jsonl` twice.
pub fn compute_cohort_fields_with_rollup(
    run_root: &Path,
    params: &Value,
    rollup: Option<&cost::ModelTraceRollup>,
) -> CohortFields {
    let dataset_fingerprint = cohort::dataset_fingerprint(run_root);
    let scored = artifacts::read_scored(run_root);
    let judge_model = artifacts::judge_model_from(scored.as_ref());
    let judge_prompt_mode = artifacts::judge_prompt_mode_from(scored.as_ref());
    let mut models = rollup
        .map(|rollup| Models::from_roles(&rollup.roles))
        .unwrap_or_default();
    models = models_with_param_fallback(models, params, judge_model.as_deref());
    let config_signature = cohort::config_signature(params, &models);
    CohortFields {
        dataset_fingerprint,
        judge_model,
        judge_prompt_mode,
        models,
        role_stats: rollup
            .map(|rollup| rollup.roles_detail.clone())
            .unwrap_or_default(),
        config_signature,
        cost_micro_usd: rollup.and_then(|rollup| rollup.cost_micro_usd),
        latency_ms_p50: rollup.and_then(|rollup| rollup.latency_ms_p50),
        latency_ms_p95: rollup.and_then(|rollup| rollup.latency_ms_p95),
        cached_input_tokens: rollup.map(|rollup| rollup.cached_input_tokens),
        uncached_input_tokens: rollup.map(|rollup| rollup.uncached_input_tokens),
        output_tokens: rollup.map(|rollup| rollup.output_tokens),
        response_cache_hits: rollup.map(|rollup| rollup.response_cache_hits),
        prompt_cache_hits: rollup.map(|rollup| rollup.prompt_cache_hits),
        prompt_cache_partial_hits: rollup.map(|rollup| rollup.prompt_cache_partial_hits),
        prompt_cache_misses: rollup.map(|rollup| rollup.prompt_cache_misses),
    }
}

fn models_with_param_fallback(
    mut models: Models,
    params: &Value,
    judge_model: Option<&str>,
) -> Models {
    if models.answer.is_none() {
        models.answer =
            configured_model(params, "answer").or_else(|| runtime_model(params, "answer"));
    }
    if models.distill.is_none() {
        models.distill =
            configured_model(params, "distill").or_else(|| runtime_model(params, "distill"));
    }
    if models.embed.is_none() {
        models.embed = configured_model(params, "embed").or_else(|| runtime_model(params, "embed"));
    }
    if models.rerank.is_none() {
        models.rerank = configured_rerank_model(params);
    }
    if models.judge.is_none() {
        models.judge = configured_model(params, "judge")
            .or_else(|| runtime_model(params, "judge"))
            .or_else(|| judge_model.map(ToOwned::to_owned));
    }
    models
}

fn configured_model(params: &Value, role: &str) -> Option<String> {
    nested_string(params, &["configured_models", role, "model"])
        .or_else(|| nested_string(params, &[&format!("{role}_model")]))
}

/// Resolve the reranker model recorded in `run-params.json`. membench writes the
/// resolved rerank binding both under `configured_models.rerank` and at the
/// top-level `rerank` key, shaped `{ enabled, model, .. }`. When rerank is off
/// the binding is `{ "enabled": false }` with a null/absent model, so honour the
/// `enabled` flag and require a concrete model before reporting one. This is the
/// only role not surfaced via the model-trace rollup, so without this fallback
/// the Overview shows the rerank model as "none" even when it was configured.
fn configured_rerank_model(params: &Value) -> Option<String> {
    for base in [
        params.get("configured_models").and_then(|m| m.get("rerank")),
        params.get("rerank"),
    ]
    .into_iter()
    .flatten()
    {
        // Skip explicitly-disabled rerank bindings.
        if base.get("enabled") == Some(&Value::Bool(false)) {
            continue;
        }
        if let Some(model) = base
            .get("model")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|model| !model.is_empty())
        {
            return Some(model.to_string());
        }
    }
    None
}

fn runtime_model(params: &Value, role: &str) -> Option<String> {
    let raw = nested_string(params, &["runtime_models", role])?;
    // Queue keys are `status:operator:model`, and `model` may itself contain ':'
    // (e.g. an OpenRouter `:free` suffix) — take everything after the 2nd colon, not the
    // last segment. `rsplit(':').next()` would wrongly yield "free" for `.../model:free`.
    Some(
        raw.splitn(3, ':')
            .nth(2)
            .filter(|part| !part.is_empty())
            .unwrap_or(&raw)
            .to_string(),
    )
}

/// Stable hash of a run id, handy for cache keys.
pub fn run_id_hash(run_id: &str) -> String {
    stable_hash(run_id.as_bytes())
}

/// A run folder that has started (has `run-params.json`) but has not finalized
/// (no `benchmark-report.json`). These are in-flight or stalled runs.
#[derive(Clone, Debug, Serialize)]
pub struct PendingRun {
    pub run_id: String,
    pub origin: String,
    pub system: String,
    pub benchmark: String,
    pub limit: Option<u64>,
    pub run_name: String,
    pub config_label: String,
    pub settings_label: String,
    /// `running` (recent file activity) or `stalled`.
    pub status: String,
    pub started_ms: Option<i64>,
    pub updated_ms: Option<i64>,
    pub age_secs: Option<i64>,
    /// Hypotheses produced so far (answer progress).
    pub hypotheses: u64,
    /// Question vaults built so far (ingest progress).
    pub ingested: u64,
    /// Oracle-gold run: gold evidence fed straight to the answerer (reader-ceiling method).
    pub oracle_gold: bool,
}

/// Idle thresholds for a run's liveness. Under `RUNNING` it's actively writing;
/// between the two it's idle (a soft warning — possibly stalling, possibly just
/// a batch/quota gap); past `STALLED` it's treated as stalled.
const RUNNING_WINDOW_MS: i64 = 180_000;
const STALLED_WINDOW_MS: i64 = 300_000;

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|delta| delta.as_millis() as i64)
        .unwrap_or(0)
}

fn file_mtime_ms(path: &Path) -> Option<i64> {
    std::fs::metadata(path)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|delta| delta.as_millis() as i64)
}

fn count_nonempty_lines(path: &Path) -> u64 {
    count_nonempty_lines_opt(path).unwrap_or(0)
}

/// Count lines that contain at least one non-whitespace byte. Reads raw bytes
/// (no UTF-8 validation, no `String` allocation) because these files can be
/// large and are polled frequently by the dashboard. Returns `None` when the
/// file is absent so callers can fall back across candidate paths.
pub fn count_nonempty_lines_opt(path: &Path) -> Option<u64> {
    let bytes = std::fs::read(path).ok()?;
    Some(count_nonempty_lines_bytes(&bytes))
}

fn count_nonempty_lines_bytes(bytes: &[u8]) -> u64 {
    let mut count = 0u64;
    let mut has_non_ws = false;
    for &byte in bytes {
        match byte {
            b'\n' => {
                if has_non_ws {
                    count += 1;
                }
                has_non_ws = false;
            }
            b' ' | b'\t' | b'\r' => {}
            _ => has_non_ws = true,
        }
    }
    if has_non_ws {
        count += 1;
    }
    count
}

fn count_dir_entries(path: &Path) -> u64 {
    std::fs::read_dir(path)
        .map(|entries| entries.flatten().count() as u64)
        .unwrap_or(0)
}

fn first_file_mtime_ms(dir: &Path, candidates: &[&str]) -> Option<i64> {
    candidates
        .iter()
        .filter_map(|candidate| file_mtime_ms(&dir.join(candidate)))
        .max()
}

/// Walk for run folders that have started but not finalized. A run "started" if
/// it carries any native marker — `run-params.json` (membench-launched),
/// live `raw/`/`traces/` files, or a `vaults`/`workflow`/`provider-queue` state
/// dir. Prunes the heavy state directories so large `vaults/` trees don't slow
/// the scan.
fn collect_pending_dirs(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut has_marker = false;
    let mut has_report = false;
    let mut subdirs = Vec::new();
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_symlink() {
            continue;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if file_type.is_dir() {
            if matches!(name.as_ref(), "vaults" | "workflow" | "provider-queue") {
                has_marker = true;
            }
            if !PRUNE_DIRS.contains(&name.as_ref()) {
                subdirs.push(entry.path());
            }
        } else {
            match name.as_ref() {
                "benchmark-report.json" => has_report = true,
                "run-params.json" => has_marker = true,
                _ => {}
            }
        }
    }
    if has_marker {
        // A run dir: record it when in-flight; never descend further (its
        // subdirs are state, not nested runs).
        if !has_report {
            out.push(dir.to_path_buf());
        }
        return;
    }
    for subdir in subdirs {
        collect_pending_dirs(&subdir, out);
    }
}

/// Scan registry roots for in-flight / stalled runs.
pub fn scan_pending(roots: &[PathBuf], repo_root: &Path) -> Vec<PendingRun> {
    let mut dirs = Vec::new();
    for root in roots {
        collect_pending_dirs(root, &mut dirs);
    }
    dirs.sort();
    dirs.dedup();

    let now = now_ms();
    let mut pending: Vec<PendingRun> = dirs
        .into_iter()
        .filter_map(|dir| {
            let run_id = dir
                .strip_prefix(repo_root)
                .unwrap_or(&dir)
                .to_string_lossy()
                .replace('\\', "/");
            let segments: Vec<&str> = run_id.split('/').collect();
            // Only benchmark runs: {origin}/{system}/{benchmark}/{limit}/{run_name}
            // with a numeric limit. This excludes other scratch under `runs/`
            // (embed-stress, rejudges, ad-hoc stress dirs) that also leave a
            // provider-queue/vaults trail.
            if segments.len() != 5 || segments[3].parse::<u64>().is_err() {
                return None;
            }
            let params = std::fs::read_to_string(dir.join("run-params.json"))
                .ok()
                .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
                .unwrap_or(Value::Null);
            let seg = |idx: usize| segments.get(idx).map(|s| s.to_string());

            let updated_ms = first_file_mtime_ms(
                &dir,
                &[
                    "raw/hypotheses.jsonl",
                    "raw/provenance.jsonl",
                    "raw/model-traces.jsonl",
                    "raw/memory-traces.jsonl",
                    "traces/memory-events.jsonl",
                    "provider-queue/model-queue-traces.jsonl",
                    "workflow/longmemeval/queue.sqlite",
                    "run-params.json",
                ],
            );

            let status = match updated_ms {
                Some(ms) if now - ms < RUNNING_WINDOW_MS => "running",
                Some(ms) if now - ms < STALLED_WINDOW_MS => "warning",
                _ => "stalled",
            };
            let models = Models::default();

            Some(PendingRun {
                origin: segments.first().map(|s| s.to_string()).unwrap_or_default(),
                system: nested_string(&params, &["system"])
                    .or_else(|| seg(1))
                    .unwrap_or_default(),
                benchmark: nested_string(&params, &["benchmark"])
                    .or_else(|| seg(2))
                    .unwrap_or_default(),
                limit: nested_u64(&params, &["limit"])
                    .or_else(|| seg(3).and_then(|s| s.parse().ok())),
                run_name: nested_string(&params, &["run_name"])
                    .or_else(|| seg(4))
                    .unwrap_or_default(),
                config_label: config_label(&seg(4).unwrap_or_default(), &params, &models),
                settings_label: settings_label(&params),
                status: status.to_string(),
                started_ms: file_mtime_ms(&dir.join("run-params.json"))
                    .or_else(|| file_mtime_ms(&dir)),
                updated_ms,
                age_secs: updated_ms.map(|ms| (now - ms) / 1000),
                hypotheses: count_nonempty_lines(&dir.join("raw/hypotheses.jsonl")),
                ingested: count_dir_entries(&dir.join("vaults")),
                oracle_gold: nested_bool(&params, &["oracle_gold"]).unwrap_or(false),
                run_id,
            })
        })
        .collect();

    // Running first, then most-recently-updated.
    // running, then warning, then stalled; most-recently-updated within each.
    let rank = |status: &str| match status {
        "running" => 0,
        "warning" => 1,
        _ => 2,
    };
    pending.sort_by(|a, b| {
        rank(&a.status)
            .cmp(&rank(&b.status))
            .then(b.updated_ms.cmp(&a.updated_ms))
    });
    pending
}

fn settings_label(params: &Value) -> String {
    let mut parts = Vec::new();
    if let Some(workflow) = nested_u64(params, &["workflow", "max_in_flight"])
        .or_else(|| nested_u64(params, &["workflow_max_in_flight"]))
    {
        parts.push(format!("wf{workflow}"));
    }
    if let Some(label) = nested_string(params, &["transport", "embed", "label"])
        .or_else(|| tuning_shape_from_params(params).map(|shape| format!("embed {shape}")))
    {
        let label = label
            .strip_prefix("embed ")
            .map(|shape| format!("embed {shape}"))
            .unwrap_or_else(|| format!("embed {label}"));
        parts.push(label);
    }
    if let Some(label) = nested_string(params, &["transport", "chat", "label"])
        .or_else(|| chat_tuning_shape_from_params(params).map(|shape| format!("chat {shape}")))
    {
        let label = label
            .strip_prefix("chat ")
            .map(|shape| format!("chat {shape}"))
            .unwrap_or_else(|| format!("chat {label}"));
        parts.push(label);
    }
    if let Some(thinking) = nested_string(params, &["thinking", "summary"])
        .or_else(|| thinking_summary_from_roles(params))
    {
        if !thinking.is_empty() {
            parts.push(thinking);
        }
    }
    parts.join(" · ")
}

fn thinking_summary_from_roles(params: &Value) -> Option<String> {
    let roles = ["distill", "query_planner", "answer", "judge"];
    let values: Vec<String> = roles
        .iter()
        .filter_map(|role| nested_string(params, &["role_settings", role, "thinking"]))
        .collect();
    if values.is_empty() {
        None
    } else if values.len() == roles.len() && values.iter().all(|value| value == "disabled") {
        Some("nonthinking".to_string())
    } else {
        Some(
            roles
                .iter()
                .filter_map(|role| {
                    nested_string(params, &["role_settings", role, "thinking"])
                        .map(|value| format!("{}:{value}", role.replace('_', "-")))
                })
                .collect::<Vec<_>>()
                .join(","),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn write_run(root: &Path, report: &Value, params: &Value) {
        std::fs::create_dir_all(root).unwrap();
        std::fs::write(
            root.join("benchmark-report.json"),
            serde_json::to_string_pretty(report).unwrap(),
        )
        .unwrap();
        std::fs::write(
            root.join("run-params.json"),
            serde_json::to_string_pretty(params).unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn scans_and_summarizes_runs() {
        let repo = tempfile::tempdir().unwrap();
        let run_root = repo
            .path()
            .join("runs/symbiotic-memory/long-mem-eval/500/baseline");
        let report = json!({
            "system": "symbiotic-memory",
            "benchmark": "long-mem-eval",
            "run_kind": "imported-artifact",
            "run_name": "baseline",
            "metrics": {"accuracy": {"correct": 453, "total": 500, "value": 0.906}},
            "artifact_manifest": {"available": ["scored"], "missing": ["model_traces"], "native_state_available": false}
        });
        write_run(&run_root, &report, &json!({"limit": 500}));

        let records = scan_registry(&[repo.path().join("runs")], repo.path());
        assert_eq!(records.len(), 1);
        let summary = summarize(&records[0]);
        assert_eq!(
            summary.run_id,
            "runs/symbiotic-memory/long-mem-eval/500/baseline"
        );
        assert_eq!(summary.origin, "runs");
        assert_eq!(summary.accuracy, Some(0.906));
        assert_eq!(summary.limit, Some(500));
        assert!(
            summary
                .artifacts_missing
                .contains(&"model_traces".to_string())
        );
        assert_eq!(summary.display_name, "baseline");
        assert_eq!(summary.registry_section, "benchmarks");
    }

    #[test]
    fn openrouter_qwen_raw_embed_runs_are_named_as_tuning_arms() {
        let repo = tempfile::tempdir().unwrap();
        let run_name = "target-10q-qwen1024-http1-pool32x32-rawembed-20260623-102020";
        let run_root = repo
            .path()
            .join(format!("runs/symbiotic-memory/long-mem-eval/10/{run_name}"));
        let report = json!({
            "system": "symbiotic-memory",
            "benchmark": "long-mem-eval",
            "run_kind": "native",
            "run_name": run_name,
            "metrics": {"accuracy": {"correct": 0, "total": 10, "value": 0.0}},
            "artifact_manifest": {"available": ["model_traces"], "missing": ["scored"], "native_state_available": true},
            "meta_record": {"schema": "membench.meta_record.v1"}
        });
        let params = json!({
            "limit": 10,
            "stop_after_raw_embed": true,
            "embedder": "openrouter",
            "configured_models": {
                "embed": {"model": "qwen/qwen3-embedding-8b"}
            },
            "openrouter_http1_only": true,
            "openrouter_http_client_pool_size": "32",
            "openrouter_http_pool_max_idle_per_host": "32"
        });
        write_run(&run_root, &report, &params);

        let records = scan_registry(&[repo.path().join("runs")], repo.path());
        let summary = summarize(&records[0]);
        assert_eq!(summary.registry_section, "tuning");
        assert!(summary.is_meta_record);
        assert_eq!(
            summary.tuning_cohort.as_deref(),
            Some("embed-transport/openrouter-qwen3-8b-1024-32k")
        );
        assert_eq!(summary.tuning_shape.as_deref(), Some("h1 32x32"));
        assert_eq!(summary.display_name, "h1 32x32 · qwen3-emb-8b 1024d");
    }

    #[test]
    fn deepseek_chat_distill_runs_are_named_as_tuning_arms() {
        let repo = tempfile::tempdir().unwrap();
        let run_name = "ds-chat-h2-32x32-10q-20260623-133649";
        let run_root = repo
            .path()
            .join(format!("runs/symbiotic-memory/long-mem-eval/10/{run_name}"));
        let report = json!({
            "system": "symbiotic-memory",
            "benchmark": "long-mem-eval",
            "run_kind": "native",
            "run_name": run_name,
            "metrics": {"accuracy": {"correct": 0, "total": 10, "value": 0.0}},
            "artifact_manifest": {"available": ["model_traces"], "missing": ["scored"], "native_state_available": true},
            "meta_record": {"schema": "membench.meta_record.v1"}
        });
        let params = json!({
            "limit": 10,
            "ingest_diagnostic": "distill",
            "configured_models": {
                "distill": {
                    "model": "deepseek-v4-flash",
                    "operator": "deepseek"
                }
            },
            "chat_http1_only": false,
            "chat_http_client_pool_size": "32",
            "chat_http_pool_max_idle_per_host": "32"
        });
        write_run(&run_root, &report, &params);

        let records = scan_registry(&[repo.path().join("runs")], repo.path());
        let summary = summarize(&records[0]);
        assert_eq!(summary.registry_section, "tuning");
        assert!(summary.is_meta_record);
        assert_eq!(
            summary.tuning_cohort.as_deref(),
            Some("chat-transport/deepseek-v4-flash-distill")
        );
        assert_eq!(summary.tuning_shape.as_deref(), Some("h2 32x32"));
        assert_eq!(summary.display_name, "h2 32x32 · deepseek-v4-flash");
    }

    #[test]
    fn pending_runs_read_live_native_shape() {
        let repo = tempfile::tempdir().unwrap();
        let run_root = repo
            .path()
            .join("runs/symbiotic-memory/long-mem-eval/10/current");
        std::fs::create_dir_all(run_root.join("raw")).unwrap();
        std::fs::create_dir_all(run_root.join("traces")).unwrap();
        std::fs::create_dir_all(run_root.join("vaults/q1")).unwrap();
        std::fs::create_dir_all(run_root.join("vaults/q2")).unwrap();
        std::fs::write(
            run_root.join("run-params.json"),
            serde_json::to_string_pretty(&json!({
                "system": "symbiotic-memory",
                "benchmark": "long-mem-eval",
                "run_name": "current",
                "limit": 10,
                "distiller": "heuristic",
                "embedder": "hash",
                "store": "sqlite",
                "query_planner": "flash",
                "routed": true
            }))
            .unwrap(),
        )
        .unwrap();
        std::fs::write(
            run_root.join("raw/hypotheses.jsonl"),
            "{\"question_id\":\"q1\"}\n{\"question_id\":\"q2\"}\n",
        )
        .unwrap();
        std::fs::write(
            run_root.join("traces/memory-events.jsonl"),
            "{\"operation\":\"answer\",\"event\":\"operation_succeeded\"}\n",
        )
        .unwrap();

        let pending = scan_pending(&[repo.path().join("runs")], repo.path());
        assert_eq!(pending.len(), 1);
        let run = &pending[0];
        assert_eq!(run.run_id, "runs/symbiotic-memory/long-mem-eval/10/current");
        assert_eq!(run.hypotheses, 2);
        assert_eq!(run.ingested, 2);
        assert_eq!(run.status, "running");
        assert!(run.config_label.contains("heuristic"));
        assert!(run.config_label.contains("plan:flash"));
    }

    #[test]
    fn trial_ledgers_mark_referenced_runs() {
        let repo = tempfile::tempdir().unwrap();
        let run_root = repo
            .path()
            .join("runs/symbiotic-memory/long-mem-eval/10/candidate");
        let report = json!({
            "system": "symbiotic-memory",
            "benchmark": "long-mem-eval",
            "run_kind": "native",
            "run_name": "candidate",
            "metrics": {"accuracy": {"correct": 8, "total": 10, "value": 0.8}},
            "artifact_manifest": {"available": ["scored"], "missing": [], "native_state_available": true}
        });
        write_run(&run_root, &report, &json!({"limit": 10}));

        let analysis = repo.path().join("runs/analysis/prompt-trials");
        std::fs::create_dir_all(&analysis).unwrap();
        std::fs::write(
            analysis.join("trials.jsonl"),
            serde_json::to_string(&json!({
                "schema": "membench.trial.v1",
                "stack_id": "prompt-trials",
                "run_id": "candidate",
                "run_path": "runs/symbiotic-memory/long-mem-eval/10/candidate",
                "change_id": "answer-prompt-v1",
                "change_title": "Answer prompt evidence discipline",
                "compared_to_run_id": "baseline",
                "original_baseline_run_id": "baseline",
                "decision": "diagnostic_only",
                "aggregate": {"accuracy": {"correct": 8, "total": 10, "value": 0.8}},
                "outcomes": {
                    "improvements": [{"question_id": "q1"}],
                    "regressions": [{"question_id": "q2"}],
                    "unchanged_wrong": ["q3"],
                    "unchanged_correct": ["q4", "q5"]
                }
            }))
            .unwrap()
                + "\n",
        )
        .unwrap();

        let records = scan_registry(&[repo.path().join("runs")], repo.path());
        let trial_index = scan_trial_markers(repo.path());
        let summary = summarize_with_trials(&records[0], &trial_index);
        assert!(summary.is_trial_run);
        assert_eq!(summary.trial_markers.len(), 1);
        let marker = &summary.trial_markers[0];
        assert_eq!(marker.stack_id, "prompt-trials");
        assert_eq!(marker.improvements, 1);
        assert_eq!(marker.regressions, 1);
        assert_eq!(marker.question_count, 10);
        assert_eq!(marker.sample_classification, "focused_trial");
        assert!(marker.focused);
        assert_eq!(marker.aggregate_accuracy, Some(0.8));
        assert_eq!(marker.analysis_path, "runs/analysis/prompt-trials");
    }

    #[test]
    fn cohort_models_fall_back_to_run_params_when_traces_have_no_roles() {
        let repo = tempfile::tempdir().unwrap();
        let run_root = repo.path().join("run");
        std::fs::create_dir_all(run_root.join("artifacts")).unwrap();
        std::fs::write(
            run_root.join("artifacts/model-traces.jsonl"),
            "{\"queue_id\":\"chat:deepseek:deepseek-v4-flash\",\"operation\":\"chat\",\"status\":\"succeeded\"}\n",
        )
        .unwrap();
        let params = json!({
            "configured_models": {
                "answer": {"model": "deepseek-v4-flash"},
                "distill": {"model": "deepseek-v4-flash"},
                "embed": {"model": "gemini-embedding-2"}
            },
            "runtime_models": {
                "judge": "queued:deepseek:deepseek-v4-flash"
            }
        });

        let fields = compute_cohort_fields(&run_root, &params);
        assert_eq!(fields.models.answer.as_deref(), Some("deepseek-v4-flash"));
        assert_eq!(fields.models.distill.as_deref(), Some("deepseek-v4-flash"));
        assert_eq!(fields.models.embed.as_deref(), Some("gemini-embedding-2"));
        assert_eq!(fields.models.judge.as_deref(), Some("deepseek-v4-flash"));
    }

    #[test]
    fn rerank_model_falls_back_to_run_params_configured_models() {
        // The rerank role is not surfaced by the model-trace rollup, so the
        // registry must read it from the recorded run params (configured_models
        // .rerank.model, mirrored at top-level rerank). Mirrors the shape
        // membench writes for an enabled cohere reranker.
        let params = json!({
            "configured_models": {
                "rerank": {
                    "enabled": true,
                    "model": "cohere/rerank-4-fast",
                    "operator": "openrouter",
                },
            },
            "rerank": {
                "enabled": true,
                "model": "cohere/rerank-4-fast",
                "operator": "openrouter",
            },
        });
        let models = models_with_param_fallback(Models::default(), &params, None);
        assert_eq!(models.rerank.as_deref(), Some("cohere/rerank-4-fast"));
    }

    #[test]
    fn rerank_model_none_when_disabled_or_absent() {
        // Disabled binding ({ enabled: false }) must not report a model, even if
        // a stale model string lingers in the object.
        let disabled = json!({
            "configured_models": { "rerank": { "enabled": false, "model": null } },
            "rerank": { "enabled": false },
        });
        assert!(models_with_param_fallback(Models::default(), &disabled, None).rerank.is_none());

        // Older runs recorded no rerank field at all → still None.
        let absent = json!({ "configured_models": { "answer": {"model": "deepseek-v4-flash"} } });
        assert!(models_with_param_fallback(Models::default(), &absent, None).rerank.is_none());
    }
}
