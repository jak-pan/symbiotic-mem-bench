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
/// Cap on live errors returned. Provider failures often arrive in bursts, and
/// those bursts are the debugging signal, so keep a real log instead of only a
/// tiny rolling sample.
const MAX_ERRORS: usize = 500;
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
    /// Logical request units represented by queued events. For batched
    /// embeddings this is individual texts; for chat this matches calls.
    pub queued_units: u64,
    pub running_units: u64,
    pub succeeded_units: u64,
    pub failed_units: u64,
    pub dead_units: u64,
    pub in_flight_units: u64,
    /// Running requests observed at the busiest point in the inspected window.
    pub observed_peak_running: u64,
    /// Logical request units running at the busiest point in the inspected window.
    pub observed_peak_running_units: u64,
    /// Running events in the last trace minute of the inspected window.
    pub starts_last_minute: u64,
    /// Logical request units started in the last trace minute of the inspected window.
    pub starts_last_minute_units: u64,
    /// Highest 60-second running-event count observed in the inspected window.
    pub peak_starts_per_minute: u64,
    /// Highest logical request-unit count observed in any 60-second window.
    pub peak_starts_per_minute_units: u64,
    /// Time-weighted average running requests over the inspected provider event span.
    pub avg_running: f64,
    /// Time-weighted average logical request units running over the inspected span.
    pub avg_running_units: f64,
    /// Time-weighted average queued requests over the inspected provider event span.
    pub avg_queued: f64,
    /// Time-weighted average logical request units queued over the inspected span.
    pub avg_queued_units: f64,
    /// Average request starts per minute over the inspected provider event span.
    pub avg_starts_per_minute: f64,
    /// Average logical request-unit starts per minute over the inspected span.
    pub avg_starts_per_minute_units: f64,
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

#[derive(Clone, Debug, Default, Serialize)]
pub struct StageSegment {
    pub id: String,
    pub started: u64,
    pub succeeded: u64,
    pub failed: u64,
    pub in_flight: u64,
    pub item_succeeded: u64,
    pub item_failed: u64,
    pub progress: f64,
    pub status: String,
}

/// Per-operation progress through the memory pipeline (cumulative over the run).
#[derive(Clone, Debug, Default, Serialize)]
pub struct StageProgress {
    pub operation: String,
    pub started: u64,
    pub succeeded: u64,
    pub failed: u64,
    /// Content items processed inside this stage (turns, facts, records, briefs,
    /// or retrieved context items). This is separate from branch/request counts.
    pub item_succeeded: u64,
    pub item_failed: u64,
    pub item_unit: String,
    /// `started - succeeded - failed`: requests currently in this stage.
    pub in_flight: u64,
    pub intermediate_failed: u64,
    pub segments: Vec<StageSegment>,
    pub last_event: Option<String>,
    pub last_event_at: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ErrorCategory {
    pub category: String,
    pub source: String,
    pub kind: Option<String>,
    pub count: u64,
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
    pub error_categories: Vec<ErrorCategory>,
    pub errors: Vec<LiveError>,
    /// Recent interleaved memory/provider events, newest first.
    pub activity: Vec<LiveActivity>,
}

/// Canonical pipeline order for memory operations, so stages render in the order
/// a request flows through them.
const STAGE_ORDER: &[&str] = &[
    "pre_capture_setup",
    "capture",
    "ingest",
    "distill",
    "write_archive",
    "embed_raw",
    "embed_facts",
    "index",
    "consolidate",
    "pre_recall_setup",
    "query_plan",
    "embed_query",
    "fact_search",
    "raw_search",
    "support_check",
    "answer_context",
    "retrieve",
    "answer",
    "score",
    "flush",
    "state_export",
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

fn memory_operation(trace: &Value) -> Option<String> {
    let op = str_at(trace, "operation")?;
    if op == "adapter_call" {
        if let Some(stage @ ("pre_capture_setup" | "pre_recall_setup")) = str_at(trace, "stage") {
            return Some(stage.to_string());
        }
        return None;
    }
    let metrics = trace.get("metrics").unwrap_or(&Value::Null);
    if op == "embed_facts" && str_at(metrics, "kind") == Some("brief") {
        return Some("consolidate".to_string());
    }
    Some(op.to_string())
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
    let input_tokens = usage_u64(trace, &["input_tokens", "prompt_tokens"]);
    model.input_tokens += if input_tokens == 0 && str_at(trace, "operation") == Some("embedding") {
        trace
            .get("input_units")
            .and_then(Value::as_u64)
            .unwrap_or(0)
    } else {
        input_tokens
    };
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
        for (key, label) in [
            ("store_open_ms", "store open"),
            ("zvec_cache_ms", "zvec"),
            ("load_existing_ms", "existing"),
            ("manifest_ms", "manifest"),
            ("load_counts_ms", "counts"),
            ("ensure_recall_index_ms", "ensure index"),
        ] {
            if let Some(value) = metrics.get(key).and_then(Value::as_u64) {
                parts.push(format!("{label} {value}ms"));
            }
        }
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

fn segment_status(segment: &StageSegment) -> String {
    if segment.failed > 0 {
        "failed".to_string()
    } else if segment.started > 0 && segment.succeeded >= segment.started {
        "done".to_string()
    } else if segment.succeeded > 0 {
        "partial".to_string()
    } else if segment.in_flight > 0 || segment.started > 0 {
        "running".to_string()
    } else {
        "pending".to_string()
    }
}

fn segment_id(trace: &Value) -> String {
    str_at(trace, "source_id")
        .or_else(|| str_at(trace, "run_id"))
        .or_else(|| str_at(trace, "question_id"))
        .unwrap_or("run")
        .to_string()
}

fn memory_event_key(operation: &str, segment_id: &str, trace: &Value) -> (String, String, String) {
    (
        operation.to_string(),
        segment_id.to_string(),
        timestamp_string(trace).unwrap_or_default(),
    )
}

fn recovered_memory_failures(traces: &[Value]) -> HashSet<(String, String, String)> {
    let mut terminal_success_at: HashMap<(String, String), DateTime<Utc>> = HashMap::new();
    for trace in traces {
        let Some(operation) = memory_operation(trace) else {
            continue;
        };
        if !matches!(
            str_at(trace, "event"),
            Some("operation_succeeded" | "branch_joined")
        ) {
            continue;
        }
        let Some(timestamp) = timestamp_at(trace) else {
            continue;
        };
        terminal_success_at
            .entry((operation, segment_id(trace)))
            .and_modify(|existing| *existing = (*existing).max(timestamp))
            .or_insert(timestamp);
    }

    let mut recovered = HashSet::new();
    for trace in traces {
        let Some(operation) = memory_operation(trace) else {
            continue;
        };
        if str_at(trace, "event") != Some("operation_failed") {
            continue;
        }
        let source = segment_id(trace);
        let Some(failed_at) = timestamp_at(trace) else {
            continue;
        };
        if terminal_success_at
            .get(&(operation.clone(), source.clone()))
            .is_some_and(|succeeded_at| *succeeded_at > failed_at)
        {
            recovered.insert(memory_event_key(&operation, &source, trace));
        }
    }
    recovered
}

fn error_category(message: &str, kind: Option<&str>) -> String {
    let raw = kind.unwrap_or(message).to_ascii_lowercase();
    let msg = message.to_ascii_lowercase();
    let hay = format!("{raw} {msg}");
    if hay.contains("rate limit") || hay.contains("429") || hay.contains("quota") {
        "rate_limit".to_string()
    } else if hay.contains("connect") && hay.contains("timeout") {
        "connect_timeout".to_string()
    } else if hay.contains("read timeout")
        || hay.contains("body timeout")
        || hay.contains("deadline")
    {
        "read_timeout".to_string()
    } else if hay.contains("timed out") || hay.contains("timeout") {
        "timeout".to_string()
    } else if hay.contains("connection closed") || hay.contains("closed before message") {
        "connection_closed".to_string()
    } else if hay.contains("503") || hay.contains("502") || hay.contains("500") {
        "provider_5xx".to_string()
    } else if hay.contains("400")
        || hay.contains("401")
        || hay.contains("403")
        || hay.contains("404")
    {
        "provider_4xx".to_string()
    } else if hay.contains("provider unavailable") {
        "provider_unavailable".to_string()
    } else {
        "other".to_string()
    }
}

fn summarize_error_categories(errors: &[LiveError]) -> Vec<ErrorCategory> {
    let mut grouped: BTreeMap<(String, String, Option<String>), u64> = BTreeMap::new();
    for error in errors {
        let category = error_category(&error.message, error.kind.as_deref());
        *grouped
            .entry((error.source.clone(), category, error.kind.clone()))
            .or_default() += 1;
    }
    let mut categories: Vec<ErrorCategory> = grouped
        .into_iter()
        .map(|((source, category, kind), count)| ErrorCategory {
            category,
            source,
            kind,
            count,
        })
        .collect();
    categories.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.source.cmp(&b.source)));
    categories
}

fn metric_u64(metrics: &Value, keys: &[&str]) -> Option<(u64, &'static str)> {
    for key in keys {
        if let Some(value) = metrics.get(*key).and_then(Value::as_u64) {
            let unit = match *key {
                "turn_count" | "raw_turn_count" | "total_turn_count" => "turns",
                "fact_count" | "base_fact_count" | "total_fact_count" => "facts",
                "record_count" => "records",
                "brief_count" | "extractive_brief_count" => "briefs",
                "context_item_count" | "evidence_id_count" => "ctx",
                _ => "items",
            };
            return Some((value, unit));
        }
    }
    None
}

fn memory_item_count(trace: &Value) -> Option<(u64, &'static str)> {
    let metrics = trace.get("metrics").unwrap_or(&Value::Null);
    if str_at(metrics, "kind") == Some("brief")
        && let Some(value) = metrics.get("fact_count").and_then(Value::as_u64)
    {
        return Some((value, "briefs"));
    }
    metric_u64(
        metrics,
        &[
            "turn_count",
            "fact_count",
            "record_count",
            "brief_count",
            "context_item_count",
            "evidence_id_count",
            "raw_turn_count",
            "base_fact_count",
            "extractive_brief_count",
            "total_turn_count",
            "total_fact_count",
        ],
    )
    .or_else(|| {
        trace
            .get("item_count")
            .and_then(Value::as_u64)
            .map(|count| (count, "items"))
    })
}

#[allow(clippy::too_many_arguments)]
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
    peak_running_units: u64,
    starts_last_minute: u64,
    starts_last_minute_units: u64,
    peak_starts_per_minute: u64,
    peak_starts_per_minute_units: u64,
    avg_running: f64,
    avg_running_units: f64,
    avg_queued: f64,
    avg_queued_units: f64,
    avg_starts_per_minute: f64,
    avg_starts_per_minute_units: f64,
    observed_duration_secs: f64,
    last_event_at: Option<String>,
}

fn request_units_at(event: &Value) -> u64 {
    event
        .get("request_units")
        .and_then(Value::as_u64)
        .unwrap_or(1)
        .max(1)
}

fn unit_sum(items: &HashMap<String, u64>) -> u64 {
    items.values().sum()
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
        let mut running_units_at: HashMap<String, u64> = HashMap::new();
        let mut queued_units_at: HashMap<String, u64> = HashMap::new();
        let mut peak_running = 0_u64;
        let mut peak_running_units = 0_u64;
        let mut starts: Vec<(DateTime<Utc>, u64)> = Vec::new();
        let mut last_event_at = None;
        let mut prev_ts = first_ts;
        let mut running_weighted_ms = 0_f64;
        let mut queued_weighted_ms = 0_f64;
        let mut running_unit_weighted_ms = 0_f64;
        let mut queued_unit_weighted_ms = 0_f64;

        for event in queue_events {
            let ts = timestamp_at(event).unwrap_or(prev_ts);
            let elapsed_ms = (ts - prev_ts).num_milliseconds().max(0) as f64;
            running_weighted_ms += running_at.len() as f64 * elapsed_ms;
            queued_weighted_ms += queued_at.len() as f64 * elapsed_ms;
            running_unit_weighted_ms += unit_sum(&running_units_at) as f64 * elapsed_ms;
            queued_unit_weighted_ms += unit_sum(&queued_units_at) as f64 * elapsed_ms;
            prev_ts = ts;
            last_event_at = Some(ts.to_rfc3339_opts(chrono::SecondsFormat::Millis, true));
            let status = status_at(event);
            let request_units = request_units_at(event);
            if status == "running" {
                starts.push((ts, request_units));
                if last_cutoff.is_some_and(|cutoff| ts >= cutoff) {
                    last_minute += 1;
                }
                if let Some(item) = str_at(event, "item_id") {
                    queued_at.remove(item);
                    queued_units_at.remove(item);
                    running_at.insert(item.to_string());
                    running_units_at.insert(item.to_string(), request_units);
                }
                peak_running = peak_running.max(running_at.len() as u64);
                peak_running_units = peak_running_units.max(unit_sum(&running_units_at));
            } else if matches!(status, "queued" | "pending") {
                if let Some(item) = str_at(event, "item_id") {
                    queued_at.insert(item.to_string());
                    queued_units_at.insert(item.to_string(), request_units);
                }
            } else if is_terminal_status(status)
                && let Some(item) = str_at(event, "item_id")
            {
                running_at.remove(item);
                queued_at.remove(item);
                running_units_at.remove(item);
                queued_units_at.remove(item);
            }
        }
        starts.sort();
        let mut left = 0_usize;
        let mut peak_rpm = 0_u64;
        let mut peak_rpm_units = 0_u64;
        let mut window_units = 0_u64;
        for right in 0..starts.len() {
            window_units += starts[right].1;
            while starts[right].0 - starts[left].0 > Duration::seconds(60) {
                window_units = window_units.saturating_sub(starts[left].1);
                left += 1;
            }
            peak_rpm = peak_rpm.max((right - left + 1) as u64);
            peak_rpm_units = peak_rpm_units.max(window_units);
        }
        let starts_last_minute_units = starts
            .iter()
            .filter(|(ts, _)| last_cutoff.is_some_and(|cutoff| *ts >= cutoff))
            .map(|(_, units)| *units)
            .sum();
        let duration_ms = (max_ts - first_ts).num_milliseconds().max(1) as f64;
        stats.insert(
            queue_id,
            QueueObservedStats {
                peak_running,
                peak_running_units,
                starts_last_minute: last_minute,
                starts_last_minute_units,
                peak_starts_per_minute: peak_rpm,
                peak_starts_per_minute_units: peak_rpm_units,
                avg_running: running_weighted_ms / duration_ms,
                avg_running_units: running_unit_weighted_ms / duration_ms,
                avg_queued: queued_weighted_ms / duration_ms,
                avg_queued_units: queued_unit_weighted_ms / duration_ms,
                avg_starts_per_minute: starts.len() as f64 / (duration_ms / 60_000_f64),
                avg_starts_per_minute_units: starts.iter().map(|(_, units)| *units).sum::<u64>()
                    as f64
                    / (duration_ms / 60_000_f64),
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
    let mut last_status: HashMap<String, (String, String, String, u64)> = HashMap::new();
    let mut windows_by_queue: BTreeMap<String, u64> = BTreeMap::new();
    for event in &queue_events {
        let status = status_at(event);
        let queue_id = queue_group_key(event);
        let operation = str_at(event, "operation")
            .or_else(|| str_at(event, "kind"))
            .unwrap_or("")
            .to_string();
        let request_units = request_units_at(event);
        *windows_by_queue.entry(queue_id.clone()).or_default() += 1;
        if let Some(item) = str_at(event, "item_id") {
            last_status.insert(
                item.to_string(),
                (
                    status.to_string(),
                    queue_id.clone(),
                    operation.clone(),
                    request_units,
                ),
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
    for (_, (status, queue_id, operation, request_units)) in last_status {
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
                observed_peak_running_units: observed_stats
                    .get(&queue_id)
                    .map(|stats| stats.peak_running_units)
                    .unwrap_or_default(),
                starts_last_minute: observed_stats
                    .get(&queue_id)
                    .map(|stats| stats.starts_last_minute)
                    .unwrap_or_default(),
                starts_last_minute_units: observed_stats
                    .get(&queue_id)
                    .map(|stats| stats.starts_last_minute_units)
                    .unwrap_or_default(),
                peak_starts_per_minute: observed_stats
                    .get(&queue_id)
                    .map(|stats| stats.peak_starts_per_minute)
                    .unwrap_or_default(),
                peak_starts_per_minute_units: observed_stats
                    .get(&queue_id)
                    .map(|stats| stats.peak_starts_per_minute_units)
                    .unwrap_or_default(),
                avg_running: observed_stats
                    .get(&queue_id)
                    .map(|stats| stats.avg_running)
                    .unwrap_or_default(),
                avg_running_units: observed_stats
                    .get(&queue_id)
                    .map(|stats| stats.avg_running_units)
                    .unwrap_or_default(),
                avg_queued: observed_stats
                    .get(&queue_id)
                    .map(|stats| stats.avg_queued)
                    .unwrap_or_default(),
                avg_queued_units: observed_stats
                    .get(&queue_id)
                    .map(|stats| stats.avg_queued_units)
                    .unwrap_or_default(),
                avg_starts_per_minute: observed_stats
                    .get(&queue_id)
                    .map(|stats| stats.avg_starts_per_minute)
                    .unwrap_or_default(),
                avg_starts_per_minute_units: observed_stats
                    .get(&queue_id)
                    .map(|stats| stats.avg_starts_per_minute_units)
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
                entry.queued_units += request_units;
                detail.queue.queued += 1;
            }
            "running" => {
                entry.running += 1;
                entry.running_units += request_units;
                detail.queue.running += 1;
            }
            "succeeded" => {
                entry.succeeded += 1;
                entry.succeeded_units += request_units;
                detail.queue.succeeded += 1;
            }
            "failed" => {
                entry.failed += 1;
                entry.failed_units += request_units;
                detail.queue.failed += 1;
            }
            "dead" => {
                entry.dead += 1;
                entry.dead_units += request_units;
                detail.queue.dead += 1;
            }
            _ => {}
        }
    }
    detail.queue.in_flight = detail.queue.queued + detail.queue.running;
    for stage in by_queue.values_mut() {
        stage.in_flight = stage.queued + stage.running;
        stage.in_flight_units = stage.queued_units + stage.running_units;
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
        let provider_queue_event = model_path == queue_path || trace.get("queue_id").is_some();
        if provider_queue_event && !is_terminal_status(status) {
            continue;
        }
        if provider_queue_event || !status.is_empty() || trace.get("outcome").is_some() {
            detail.model.window_calls += 1;
            let failed = if provider_queue_event {
                matches!(status, "failed" | "dead")
            } else {
                str_at(&trace, "outcome")
                    .or_else(|| str_at(&trace, "status"))
                    .is_some_and(|o| o != "succeeded")
            };
            if failed {
                detail.model.window_failed += 1;
                if !provider_queue_event {
                    push_error(&mut detail.errors, &trace, "model");
                }
            }
            add_usage(&mut detail.model, &trace);
        }
    }

    // --- per-stage memory progress (cumulative over the whole run) ---
    let mut stages: BTreeMap<String, StageProgress> = BTreeMap::new();
    let mut stage_segments: BTreeMap<String, BTreeMap<String, StageSegment>> = BTreeMap::new();
    let mut batch_item_succeeded: BTreeMap<String, u64> = BTreeMap::new();
    let mut terminal_item_succeeded: BTreeMap<String, u64> = BTreeMap::new();
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
    let memory_traces = read_values_capped(&memory_path, MEMORY_FULL_CAP);
    let recovered_failures = recovered_memory_failures(&memory_traces);
    for trace in memory_traces {
        let Some(op) = memory_operation(&trace) else {
            continue;
        };
        let entry = stages.entry(op.clone()).or_insert_with(|| StageProgress {
            operation: op.clone(),
            ..Default::default()
        });
        let event = str_at(&trace, "event").unwrap_or("");
        let segment_id = segment_id(&trace);
        let recovered_failure = event == "operation_failed"
            && recovered_failures.contains(&memory_event_key(&op, &segment_id, &trace));
        let segment = stage_segments
            .entry(op.clone())
            .or_default()
            .entry(segment_id.clone())
            .or_insert_with(|| StageSegment {
                id: segment_id,
                ..Default::default()
            });
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
            op.clone(),
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
            "operation_started" | "branch_started" | "batch_started" => {
                entry.started += 1;
                segment.started += 1;
            }
            "operation_succeeded" | "branch_joined" => {
                entry.succeeded += 1;
                segment.succeeded += 1;
            }
            "batch_succeeded" => {
                entry.started += 1;
                entry.succeeded += 1;
                segment.started += 1;
                segment.succeeded += 1;
            }
            "operation_failed" => {
                if recovered_failure {
                    entry.succeeded += 1;
                    segment.succeeded += 1;
                    entry.intermediate_failed += 1;
                } else {
                    entry.failed += 1;
                    segment.failed += 1;
                    if let Some((count, unit)) = memory_item_count(&trace) {
                        entry.item_failed += count;
                        entry.item_unit = unit.to_string();
                        segment.item_failed += count;
                    }
                    detail.memory_failures += 1;
                }
                push_error(&mut detail.errors, &trace, "memory");
            }
            "batch_failed" => {
                entry.started += 1;
                entry.failed += 1;
                entry.intermediate_failed += 1;
                segment.started += 1;
                segment.failed += 1;
                if let Some((count, unit)) = memory_item_count(&trace) {
                    entry.item_failed += count;
                    entry.item_unit = unit.to_string();
                    segment.item_failed += count;
                }
                detail.memory_failures += 1;
                push_error(&mut detail.errors, &trace, "memory");
            }
            _ => {
                if trace.get("error").is_some_and(|e| !e.is_null()) {
                    push_error(&mut detail.errors, &trace, "memory");
                }
            }
        }
        if matches!(
            event,
            "batch_succeeded" | "operation_succeeded" | "branch_joined"
        ) && let Some((count, unit)) = memory_item_count(&trace)
        {
            entry.item_unit = unit.to_string();
            segment.item_succeeded += count;
            if event == "batch_succeeded" {
                *batch_item_succeeded.entry(op.clone()).or_default() += count;
            } else {
                *terminal_item_succeeded.entry(op.clone()).or_default() += count;
            }
        }
    }
    for stage in stages.values_mut() {
        stage.in_flight = stage
            .started
            .saturating_sub(stage.succeeded)
            .saturating_sub(stage.failed);
        stage.segments = stage_segments
            .remove(&stage.operation)
            .unwrap_or_default()
            .into_values()
            .map(|mut segment| {
                segment.in_flight = segment
                    .started
                    .saturating_sub(segment.succeeded)
                    .saturating_sub(segment.failed);
                let total = segment.started.max(1);
                segment.progress =
                    ((segment.succeeded + segment.failed) as f64 / total as f64).clamp(0.0, 1.0);
                segment.status = segment_status(&segment);
                segment
            })
            .collect();
        stage.item_succeeded = batch_item_succeeded
            .get(&stage.operation)
            .copied()
            .unwrap_or_else(|| {
                terminal_item_succeeded
                    .get(&stage.operation)
                    .copied()
                    .unwrap_or_default()
            });
        if stage.item_unit.is_empty() {
            stage.item_unit = "items".to_string();
        }
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
    detail.error_categories = summarize_error_categories(&detail.errors);
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
            "{\"outcome\":\"succeeded\",\"usage\":{\"input_tokens\":10,\"output_tokens\":2}}\n\
             {\"queue_id\":\"embedding:openrouter:qwen/qwen3-embedding-8b\",\"operation\":\"embedding\",\"status\":\"queued\",\"timestamp\":\"2026-06-18T10:00:00Z\",\"input_units\":100}\n\
             {\"queue_id\":\"embedding:openrouter:qwen/qwen3-embedding-8b\",\"operation\":\"embedding\",\"status\":\"running\",\"timestamp\":\"2026-06-18T10:00:01Z\",\"input_units\":100}\n\
             {\"queue_id\":\"embedding:openrouter:qwen/qwen3-embedding-8b\",\"operation\":\"embedding\",\"status\":\"succeeded\",\"timestamp\":\"2026-06-18T10:00:02Z\",\"input_units\":100}\n\
             {\"queue_id\":\"embedding:openrouter:qwen/qwen3-embedding-8b\",\"operation\":\"embedding\",\"status\":\"failed\",\"timestamp\":\"2026-06-18T10:00:03Z\",\"error\":\"queue timeout\"}\n\
             {\"outcome\":\"failed\",\"error_class\":\"timeout\",\"error\":\"deadline\"}\n",
        )
        .unwrap();
        std::fs::write(
            root.join("traces/memory-events.jsonl"),
            "{\"operation\":\"capture\",\"event\":\"operation_succeeded\",\"timestamp\":\"2026-06-18T10:00:00Z\"}\n\
             {\"operation\":\"distill\",\"event\":\"operation_started\",\"timestamp\":\"2026-06-18T10:00:01Z\"}\n\
             {\"operation\":\"distill\",\"event\":\"operation_started\",\"timestamp\":\"2026-06-18T10:00:02Z\"}\n\
             {\"operation\":\"distill\",\"event\":\"operation_succeeded\",\"timestamp\":\"2026-06-18T10:00:03Z\",\"duration_ms\":250,\"item_count\":7,\"metrics\":{\"fact_count\":7}}\n\
             {\"operation\":\"embed_raw\",\"event\":\"batch_succeeded\",\"timestamp\":\"2026-06-18T10:00:03Z\",\"duration_ms\":50,\"item_count\":5,\"metrics\":{\"turn_count\":5}}\n\
             {\"operation\":\"embed_raw\",\"event\":\"batch_succeeded\",\"timestamp\":\"2026-06-18T10:00:04Z\",\"duration_ms\":150,\"item_count\":4,\"metrics\":{\"turn_count\":4}}\n\
             {\"operation\":\"embed_raw\",\"event\":\"branch_joined\",\"timestamp\":\"2026-06-18T10:00:05Z\",\"item_count\":20,\"metrics\":{\"turn_count\":20}}\n\
             {\"operation\":\"embed_facts\",\"event\":\"batch_succeeded\",\"timestamp\":\"2026-06-18T10:00:05Z\",\"item_count\":3,\"metrics\":{\"fact_count\":3,\"kind\":\"fact\"}}\n\
             {\"operation\":\"embed_facts\",\"event\":\"batch_succeeded\",\"timestamp\":\"2026-06-18T10:00:06Z\",\"item_count\":2,\"metrics\":{\"fact_count\":2,\"kind\":\"brief\"}}\n\
             {\"operation\":\"embed_facts\",\"event\":\"operation_succeeded\",\"timestamp\":\"2026-06-18T10:00:07Z\",\"item_count\":3,\"metrics\":{\"fact_count\":3}}\n\
             {\"operation\":\"answer\",\"event\":\"operation_started\",\"timestamp\":\"2026-06-18T10:00:04Z\"}\n\
             {\"operation\":\"query_plan\",\"event\":\"operation_succeeded\",\"timestamp\":\"2026-06-18T10:00:05Z\"}\n",
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
        assert_eq!(detail.model.window_calls, 4);
        assert_eq!(detail.model.window_failed, 2);
        assert_eq!(detail.model.input_tokens, 110);
        // capture before distill (pipeline order); distill: 2 started, 1 done → 1 in flight
        assert_eq!(detail.memory_stages[0].operation, "capture");
        let query_plan_idx = detail
            .memory_stages
            .iter()
            .position(|s| s.operation == "query_plan")
            .unwrap();
        let answer_idx = detail
            .memory_stages
            .iter()
            .position(|s| s.operation == "answer")
            .unwrap();
        assert!(query_plan_idx < answer_idx);
        let distill = detail
            .memory_stages
            .iter()
            .find(|s| s.operation == "distill")
            .unwrap();
        assert_eq!(distill.started, 2);
        assert_eq!(distill.succeeded, 1);
        assert_eq!(distill.in_flight, 1);
        assert_eq!(distill.item_succeeded, 7);
        assert_eq!(distill.item_unit, "facts");
        assert_eq!(distill.last_event.as_deref(), Some("operation_succeeded"));
        assert_eq!(distill.segments.len(), 1);
        assert_eq!(distill.segments[0].status, "partial");
        let embed_raw = detail
            .memory_stages
            .iter()
            .find(|s| s.operation == "embed_raw")
            .unwrap();
        assert_eq!(embed_raw.succeeded, 3);
        assert_eq!(embed_raw.item_succeeded, 9); // prefer batch item progress over final branch total
        assert_eq!(embed_raw.item_unit, "turns");
        let embed_facts = detail
            .memory_stages
            .iter()
            .find(|s| s.operation == "embed_facts")
            .unwrap();
        assert_eq!(embed_facts.item_succeeded, 3);
        assert_eq!(embed_facts.item_unit, "facts");
        let consolidate = detail
            .memory_stages
            .iter()
            .find(|s| s.operation == "consolidate")
            .unwrap();
        assert_eq!(consolidate.item_succeeded, 2);
        assert_eq!(consolidate.item_unit, "briefs");
        assert_eq!(detail.errors.len(), 1);
        assert_eq!(detail.errors[0].message, "deadline");
        assert_eq!(detail.error_categories.len(), 1);
        assert_eq!(detail.error_categories[0].category, "read_timeout");
        assert!(
            !detail
                .errors
                .iter()
                .any(|error| error.source == "model" && error.message == "queue timeout")
        );
        assert!(detail.activity.iter().any(|row| row.source == "provider"));
        assert!(detail.activity.iter().any(|row| row.source == "memory"));
    }

    #[test]
    fn live_detail_treats_failed_attempt_followed_by_success_as_recovered() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("provider-queue")).unwrap();
        std::fs::create_dir_all(root.join("traces")).unwrap();
        std::fs::write(
            root.join("provider-queue").join("model-queue-traces.jsonl"),
            "",
        )
        .unwrap();
        std::fs::write(
            root.join("traces/memory-events.jsonl"),
            "{\"operation\":\"distill\",\"event\":\"operation_started\",\"source_id\":\"q1\",\"timestamp\":\"2026-06-18T10:00:00Z\"}\n\
             {\"operation\":\"distill\",\"event\":\"operation_failed\",\"source_id\":\"q1\",\"timestamp\":\"2026-06-18T10:00:10Z\",\"metrics\":{\"fact_count\":12},\"error\":\"temporary length cap\"}\n\
             {\"operation\":\"distill\",\"event\":\"branch_started\",\"source_id\":\"q1\",\"timestamp\":\"2026-06-18T10:01:00Z\"}\n\
             {\"operation\":\"distill\",\"event\":\"batch_succeeded\",\"source_id\":\"q1\",\"timestamp\":\"2026-06-18T10:01:05Z\",\"metrics\":{\"fact_count\":7}}\n\
             {\"operation\":\"distill\",\"event\":\"branch_joined\",\"source_id\":\"q1\",\"timestamp\":\"2026-06-18T10:01:10Z\",\"metrics\":{\"fact_count\":7}}\n",
        )
        .unwrap();

        let detail = live_detail(root);
        assert_eq!(detail.memory_failures, 0);
        assert_eq!(detail.errors.len(), 1);
        let distill = detail
            .memory_stages
            .iter()
            .find(|s| s.operation == "distill")
            .unwrap();
        assert_eq!(distill.started, 3);
        assert_eq!(distill.succeeded, 3);
        assert_eq!(distill.failed, 0);
        assert_eq!(distill.in_flight, 0);
        assert_eq!(distill.intermediate_failed, 1);
        assert_eq!(distill.item_failed, 0);
        assert_eq!(distill.item_succeeded, 7);
        assert_eq!(distill.segments[0].status, "done");
    }
}
