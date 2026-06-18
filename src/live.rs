//! Live stats for an in-flight run, read cheaply from the run root.
//!
//! While a native run executes it appends executor-owned outputs directly under
//! `raw/`, `traces/`, `workflow/`, and `provider-queue/`. To surface progress
//! without reading whole files on every poll, this module reads a bounded
//! **tail** of each trace file and summarizes recent activity: queue pressure,
//! model throughput, memory stage mix, and the most recent errors.

use serde::Serialize;
use serde_json::Value;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// How many trailing bytes of each trace file to inspect per poll.
const TAIL_BYTES: u64 = 1_200_000;
/// Cap on recent errors returned.
const MAX_ERRORS: usize = 30;

/// Recent provider-queue pressure (over the tail window).
#[derive(Clone, Debug, Default, Serialize)]
pub struct QueuePressure {
    pub queued: u64,
    pub running: u64,
    pub succeeded: u64,
    pub failed: u64,
    pub dead: u64,
    /// Items whose most recent event in the window is still queued/running.
    pub in_flight: u64,
    /// Events inspected in the window.
    pub window: u64,
}

/// Recent model-call throughput (over the tail window).
#[derive(Clone, Debug, Default, Serialize)]
pub struct ModelLive {
    pub window_calls: u64,
    pub window_failed: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    /// Total size of `model-traces.jsonl` so far, a coarse scale indicator.
    pub total_bytes: u64,
}

/// Per-operation progress through the memory pipeline (cumulative over the run).
#[derive(Clone, Debug, Default, Serialize)]
pub struct StageProgress {
    pub operation: String,
    pub started: u64,
    pub succeeded: u64,
    pub failed: u64,
    /// `started - succeeded - failed`: requests currently in this stage.
    pub in_flight: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct LiveError {
    pub timestamp: Option<String>,
    pub source: String,
    pub kind: Option<String>,
    pub message: String,
}

/// The dynamic portion of a run's live view.
#[derive(Clone, Debug, Default, Serialize)]
pub struct LiveDetail {
    pub queue: QueuePressure,
    pub model: ModelLive,
    /// Pipeline-ordered per-stage progress (capture → distill → … → answer).
    pub memory_stages: Vec<StageProgress>,
    pub memory_failures: u64,
    pub errors: Vec<LiveError>,
}

/// Canonical pipeline order for memory operations, so stages render in the order
/// a request flows through them.
const STAGE_ORDER: &[&str] = &[
    "capture",
    "ingest",
    "distill",
    "write_archive",
    "embed_raw",
    "embed_facts",
    "index",
    "retrieve",
    "answer",
    "score",
    "flush",
    "state_export",
    "adapter_call",
    "model_call",
    "embedding_call",
    "vector_search",
];

/// Memory traces are bounded by question-count × stages (not token volume), so
/// they stay small enough to read whole for accurate progress; only fall back to
/// a tail beyond this size.
const MEMORY_FULL_CAP: u64 = 40_000_000;

/// Read up to `max_bytes` from the end of a file and return its complete JSON
/// lines (the first, possibly-partial, line of the window is dropped).
fn tail_values(path: &Path, max_bytes: u64) -> Vec<Value> {
    let Ok(mut file) = std::fs::File::open(path) else {
        return Vec::new();
    };
    let len = file.metadata().map(|meta| meta.len()).unwrap_or(0);
    let start = len.saturating_sub(max_bytes);
    if file.seek(SeekFrom::Start(start)).is_err() {
        return Vec::new();
    }
    let mut buf = Vec::new();
    if file.read_to_end(&mut buf).is_err() {
        return Vec::new();
    }
    let text = String::from_utf8_lossy(&buf);
    let mut lines: Vec<&str> = text.lines().collect();
    if start > 0 && !lines.is_empty() {
        lines.remove(0); // drop the partial leading line
    }
    lines
        .into_iter()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .collect()
}

/// Read the whole file when it's under `cap`, else fall back to a tail. Used for
/// cumulative progress that needs every line when feasible.
fn read_values_capped(path: &Path, cap: u64) -> Vec<Value> {
    let size = std::fs::metadata(path).map(|meta| meta.len()).unwrap_or(0);
    if size > cap {
        return tail_values(path, cap);
    }
    let Ok(raw) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    raw.lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .collect()
}

fn str_at<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

fn first_existing(run_root: &Path, candidates: &[&str]) -> Option<std::path::PathBuf> {
    candidates
        .iter()
        .map(|candidate| run_root.join(candidate))
        .find(|path| path.is_file())
}

/// Compute the live detail for a run root.
pub fn live_detail(run_root: &Path) -> LiveDetail {
    let mut detail = LiveDetail::default();

    // --- queue pressure ---
    let queue_path = run_root
        .join("provider-queue")
        .join("model-queue-traces.jsonl");
    let queue_events = tail_values(&queue_path, TAIL_BYTES);
    detail.queue.window = queue_events.len() as u64;
    let mut last_status: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    for event in &queue_events {
        let status = str_at(event, "status").unwrap_or("");
        match status {
            "queued" | "pending" => detail.queue.queued += 1,
            "running" => detail.queue.running += 1,
            "succeeded" => detail.queue.succeeded += 1,
            "failed" => detail.queue.failed += 1,
            "dead" => detail.queue.dead += 1,
            _ => {}
        }
        if let Some(item) = str_at(event, "item_id") {
            last_status.insert(item.to_string(), status.to_string());
        }
    }
    detail.queue.in_flight = last_status
        .values()
        .filter(|status| matches!(status.as_str(), "queued" | "pending" | "running"))
        .count() as u64;

    // --- model throughput + errors ---
    let model_path = first_existing(
        run_root,
        &[
            "raw/model-traces.jsonl",
            "model-traces.jsonl",
            "artifacts/model-traces.jsonl",
        ],
    )
    .unwrap_or_else(|| run_root.join("raw/model-traces.jsonl"));
    detail.model.total_bytes = std::fs::metadata(&model_path).map(|m| m.len()).unwrap_or(0);
    for trace in tail_values(&model_path, TAIL_BYTES) {
        detail.model.window_calls += 1;
        let failed = str_at(&trace, "outcome").is_some_and(|o| o != "succeeded");
        if failed {
            detail.model.window_failed += 1;
            push_error(&mut detail.errors, &trace, "model");
        }
        if let Some(usage) = trace.get("usage") {
            detail.model.input_tokens += usage
                .get("input_tokens")
                .and_then(Value::as_u64)
                .unwrap_or(0);
            detail.model.output_tokens += usage
                .get("output_tokens")
                .and_then(Value::as_u64)
                .unwrap_or(0);
        }
    }

    // --- per-stage memory progress (cumulative over the whole run) ---
    let mut stages: std::collections::BTreeMap<String, StageProgress> =
        std::collections::BTreeMap::new();
    let memory_path = first_existing(
        run_root,
        &[
            "raw/memory-traces.jsonl",
            "traces/memory-events.jsonl",
            "memory-traces.jsonl",
            "artifacts/memory-traces.jsonl",
        ],
    )
    .unwrap_or_else(|| run_root.join("raw/memory-traces.jsonl"));
    for trace in read_values_capped(&memory_path, MEMORY_FULL_CAP) {
        let Some(op) = str_at(&trace, "operation") else {
            continue;
        };
        let entry = stages
            .entry(op.to_string())
            .or_insert_with(|| StageProgress {
                operation: op.to_string(),
                ..Default::default()
            });
        let event = str_at(&trace, "event").unwrap_or("");
        match event {
            "operation_started" => entry.started += 1,
            "operation_succeeded" => entry.succeeded += 1,
            "operation_failed" => {
                entry.failed += 1;
                detail.memory_failures += 1;
                push_error(&mut detail.errors, &trace, "memory");
            }
            _ => {
                if trace.get("error").is_some_and(|e| !e.is_null()) {
                    push_error(&mut detail.errors, &trace, "memory");
                }
            }
        }
    }
    for stage in stages.values_mut() {
        stage.in_flight = stage
            .started
            .saturating_sub(stage.succeeded)
            .saturating_sub(stage.failed);
    }
    // Order along the pipeline; unknown ops keep alphabetical order at the end.
    let mut ordered: Vec<StageProgress> = stages.into_values().collect();
    ordered.sort_by_key(|stage| {
        STAGE_ORDER
            .iter()
            .position(|known| *known == stage.operation)
            .unwrap_or(STAGE_ORDER.len())
    });
    detail.memory_stages = ordered;

    // newest errors first (by timestamp), capped
    detail.errors.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    detail.errors.truncate(MAX_ERRORS);
    detail
}

fn push_error(errors: &mut Vec<LiveError>, trace: &Value, source: &str) {
    let message = str_at(trace, "error")
        .or_else(|| str_at(trace, "error_class"))
        .or_else(|| str_at(trace, "outcome"))
        .unwrap_or("error")
        .to_string();
    errors.push(LiveError {
        timestamp: str_at(trace, "timestamp").map(ToOwned::to_owned),
        source: source.to_string(),
        kind: str_at(trace, "error_class").map(ToOwned::to_owned),
        message,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_detail_summarizes_recent_activity() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("provider-queue")).unwrap();
        std::fs::create_dir_all(root.join("raw")).unwrap();
        std::fs::create_dir_all(root.join("traces")).unwrap();
        std::fs::write(
            root.join("provider-queue").join("model-queue-traces.jsonl"),
            "{\"item_id\":\"a\",\"status\":\"running\"}\n{\"item_id\":\"a\",\"status\":\"succeeded\"}\n{\"item_id\":\"b\",\"status\":\"running\"}\n",
        )
        .unwrap();
        std::fs::write(
            root.join("raw/model-traces.jsonl"),
            "{\"outcome\":\"succeeded\",\"usage\":{\"input_tokens\":10,\"output_tokens\":2}}\n{\"outcome\":\"failed\",\"error_class\":\"timeout\",\"error\":\"deadline\"}\n",
        )
        .unwrap();
        std::fs::write(
            root.join("traces/memory-events.jsonl"),
            "{\"operation\":\"capture\",\"event\":\"operation_succeeded\"}\n\
             {\"operation\":\"distill\",\"event\":\"operation_started\"}\n\
             {\"operation\":\"distill\",\"event\":\"operation_started\"}\n\
             {\"operation\":\"distill\",\"event\":\"operation_succeeded\"}\n",
        )
        .unwrap();

        let detail = live_detail(root);
        assert_eq!(detail.queue.succeeded, 1);
        assert_eq!(detail.queue.in_flight, 1); // item b still running
        assert_eq!(detail.model.window_calls, 2);
        assert_eq!(detail.model.window_failed, 1);
        assert_eq!(detail.model.input_tokens, 10);
        // capture before distill (pipeline order); distill: 2 started, 1 done → 1 in flight
        assert_eq!(detail.memory_stages[0].operation, "capture");
        let distill = detail
            .memory_stages
            .iter()
            .find(|s| s.operation == "distill")
            .unwrap();
        assert_eq!(distill.started, 2);
        assert_eq!(distill.succeeded, 1);
        assert_eq!(distill.in_flight, 1);
        assert_eq!(detail.errors.len(), 1);
        assert_eq!(detail.errors[0].message, "deadline");
    }
}
