//! Live stats for an in-flight run, read cheaply from the run root.
//!
//! While a native run executes it appends executor-owned outputs directly under
//! `raw/`, `traces/`, `workflow/`, and `provider-queue/`. To surface progress
//! without reading whole files on every poll, this module reads a bounded
//! **tail** of each trace file and summarizes recent activity: queue pressure,
//! model throughput, memory stage mix, and the most recent errors.

use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// How many trailing bytes of each trace file to inspect per poll.
const TAIL_BYTES: u64 = 1_200_000;
/// Provider queue traces are compact enough for completed small/medium runs.
/// Above this size live polling falls back to a tail to avoid dragging giant
/// experimental traces through the dashboard loop.
const PROVIDER_FULL_CAP: u64 = 80_000_000;
/// Cap on recent errors returned.
const MAX_ERRORS: usize = 30;
/// Cap on recent activity rows returned.
const MAX_ACTIVITY: usize = 80;

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

/// Current state for one provider/model queue in the inspected window.
#[derive(Clone, Debug, Default, Serialize)]
pub struct QueueBreakdown {
    pub queue_id: String,
    pub operation: String,
    pub queued: u64,
    pub running: u64,
    pub succeeded: u64,
    pub failed: u64,
    pub dead: u64,
    pub in_flight: u64,
    pub window: u64,
    /// Running requests observed at the busiest point in the inspected window.
    pub observed_peak_running: u64,
    /// Running events in the last trace minute of the inspected window.
    pub starts_last_minute: u64,
    /// Highest 60-second running-event count observed in the inspected window.
    pub peak_starts_per_minute: u64,
    /// Time-weighted average running requests over the inspected provider event span.
    pub avg_running: f64,
    /// Time-weighted average queued requests over the inspected provider event span.
    pub avg_queued: f64,
    /// Average request starts per minute over the inspected provider event span.
    pub avg_starts_per_minute: f64,
    /// Seconds between the first and last event for this queue in the inspected span.
    pub observed_duration_secs: f64,
    /// Latest event timestamp for this queue in the inspected window.
    pub last_event_at: Option<String>,
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
    pub last_event: Option<String>,
    pub last_event_at: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct LiveError {
    pub timestamp: Option<String>,
    pub source: String,
    pub kind: Option<String>,
    pub message: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct LiveActivity {
    pub timestamp: Option<String>,
    pub source: String,
    pub operation: String,
    pub status: String,
    pub queue_id: Option<String>,
    pub message: String,
    pub severity: String,
}

/// The dynamic portion of a run's live view.
#[derive(Clone, Debug, Default, Serialize)]
pub struct LiveDetail {
    pub queue: QueuePressure,
    pub queues: Vec<QueueBreakdown>,
    pub model: ModelLive,
    /// Pipeline-ordered per-stage progress (capture → distill → … → answer).
    pub memory_stages: Vec<StageProgress>,
    pub memory_failures: u64,
    pub errors: Vec<LiveError>,
    /// Recent interleaved memory/provider events, newest first.
    pub activity: Vec<LiveActivity>,
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
    "consolidate",
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

fn timestamp_at(value: &Value) -> Option<DateTime<Utc>> {
    str_at(value, "timestamp").and_then(|raw| {
        DateTime::parse_from_rfc3339(raw)
            .ok()
            .map(|dt| dt.with_timezone(&Utc))
    })
}

fn timestamp_string(value: &Value) -> Option<String> {
    str_at(value, "timestamp").map(ToOwned::to_owned)
}

fn first_existing(run_root: &Path, candidates: &[&str]) -> Option<std::path::PathBuf> {
    candidates
        .iter()
        .map(|candidate| run_root.join(candidate))
        .find(|path| path.is_file())
}

fn status_at(value: &Value) -> &str {
    str_at(value, "status")
        .or_else(|| str_at(value, "outcome"))
        .unwrap_or("")
}

fn queue_group_key(event: &Value) -> String {
    str_at(event, "queue_id")
        .or_else(|| str_at(event, "model"))
        .unwrap_or("unknown")
        .to_string()
}

fn usage_u64(trace: &Value, keys: &[&str]) -> u64 {
    let usage = trace.get("usage").unwrap_or(trace);
    keys.iter()
        .find_map(|key| usage.get(*key).and_then(Value::as_u64))
        .unwrap_or(0)
}

fn add_usage(model: &mut ModelLive, trace: &Value) {
    model.input_tokens += usage_u64(trace, &["input_tokens", "prompt_tokens"]);
    model.output_tokens += usage_u64(trace, &["output_tokens", "completion_tokens"]);
}

fn is_terminal_status(status: &str) -> bool {
    matches!(status, "succeeded" | "failed" | "dead")
}

fn queue_message(event: &Value, status: &str) -> String {
    let attempt = event.get("attempt").and_then(Value::as_u64).unwrap_or(0);
    let mut message = if attempt > 0 {
        format!("{status} attempt {attempt}")
    } else {
        status.to_string()
    };
    if let Some(error) = str_at(event, "error") {
        message.push_str(": ");
        message.push_str(error);
    } else if let Some(error) = str_at(event, "error_class") {
        message.push_str(": ");
        message.push_str(error);
    }
    message
}

fn memory_message(trace: &Value, event: &str) -> String {
    let mut parts = vec![event.to_string()];
    if let Some(duration) = trace.get("duration_ms").and_then(Value::as_u64) {
        parts.push(format!("{duration}ms"));
    }
    if let Some(item_count) = trace.get("item_count").and_then(Value::as_u64) {
        parts.push(format!("{item_count} items"));
    }
    if let Some(metrics) = trace.get("metrics") {
        if let Some(turns) = metrics.get("turn_count").and_then(Value::as_u64) {
            parts.push(format!("{turns} turns"));
        }
        if let Some(facts) = metrics
            .get("fact_count")
            .or_else(|| metrics.get("base_fact_count"))
            .and_then(Value::as_u64)
        {
            parts.push(format!("{facts} facts"));
        }
    }
    if let Some(error) = str_at(trace, "error") {
        parts.push(error.to_string());
    }
    parts.join(" · ")
}

fn push_activity(
    activity: &mut Vec<LiveActivity>,
    timestamp: Option<String>,
    source: &str,
    operation: String,
    status: String,
    queue_id: Option<String>,
    message: String,
    severity: &str,
) {
    activity.push(LiveActivity {
        timestamp,
        source: source.to_string(),
        operation,
        status,
        queue_id,
        message,
        severity: severity.to_string(),
    });
}

#[derive(Clone, Debug, Default)]
struct QueueObservedStats {
    peak_running: u64,
    starts_last_minute: u64,
    peak_starts_per_minute: u64,
    avg_running: f64,
    avg_queued: f64,
    avg_starts_per_minute: f64,
    observed_duration_secs: f64,
    last_event_at: Option<String>,
}

fn observed_queue_stats(events: &[Value]) -> BTreeMap<String, QueueObservedStats> {
    let mut by_queue: BTreeMap<String, Vec<&Value>> = BTreeMap::new();
    for event in events {
        by_queue
            .entry(queue_group_key(event))
            .or_default()
            .push(event);
    }

    let mut stats = BTreeMap::new();
    for (queue_id, mut queue_events) in by_queue {
        queue_events.sort_by_key(|event| timestamp_at(event));
        let max_ts = queue_events
            .iter()
            .filter_map(|event| timestamp_at(event))
            .max();
        let Some(first_ts) = queue_events
            .iter()
            .filter_map(|event| timestamp_at(event))
            .min()
        else {
            stats.insert(queue_id, QueueObservedStats::default());
            continue;
        };
        let Some(max_ts) = max_ts else {
            stats.insert(queue_id, QueueObservedStats::default());
            continue;
        };
        let last_cutoff = Some(max_ts - Duration::seconds(60));
        let mut last_minute = 0_u64;
        let mut running_at: HashSet<String> = HashSet::new();
        let mut queued_at: HashSet<String> = HashSet::new();
        let mut peak_running = 0_u64;
        let mut starts = Vec::new();
        let mut last_event_at = None;
        let mut prev_ts = first_ts;
        let mut running_weighted_ms = 0_f64;
        let mut queued_weighted_ms = 0_f64;

        for event in queue_events {
            let ts = timestamp_at(event).unwrap_or(prev_ts);
            let elapsed_ms = (ts - prev_ts).num_milliseconds().max(0) as f64;
            running_weighted_ms += running_at.len() as f64 * elapsed_ms;
            queued_weighted_ms += queued_at.len() as f64 * elapsed_ms;
            prev_ts = ts;
            last_event_at = Some(ts.to_rfc3339_opts(chrono::SecondsFormat::Millis, true));
            let status = status_at(event);
            if status == "running" {
                starts.push(ts);
                if last_cutoff.is_some_and(|cutoff| ts >= cutoff) {
                    last_minute += 1;
                }
                if let Some(item) = str_at(event, "item_id") {
                    queued_at.remove(item);
                    running_at.insert(item.to_string());
                }
                peak_running = peak_running.max(running_at.len() as u64);
            } else if matches!(status, "queued" | "pending") {
                if let Some(item) = str_at(event, "item_id") {
                    queued_at.insert(item.to_string());
                }
            } else if is_terminal_status(status) {
                if let Some(item) = str_at(event, "item_id") {
                    running_at.remove(item);
                    queued_at.remove(item);
                }
            }
        }
        starts.sort();
        let mut left = 0_usize;
        let mut peak_rpm = 0_u64;
        for right in 0..starts.len() {
            while starts[right] - starts[left] > Duration::seconds(60) {
                left += 1;
            }
            peak_rpm = peak_rpm.max((right - left + 1) as u64);
        }
        let duration_ms = (max_ts - first_ts).num_milliseconds().max(1) as f64;
        stats.insert(
            queue_id,
            QueueObservedStats {
                peak_running,
                starts_last_minute: last_minute,
                peak_starts_per_minute: peak_rpm,
                avg_running: running_weighted_ms / duration_ms,
                avg_queued: queued_weighted_ms / duration_ms,
                avg_starts_per_minute: starts.len() as f64 / (duration_ms / 60_000_f64),
                observed_duration_secs: duration_ms / 1000_f64,
                last_event_at,
            },
        );
    }
    stats
}

/// Compute the live detail for a run root.
pub fn live_detail(run_root: &Path) -> LiveDetail {
    let mut detail = LiveDetail::default();

    // --- queue pressure ---
    let queue_path = run_root
        .join("provider-queue")
        .join("model-queue-traces.jsonl");
    let queue_events = read_values_capped(&queue_path, PROVIDER_FULL_CAP);
    detail.queue.window = queue_events.len() as u64;
    let mut last_status: HashMap<String, (String, String, String)> = HashMap::new();
    let mut windows_by_queue: BTreeMap<String, u64> = BTreeMap::new();
    for event in &queue_events {
        let status = status_at(event);
        let queue_id = queue_group_key(event);
        let operation = str_at(event, "operation")
            .or_else(|| str_at(event, "kind"))
            .unwrap_or("")
            .to_string();
        *windows_by_queue.entry(queue_id.clone()).or_default() += 1;
        if let Some(item) = str_at(event, "item_id") {
            last_status.insert(
                item.to_string(),
                (status.to_string(), queue_id.clone(), operation.clone()),
            );
        }
        if matches!(status, "failed" | "dead") {
            push_error(&mut detail.errors, event, "provider");
        }
        push_activity(
            &mut detail.activity,
            timestamp_string(event),
            "provider",
            operation,
            status.to_string(),
            Some(queue_id),
            queue_message(event, status),
            if matches!(status, "failed" | "dead") {
                "error"
            } else {
                "info"
            },
        );
    }
    let observed_stats = observed_queue_stats(&queue_events);
    let mut by_queue: BTreeMap<String, QueueBreakdown> = BTreeMap::new();
    for (_, (status, queue_id, operation)) in last_status {
        let entry = by_queue
            .entry(queue_id.clone())
            .or_insert_with(|| QueueBreakdown {
                queue_id: queue_id.clone(),
                operation,
                window: windows_by_queue.get(&queue_id).copied().unwrap_or_default(),
                observed_peak_running: observed_stats
                    .get(&queue_id)
                    .map(|stats| stats.peak_running)
                    .unwrap_or_default(),
                starts_last_minute: observed_stats
                    .get(&queue_id)
                    .map(|stats| stats.starts_last_minute)
                    .unwrap_or_default(),
                peak_starts_per_minute: observed_stats
                    .get(&queue_id)
                    .map(|stats| stats.peak_starts_per_minute)
                    .unwrap_or_default(),
                avg_running: observed_stats
                    .get(&queue_id)
                    .map(|stats| stats.avg_running)
                    .unwrap_or_default(),
                avg_queued: observed_stats
                    .get(&queue_id)
                    .map(|stats| stats.avg_queued)
                    .unwrap_or_default(),
                avg_starts_per_minute: observed_stats
                    .get(&queue_id)
                    .map(|stats| stats.avg_starts_per_minute)
                    .unwrap_or_default(),
                observed_duration_secs: observed_stats
                    .get(&queue_id)
                    .map(|stats| stats.observed_duration_secs)
                    .unwrap_or_default(),
                last_event_at: observed_stats
                    .get(&queue_id)
                    .and_then(|stats| stats.last_event_at.clone()),
                ..Default::default()
            });
        match status.as_str() {
            "queued" | "pending" => {
                entry.queued += 1;
                detail.queue.queued += 1;
            }
            "running" => {
                entry.running += 1;
                detail.queue.running += 1;
            }
            "succeeded" => {
                entry.succeeded += 1;
                detail.queue.succeeded += 1;
            }
            "failed" => {
                entry.failed += 1;
                detail.queue.failed += 1;
            }
            "dead" => {
                entry.dead += 1;
                detail.queue.dead += 1;
            }
            _ => {}
        }
    }
    detail.queue.in_flight = detail.queue.queued + detail.queue.running;
    for stage in by_queue.values_mut() {
        stage.in_flight = stage.queued + stage.running;
    }
    detail.queues = by_queue.into_values().collect();

    // --- model throughput + errors ---
    let model_path = first_existing(
        run_root,
        &[
            "raw/model-traces.jsonl",
            "model-traces.jsonl",
            "artifacts/model-traces.jsonl",
            "provider-queue/model-queue-traces.jsonl",
        ],
    )
    .unwrap_or_else(|| queue_path.clone());
    detail.model.total_bytes = std::fs::metadata(&model_path).map(|m| m.len()).unwrap_or(0);
    for trace in tail_values(&model_path, TAIL_BYTES) {
        let status = status_at(&trace);
        let provider_queue_event = model_path == queue_path;
        if provider_queue_event && !is_terminal_status(status) {
            continue;
        }
        if provider_queue_event || !status.is_empty() || trace.get("outcome").is_some() {
            detail.model.window_calls += 1;
            let failed = str_at(&trace, "outcome")
                .or_else(|| str_at(&trace, "status"))
                .is_some_and(|o| o != "succeeded");
            if failed {
                detail.model.window_failed += 1;
                push_error(&mut detail.errors, &trace, "model");
            }
            add_usage(&mut detail.model, &trace);
        }
    }

    // --- per-stage memory progress (cumulative over the whole run) ---
    let mut stages: BTreeMap<String, StageProgress> = BTreeMap::new();
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
        if timestamp_at(&trace)
            > entry.last_event_at.as_deref().and_then(|raw| {
                DateTime::parse_from_rfc3339(raw)
                    .ok()
                    .map(|dt| dt.with_timezone(&Utc))
            })
        {
            entry.last_event = Some(event.to_string());
            entry.last_event_at = timestamp_string(&trace);
        }
        push_activity(
            &mut detail.activity,
            timestamp_string(&trace),
            "memory",
            op.to_string(),
            event.to_string(),
            None,
            memory_message(&trace, event),
            if matches!(event, "operation_failed" | "batch_failed")
                || trace.get("error").is_some_and(|e| !e.is_null())
            {
                "error"
            } else {
                "info"
            },
        );
        match event {
            "operation_started" | "branch_started" | "batch_started" => entry.started += 1,
            "operation_succeeded" | "branch_joined" | "batch_succeeded" => entry.succeeded += 1,
            "operation_failed" | "batch_failed" => {
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
        .activity
        .sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    detail.activity.truncate(MAX_ACTIVITY);
    detail
}

fn push_error(errors: &mut Vec<LiveError>, trace: &Value, source: &str) {
    let message = str_at(trace, "error")
        .or_else(|| str_at(trace, "error_class"))
        .or_else(|| str_at(trace, "outcome"))
        .or_else(|| str_at(trace, "status"))
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
            "{\"queue_id\":\"chat:deepseek:deepseek-v4-flash\",\"item_id\":\"a\",\"operation\":\"chat\",\"status\":\"queued\",\"timestamp\":\"2026-06-18T10:00:00Z\"}\n\
             {\"queue_id\":\"chat:deepseek:deepseek-v4-flash\",\"item_id\":\"a\",\"operation\":\"chat\",\"status\":\"running\",\"timestamp\":\"2026-06-18T10:00:01Z\"}\n\
             {\"queue_id\":\"chat:deepseek:deepseek-v4-flash\",\"item_id\":\"b\",\"operation\":\"chat\",\"status\":\"running\",\"timestamp\":\"2026-06-18T10:00:02Z\"}\n\
             {\"queue_id\":\"chat:deepseek:deepseek-v4-flash\",\"item_id\":\"a\",\"operation\":\"chat\",\"status\":\"succeeded\",\"timestamp\":\"2026-06-18T10:00:03Z\"}\n",
        )
        .unwrap();
        std::fs::write(
            root.join("raw/model-traces.jsonl"),
            "{\"outcome\":\"succeeded\",\"usage\":{\"input_tokens\":10,\"output_tokens\":2}}\n{\"outcome\":\"failed\",\"error_class\":\"timeout\",\"error\":\"deadline\"}\n",
        )
        .unwrap();
        std::fs::write(
            root.join("traces/memory-events.jsonl"),
            "{\"operation\":\"capture\",\"event\":\"operation_succeeded\",\"timestamp\":\"2026-06-18T10:00:00Z\"}\n\
             {\"operation\":\"distill\",\"event\":\"operation_started\",\"timestamp\":\"2026-06-18T10:00:01Z\"}\n\
             {\"operation\":\"distill\",\"event\":\"operation_started\",\"timestamp\":\"2026-06-18T10:00:02Z\"}\n\
             {\"operation\":\"distill\",\"event\":\"operation_succeeded\",\"timestamp\":\"2026-06-18T10:00:03Z\"}\n",
        )
        .unwrap();

        let detail = live_detail(root);
        assert_eq!(detail.queue.succeeded, 1);
        assert_eq!(detail.queue.in_flight, 1); // item b still running
        assert_eq!(detail.queue.running, 1);
        assert_eq!(detail.queues.len(), 1);
        assert_eq!(detail.queues[0].running, 1);
        assert_eq!(detail.queues[0].succeeded, 1);
        assert_eq!(detail.queues[0].observed_peak_running, 2);
        assert_eq!(detail.queues[0].starts_last_minute, 2);
        assert_eq!(detail.queues[0].peak_starts_per_minute, 2);
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
        assert_eq!(distill.last_event.as_deref(), Some("operation_succeeded"));
        assert_eq!(detail.errors.len(), 1);
        assert_eq!(detail.errors[0].message, "deadline");
        assert!(detail.activity.iter().any(|row| row.source == "provider"));
        assert!(detail.activity.iter().any(|row| row.source == "memory"));
    }
}
