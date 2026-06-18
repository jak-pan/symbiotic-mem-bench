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
    pub run_kind: String,
    pub config_label: String,
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
    pub created_at: Option<String>,
    pub modified_ms: Option<i64>,
    pub per_question_type: Option<Value>,
    pub artifacts_available: Vec<String>,
    pub artifacts_missing: Vec<String>,
    pub native_state_available: Option<bool>,
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

/// Reduce a record to its index row. This is the cheap path: it reads `scored`
/// and `verdicts` (≈ `limit` lines) for cohort identity but never the large
/// trace files — cost/latency come from the report when persisted.
pub fn summarize(record: &RunRecord) -> RunSummary {
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
    let judge_model = nested_string(report, &["cohort", "judge_model"])
        .or_else(|| artifacts::judge_model(&record.run_root));
    let judge_prompt_mode = nested_string(report, &["cohort", "judge_prompt_mode"])
        .or_else(|| nested_string(report, &["judge_prompt_mode"]))
        .or_else(|| artifacts::judge_prompt_mode(&record.run_root));
    let models = report
        .get("models")
        .and_then(|value| serde_json::from_value::<Models>(value.clone()).ok())
        .unwrap_or_default();
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

    let per_question_type = artifacts::read_scored(&record.run_root)
        .and_then(|scored| scored.get("per_question_type").cloned());

    RunSummary {
        run_id: record.run_id.clone(),
        origin: record.origin.clone(),
        config_label: config_label(&run_name, params, &models),
        system,
        benchmark,
        limit,
        run_name,
        run_kind,
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
        created_at: nested_string(report, &["created_at"]),
        modified_ms: record.modified_ms,
        per_question_type,
        artifacts_available: string_list("available"),
        artifacts_missing: string_list("missing"),
        native_state_available: nested_bool(
            report,
            &["artifact_manifest", "native_state_available"],
        ),
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

/// Derive cohort + cost fields for a run, reading trace files. Used at report
/// finalize (persisted) and by the server's run-detail endpoint.
pub fn compute_cohort_fields(run_root: &Path, params: &Value) -> CohortFields {
    let dataset_fingerprint = cohort::dataset_fingerprint(run_root);
    let judge_model = artifacts::judge_model(run_root);
    let judge_prompt_mode = artifacts::judge_prompt_mode(run_root);
    let rollup = cost::rollup_model_traces(run_root);
    let mut models = rollup
        .as_ref()
        .map(|rollup| Models::from_roles(&rollup.roles))
        .unwrap_or_default();
    if models.judge.is_none() {
        models.judge = judge_model.clone();
    }
    let config_signature = cohort::config_signature(params, &models);
    CohortFields {
        dataset_fingerprint,
        judge_model,
        judge_prompt_mode,
        models,
        role_stats: rollup
            .as_ref()
            .map(|rollup| rollup.roles_detail.clone())
            .unwrap_or_default(),
        config_signature,
        cost_micro_usd: rollup.as_ref().and_then(|rollup| rollup.cost_micro_usd),
        latency_ms_p50: rollup.as_ref().and_then(|rollup| rollup.latency_ms_p50),
        latency_ms_p95: rollup.as_ref().and_then(|rollup| rollup.latency_ms_p95),
        cached_input_tokens: rollup.as_ref().map(|rollup| rollup.cached_input_tokens),
        uncached_input_tokens: rollup.as_ref().map(|rollup| rollup.uncached_input_tokens),
        response_cache_hits: rollup.as_ref().map(|rollup| rollup.response_cache_hits),
        prompt_cache_hits: rollup.as_ref().map(|rollup| rollup.prompt_cache_hits),
        prompt_cache_partial_hits: rollup
            .as_ref()
            .map(|rollup| rollup.prompt_cache_partial_hits),
        prompt_cache_misses: rollup.as_ref().map(|rollup| rollup.prompt_cache_misses),
    }
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
    /// `running` (recent file activity) or `stalled`.
    pub status: String,
    pub started_ms: Option<i64>,
    pub updated_ms: Option<i64>,
    pub age_secs: Option<i64>,
    /// Hypotheses produced so far (answer progress).
    pub hypotheses: u64,
    /// Question vaults built so far (ingest progress).
    pub ingested: u64,
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
    std::fs::read_to_string(path)
        .map(|raw| raw.lines().filter(|line| !line.trim().is_empty()).count() as u64)
        .unwrap_or(0)
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
                status: status.to_string(),
                started_ms: file_mtime_ms(&dir.join("run-params.json"))
                    .or_else(|| file_mtime_ms(&dir)),
                updated_ms,
                age_secs: updated_ms.map(|ms| (now - ms) / 1000),
                hypotheses: count_nonempty_lines(&dir.join("raw/hypotheses.jsonl")),
                ingested: count_dir_entries(&dir.join("vaults")),
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
                "query_planner": "scripted",
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
        assert!(run.config_label.contains("plan:scripted"));
    }
}
