use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Default)]
struct StepAcc {
    started: u64,
    succeeded: u64,
    failed: u64,
    durations_ms: Vec<i64>,
    failed_durations_ms: Vec<i64>,
    terminal_durations_ms: Vec<i64>,
    item_counts: Vec<i64>,
    numeric_metrics: BTreeMap<String, Vec<f64>>,
}

#[derive(Default)]
struct QuestionAcc {
    events: u64,
    first_ts: Option<DateTime<Utc>>,
    last_ts: Option<DateTime<Utc>>,
    operations: BTreeMap<String, StepAcc>,
    context_chars: Option<u64>,
    context_item_count: Option<u64>,
    answer_chars: Option<u64>,
    answer_ms: Option<i64>,
    query_plan_ms: Option<i64>,
    retrieval_query_count: Option<u64>,
    dense_query_count: Option<u64>,
    sparse_term_count: Option<u64>,
    fact_searches: u64,
    raw_searches: u64,
}

#[derive(Default)]
struct ModelItemAcc {
    queue_id: Option<String>,
    role_binding: Option<String>,
    operation: Option<String>,
    model: Option<String>,
    first_ts: Option<DateTime<Utc>>,
    last_ts: Option<DateTime<Utc>>,
    queued_ts: Option<DateTime<Utc>>,
    terminal_ts: Option<DateTime<Utc>>,
    terminal_status: Option<String>,
    statuses: Vec<String>,
    timing_total_ms: Option<i64>,
}

#[derive(Default)]
struct ModelGroupAcc {
    items: u64,
    failed_items: u64,
    queue_durations_ms: Vec<i64>,
    failed_queue_durations_ms: Vec<i64>,
    terminal_queue_durations_ms: Vec<i64>,
    timing_total_ms: Vec<i64>,
    failed_timing_total_ms: Vec<i64>,
    terminal_timing_total_ms: Vec<i64>,
}

pub fn derive_run_step_analytics(run_root: &Path) -> anyhow::Result<Option<Value>> {
    let memory_path = run_root.join("artifacts").join("memory-traces.jsonl");
    let model_path = run_root.join("artifacts").join("model-traces.jsonl");
    let has_memory = memory_path.exists();
    let has_model = model_path.exists();
    if !has_memory && !has_model {
        return Ok(None);
    }

    let mut report = json!({
        "schema": "membench.step_analytics.v1",
        "generated_at": Utc::now().to_rfc3339(),
        "sources": {
            "memory_traces": has_memory,
            "model_traces": has_model,
            "question_debug": run_root.join("vaults").exists(),
        }
    });
    if has_memory {
        report["memory"] = derive_memory(&memory_path)?;
    }
    if has_model {
        report["model"] = derive_model(&model_path)?;
    }
    let debug = derive_question_debug(run_root)?;
    if !debug.as_array().is_none_or(Vec::is_empty) {
        report["question_debug"] = json!({
            "questions": debug,
            "note": "retrieval queries are hashed, not stored raw, to keep portable artifacts publishable"
        });
    }
    Ok(Some(report))
}

fn derive_memory(path: &Path) -> anyhow::Result<Value> {
    let rows = read_jsonl(path)?;
    let mut first_ts = None;
    let mut last_ts = None;
    let mut operations: BTreeMap<String, StepAcc> = BTreeMap::new();
    let mut questions: BTreeMap<String, QuestionAcc> = BTreeMap::new();

    for row in rows {
        let ts = row.get("timestamp").and_then(parse_ts);
        update_span(&mut first_ts, &mut last_ts, ts);
        let operation = row
            .get("operation")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string();
        update_step(operations.entry(operation.clone()).or_default(), &row);

        let question_id = row
            .get("question_id")
            .or_else(|| row.get("source_id"))
            .and_then(Value::as_str)
            .unwrap_or_default();
        if question_id.is_empty() {
            continue;
        }
        let q = questions.entry(question_id.to_string()).or_default();
        q.events += 1;
        update_span(&mut q.first_ts, &mut q.last_ts, ts);
        update_step(q.operations.entry(operation.clone()).or_default(), &row);

        if row.get("event").and_then(Value::as_str) == Some("operation_succeeded") {
            let metrics = row.get("metrics").and_then(Value::as_object);
            if operation == "answer_context" {
                q.context_chars = metrics
                    .and_then(|m| m.get("context_chars"))
                    .and_then(Value::as_u64);
                q.context_item_count = metrics
                    .and_then(|m| m.get("context_item_count"))
                    .and_then(Value::as_u64);
            } else if operation == "answer" {
                q.answer_chars = metrics
                    .and_then(|m| m.get("answer_chars"))
                    .and_then(Value::as_u64);
                q.answer_ms = row.get("duration_ms").and_then(Value::as_i64);
            } else if operation == "query_plan" {
                q.query_plan_ms = row.get("duration_ms").and_then(Value::as_i64);
                q.retrieval_query_count = metrics
                    .and_then(|m| m.get("retrieval_query_count"))
                    .and_then(Value::as_u64);
                q.dense_query_count = metrics
                    .and_then(|m| m.get("dense_query_count"))
                    .and_then(Value::as_u64);
                q.sparse_term_count = metrics
                    .and_then(|m| m.get("sparse_term_count"))
                    .and_then(Value::as_u64);
            } else if operation == "fact_search" {
                q.fact_searches += 1;
            } else if operation == "raw_search" {
                q.raw_searches += 1;
            }
        }
    }

    Ok(json!({
        "trace_span": span_json(first_ts, last_ts),
        "operations": operations.into_iter().map(|(operation, acc)| {
            let mut value = step_json(acc);
            value["operation"] = json!(operation);
            value
        }).collect::<Vec<_>>(),
        "questions": questions.into_iter().map(|(question_id, acc)| question_json(question_id, acc)).collect::<Vec<_>>(),
    }))
}

fn update_step(acc: &mut StepAcc, row: &Value) {
    let event = row.get("event").and_then(Value::as_str);
    match event {
        Some("operation_started") => acc.started += 1,
        Some("operation_succeeded") => acc.succeeded += 1,
        Some("operation_failed") => acc.failed += 1,
        _ => {}
    }
    if let Some(duration) = row.get("duration_ms").and_then(Value::as_i64) {
        match event {
            Some("operation_succeeded") => acc.durations_ms.push(duration),
            Some("operation_failed") => acc.failed_durations_ms.push(duration),
            _ => {}
        }
        if matches!(
            event,
            Some("operation_succeeded") | Some("operation_failed")
        ) {
            acc.terminal_durations_ms.push(duration);
        }
    }
    if let Some(item_count) = row.get("item_count").and_then(Value::as_i64) {
        acc.item_counts.push(item_count);
    }
    if let Some(metrics) = row.get("metrics").and_then(Value::as_object) {
        for (key, value) in metrics {
            if let Some(number) = value.as_f64() {
                acc.numeric_metrics
                    .entry(key.clone())
                    .or_default()
                    .push(number);
            }
        }
    }
}

fn derive_model(path: &Path) -> anyhow::Result<Value> {
    let rows = read_jsonl(path)?;
    let mut first_ts = None;
    let mut last_ts = None;
    let mut items: BTreeMap<String, ModelItemAcc> = BTreeMap::new();

    for row in rows {
        let item_id = row
            .get("item_id")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if item_id.is_empty() {
            continue;
        }
        let ts = row.get("timestamp").and_then(parse_ts);
        update_span(&mut first_ts, &mut last_ts, ts);
        let item = items.entry(item_id.to_string()).or_default();
        update_span(&mut item.first_ts, &mut item.last_ts, ts);
        if let Some(queue_id) = row.get("queue_id").and_then(Value::as_str) {
            item.queue_id = Some(queue_id.to_string());
        }
        if let Some(role) = row.get("role_binding").and_then(Value::as_str) {
            item.role_binding = Some(role.to_string());
        }
        if let Some(operation) = row.get("operation").and_then(Value::as_str) {
            item.operation = Some(operation.to_string());
        }
        if let Some(status) = row.get("status").and_then(Value::as_str) {
            item.statuses.push(status.to_string());
            if status == "queued" {
                item.queued_ts
                    .get_or_insert_with(|| ts.unwrap_or_else(Utc::now));
            }
            if is_terminal_status(status) {
                item.terminal_ts = ts;
                item.terminal_status = Some(status.to_string());
            }
        }
        if let Some(model) = model_id(&row) {
            item.model = Some(model);
        }
        if let Some(total_ms) = row
            .get("timing")
            .and_then(|timing| timing.get("total_ms"))
            .and_then(Value::as_i64)
        {
            item.timing_total_ms = Some(total_ms);
        }
    }

    let mut by_queue: BTreeMap<String, ModelGroupAcc> = BTreeMap::new();
    let mut by_role: BTreeMap<String, ModelGroupAcc> = BTreeMap::new();
    let mut by_model: BTreeMap<String, ModelGroupAcc> = BTreeMap::new();
    let mut terminal_items = 0;
    let mut failed_items = 0;
    for item in items.values() {
        if item.terminal_ts.is_some() {
            terminal_items += 1;
        }
        let failed = item
            .terminal_status
            .as_deref()
            .is_some_and(is_failed_terminal_status);
        if failed {
            failed_items += 1;
        }
        let queue_duration = item
            .queued_ts
            .zip(item.terminal_ts)
            .map(|(start, end)| (end - start).num_milliseconds());
        update_model_group(
            by_queue
                .entry(
                    item.queue_id
                        .clone()
                        .unwrap_or_else(|| "unknown".to_string()),
                )
                .or_default(),
            failed,
            queue_duration,
            item.timing_total_ms,
        );
        update_model_group(
            by_role
                .entry(
                    item.role_binding
                        .clone()
                        .unwrap_or_else(|| "unknown".to_string()),
                )
                .or_default(),
            failed,
            queue_duration,
            item.timing_total_ms,
        );
        update_model_group(
            by_model
                .entry(item.model.clone().unwrap_or_else(|| "unknown".to_string()))
                .or_default(),
            failed,
            queue_duration,
            item.timing_total_ms,
        );
    }

    Ok(json!({
        "trace_span": span_json(first_ts, last_ts),
        "items": {
            "total": items.len(),
            "terminal": terminal_items,
            "failed": failed_items,
        },
        "by_queue": model_groups_json(by_queue),
        "by_role": model_groups_json(by_role),
        "by_model": model_groups_json(by_model),
    }))
}

fn update_model_group(
    group: &mut ModelGroupAcc,
    failed: bool,
    queue_duration_ms: Option<i64>,
    timing_total_ms: Option<i64>,
) {
    group.items += 1;
    if failed {
        group.failed_items += 1;
    }
    if let Some(duration) = queue_duration_ms {
        if failed {
            group.failed_queue_durations_ms.push(duration);
        } else {
            group.queue_durations_ms.push(duration);
        }
        group.terminal_queue_durations_ms.push(duration);
    }
    if let Some(total) = timing_total_ms {
        if failed {
            group.failed_timing_total_ms.push(total);
        } else {
            group.timing_total_ms.push(total);
        }
        group.terminal_timing_total_ms.push(total);
    }
}

fn derive_question_debug(run_root: &Path) -> anyhow::Result<Value> {
    let vaults = run_root.join("vaults");
    if !vaults.is_dir() {
        return Ok(json!([]));
    }
    let mut rows = Vec::new();
    for entry in std::fs::read_dir(vaults)? {
        let entry = entry?;
        let path = entry.path().join("debug").join("question-debug.json");
        if !path.is_file() {
            continue;
        }
        let value: Value = serde_json::from_str(&std::fs::read_to_string(&path)?)?;
        let question_id = value
            .get("question")
            .and_then(|q| q.get("id"))
            .and_then(Value::as_str)
            .or_else(|| value.get("question_id").and_then(Value::as_str))
            .unwrap_or_default();
        let recall = match value.get("recall") {
            Some(recall) => recall,
            None => continue,
        };
        let retrieval_queries = recall.get("retrieval_queries");
        let query_plan = recall.get("query_plan");
        rows.push(json!({
            "question_id": question_id,
            "debug_artifact": path.strip_prefix(run_root).unwrap_or(&path).display().to_string(),
            "retrieval_query_count": retrieval_queries.and_then(Value::as_array).map_or(0, Vec::len),
            "retrieval_queries_hash": retrieval_queries.map(stable_hash_value),
            "query_plan_hash": query_plan.map(stable_hash_value),
            "recall_profile": recall.get("recall_profile").cloned().unwrap_or(Value::Null),
            "answerer_call_count": recall.get("answerer_calls").and_then(Value::as_array).map_or(0, Vec::len),
        }));
    }
    rows.sort_by(|a, b| {
        a.get("question_id")
            .and_then(Value::as_str)
            .cmp(&b.get("question_id").and_then(Value::as_str))
    });
    Ok(Value::Array(rows))
}

fn step_json(mut acc: StepAcc) -> Value {
    let metric_summaries = acc
        .numeric_metrics
        .into_iter()
        .map(|(key, mut values)| {
            values.sort_by(|a, b| a.total_cmp(b));
            (key, summary_f64(&values))
        })
        .collect::<serde_json::Map<_, _>>();
    acc.durations_ms.sort_unstable();
    acc.failed_durations_ms.sort_unstable();
    acc.terminal_durations_ms.sort_unstable();
    acc.item_counts.sort_unstable();
    json!({
        "started": acc.started,
        "succeeded": acc.succeeded,
        "failed": acc.failed,
        "duration_ms": summary_i64(&acc.durations_ms),
        "failed_duration_ms": summary_i64(&acc.failed_durations_ms),
        "worst_case_duration_ms": summary_i64(&acc.terminal_durations_ms),
        "item_count": summary_i64(&acc.item_counts),
        "numeric_metrics": metric_summaries,
    })
}

fn question_json(question_id: String, acc: QuestionAcc) -> Value {
    json!({
        "question_id": question_id,
        "events": acc.events,
        "trace_span": span_json(acc.first_ts, acc.last_ts),
        "operations": acc.operations.into_iter().map(|(operation, acc)| {
            let mut value = step_json(acc);
            value["operation"] = json!(operation);
            value
        }).collect::<Vec<_>>(),
        "summary": {
            "context_chars": acc.context_chars,
            "context_item_count": acc.context_item_count,
            "answer_chars": acc.answer_chars,
            "answer_ms": acc.answer_ms,
            "query_plan_ms": acc.query_plan_ms,
            "retrieval_query_count": acc.retrieval_query_count,
            "dense_query_count": acc.dense_query_count,
            "sparse_term_count": acc.sparse_term_count,
            "fact_searches": acc.fact_searches,
            "raw_searches": acc.raw_searches,
        }
    })
}

fn model_groups_json(groups: BTreeMap<String, ModelGroupAcc>) -> Value {
    Value::Array(
        groups
            .into_iter()
            .map(|(name, mut group)| {
                group.queue_durations_ms.sort_unstable();
                group.failed_queue_durations_ms.sort_unstable();
                group.terminal_queue_durations_ms.sort_unstable();
                group.timing_total_ms.sort_unstable();
                group.failed_timing_total_ms.sort_unstable();
                group.terminal_timing_total_ms.sort_unstable();
                json!({
                    "name": name,
                    "items": group.items,
                    "failed_items": group.failed_items,
                    "queue_duration_ms": summary_i64(&group.queue_durations_ms),
                    "failed_queue_duration_ms": summary_i64(&group.failed_queue_durations_ms),
                    "worst_case_queue_duration_ms": summary_i64(&group.terminal_queue_durations_ms),
                    "timing_total_ms": summary_i64(&group.timing_total_ms),
                    "failed_timing_total_ms": summary_i64(&group.failed_timing_total_ms),
                    "worst_case_timing_total_ms": summary_i64(&group.terminal_timing_total_ms),
                })
            })
            .collect(),
    )
}

fn read_jsonl(path: &Path) -> anyhow::Result<Vec<Value>> {
    let raw = std::fs::read_to_string(path)?;
    let mut out = Vec::new();
    for (idx, line) in raw.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        out.push(serde_json::from_str(line).map_err(|error| {
            anyhow::anyhow!(
                "invalid JSONL line {} in {}: {error}",
                idx + 1,
                path.display()
            )
        })?);
    }
    Ok(out)
}

fn summary_i64(values: &[i64]) -> Value {
    if values.is_empty() {
        return Value::Null;
    }
    let sum: i64 = values.iter().sum();
    json!({
        "count": values.len(),
        "sum": sum,
        "avg": sum as f64 / values.len() as f64,
        "p50": percentile_i64(values, 0.50),
        "p80": percentile_i64(values, 0.80),
        "p95": percentile_i64(values, 0.95),
        "p98": percentile_i64(values, 0.98),
        "max": values.last().copied(),
    })
}

fn summary_f64(values: &[f64]) -> Value {
    if values.is_empty() {
        return Value::Null;
    }
    let sum: f64 = values.iter().sum();
    json!({
        "count": values.len(),
        "sum": sum,
        "avg": sum / values.len() as f64,
        "p50": percentile_f64(values, 0.50),
        "p80": percentile_f64(values, 0.80),
        "p95": percentile_f64(values, 0.95),
        "p98": percentile_f64(values, 0.98),
        "max": values.last().copied(),
    })
}

fn percentile_i64(sorted: &[i64], pct: f64) -> Option<f64> {
    percentile_by(sorted.len(), pct, |idx| sorted[idx] as f64)
}

fn percentile_f64(sorted: &[f64], pct: f64) -> Option<f64> {
    percentile_by(sorted.len(), pct, |idx| sorted[idx])
}

fn percentile_by(len: usize, pct: f64, value_at: impl Fn(usize) -> f64) -> Option<f64> {
    if len == 0 {
        return None;
    }
    let rank = pct * (len as f64 - 1.0);
    let lo = rank.floor() as usize;
    let hi = rank.ceil() as usize;
    if lo == hi {
        return Some(value_at(lo));
    }
    let weight = rank - lo as f64;
    Some(value_at(lo) * (1.0 - weight) + value_at(hi) * weight)
}

fn update_span(
    first: &mut Option<DateTime<Utc>>,
    last: &mut Option<DateTime<Utc>>,
    ts: Option<DateTime<Utc>>,
) {
    let Some(ts) = ts else {
        return;
    };
    if first.is_none_or(|current| ts < current) {
        *first = Some(ts);
    }
    if last.is_none_or(|current| ts > current) {
        *last = Some(ts);
    }
}

fn span_json(first: Option<DateTime<Utc>>, last: Option<DateTime<Utc>>) -> Value {
    json!({
        "first_timestamp": first.map(|ts| ts.to_rfc3339()),
        "last_timestamp": last.map(|ts| ts.to_rfc3339()),
        "duration_ms": first.zip(last).map(|(first, last)| (last - first).num_milliseconds()),
    })
}

fn parse_ts(value: &Value) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value.as_str()?)
        .ok()
        .map(|ts| ts.with_timezone(&Utc))
}

fn model_id(row: &Value) -> Option<String> {
    let model = row.get("model")?;
    if let Some(text) = model.as_str() {
        return Some(text.to_string());
    }
    let operator = model.get("operator").and_then(Value::as_str).unwrap_or("");
    let operation = model.get("operation").and_then(Value::as_str).unwrap_or("");
    let name = model.get("model").and_then(Value::as_str).unwrap_or("");
    if operator.is_empty() && operation.is_empty() && name.is_empty() {
        None
    } else {
        Some(format!("{operator}:{operation}:{name}"))
    }
}

fn is_terminal_status(status: &str) -> bool {
    matches!(
        status,
        "succeeded" | "failed" | "error" | "cancelled" | "canceled"
    )
}

fn is_failed_terminal_status(status: &str) -> bool {
    matches!(status, "failed" | "error" | "cancelled" | "canceled")
}

fn stable_hash_value(value: &Value) -> Value {
    let bytes = serde_json::to_vec(value).unwrap_or_default();
    let digest = Sha256::digest(&bytes);
    json!(format!("{digest:x}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_memory_step_analytics() {
        let dir = tempfile::tempdir().unwrap();
        let artifacts = dir.path().join("artifacts");
        std::fs::create_dir_all(&artifacts).unwrap();
        std::fs::write(
            artifacts.join("memory-traces.jsonl"),
            concat!(
                "{\"timestamp\":\"2026-06-22T00:00:00Z\",\"question_id\":\"q1\",\"operation\":\"fact_search\",\"event\":\"operation_started\",\"metrics\":{}}\n",
                "{\"timestamp\":\"2026-06-22T00:00:01Z\",\"question_id\":\"q1\",\"operation\":\"fact_search\",\"event\":\"operation_succeeded\",\"duration_ms\":25,\"item_count\":20,\"metrics\":{\"best_score\":0.7,\"top_k\":20}}\n",
                "{\"timestamp\":\"2026-06-22T00:00:02Z\",\"question_id\":\"q1\",\"operation\":\"answer_context\",\"event\":\"operation_succeeded\",\"duration_ms\":0,\"item_count\":30,\"metrics\":{\"context_chars\":1234,\"context_item_count\":30}}\n",
            ),
        )
        .unwrap();

        let analytics = derive_run_step_analytics(dir.path()).unwrap().unwrap();
        assert_eq!(
            analytics["memory"]["operations"][0]["operation"],
            "answer_context"
        );
        assert_eq!(
            analytics["memory"]["questions"][0]["summary"]["context_chars"],
            1234
        );
        let fact_search = analytics["memory"]["operations"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["operation"] == "fact_search")
            .unwrap();
        assert_eq!(fact_search["duration_ms"]["p80"], 25.0);
        assert_eq!(fact_search["duration_ms"]["p98"], 25.0);
        assert_eq!(fact_search["numeric_metrics"]["best_score"]["p80"], 0.7);
        assert_eq!(fact_search["numeric_metrics"]["best_score"]["p98"], 0.7);
    }

    #[test]
    fn derives_model_queue_timing() {
        let dir = tempfile::tempdir().unwrap();
        let artifacts = dir.path().join("artifacts");
        std::fs::create_dir_all(&artifacts).unwrap();
        std::fs::write(
            artifacts.join("model-traces.jsonl"),
            concat!(
                "{\"timestamp\":\"2026-06-22T00:00:00Z\",\"queue_id\":\"chat:q\",\"item_id\":\"i1\",\"operation\":\"chat\",\"status\":\"queued\"}\n",
                "{\"timestamp\":\"2026-06-22T00:00:03Z\",\"queue_id\":\"chat:q\",\"item_id\":\"i1\",\"operation\":\"chat\",\"status\":\"succeeded\",\"role_binding\":\"bench.answer\",\"model\":{\"operator\":\"deepseek\",\"operation\":\"chat\",\"model\":\"deepseek-v4-flash\"},\"timing\":{\"total_ms\":2900}}\n",
                "{\"timestamp\":\"2026-06-22T00:00:00Z\",\"queue_id\":\"chat:q\",\"item_id\":\"i2\",\"operation\":\"chat\",\"status\":\"queued\"}\n",
                "{\"timestamp\":\"2026-06-22T00:00:09Z\",\"queue_id\":\"chat:q\",\"item_id\":\"i2\",\"operation\":\"chat\",\"status\":\"failed\",\"role_binding\":\"bench.answer\",\"model\":{\"operator\":\"deepseek\",\"operation\":\"chat\",\"model\":\"deepseek-v4-flash\"},\"timing\":{\"total_ms\":8900}}\n",
            ),
        )
        .unwrap();

        let analytics = derive_run_step_analytics(dir.path()).unwrap().unwrap();
        assert_eq!(analytics["model"]["items"]["terminal"], 2);
        assert_eq!(analytics["model"]["items"]["failed"], 1);
        assert_eq!(
            analytics["model"]["by_queue"][0]["queue_duration_ms"]["sum"],
            3000
        );
        assert_eq!(
            analytics["model"]["by_queue"][0]["failed_queue_duration_ms"]["sum"],
            9000
        );
        assert_eq!(
            analytics["model"]["by_queue"][0]["worst_case_queue_duration_ms"]["max"],
            9000
        );
    }
}
