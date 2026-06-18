//! Roll up `model-traces.jsonl` into cost, token, and latency summaries.
//!
//! Model traces are emitted per provider call with usage, timing, and the role
//! binding that issued the call (e.g. `bench.judge`, `bench.answer`). We derive:
//! - total cost (when the provider reported `cost_micro_usd`),
//! - token totals,
//! - latency percentiles over `timing.total_ms`,
//! - per-model and per-role breakdowns (the per-role map also tells us which
//!   concrete model served the judge/answer/distill/embed roles).
//!
//! Trace files for large runs can be tens of thousands of lines, so this is
//! computed lazily for run detail and persisted into the report at finalize
//! time — never on the bulk index path.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Clone, Debug, Deserialize)]
struct RawModelTrace {
    model: RawModelId,
    #[serde(default)]
    role_binding: Option<String>,
    #[serde(default)]
    cache: Option<RawCache>,
    #[serde(default)]
    usage: Option<RawUsage>,
    #[serde(default)]
    timing: Option<RawTiming>,
    #[serde(default)]
    outcome: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct RawModelId {
    #[serde(default)]
    operation: String,
    #[serde(default)]
    operator: String,
    #[serde(default)]
    model: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct RawCache {
    #[serde(default)]
    response_cache: Option<String>,
    #[serde(default)]
    prompt_cache: Option<String>,
    #[serde(default)]
    cached_input_tokens: Option<u64>,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct RawUsage {
    #[serde(default)]
    input_tokens: Option<u64>,
    #[serde(default)]
    output_tokens: Option<u64>,
    #[serde(default)]
    cost_micro_usd: Option<u64>,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct RawTiming {
    #[serde(default)]
    total_ms: Option<i64>,
}

/// Per-model usage breakdown.
#[derive(Clone, Debug, Default, Serialize)]
pub struct ModelStat {
    pub model: String,
    pub operator: String,
    pub operation: String,
    pub calls: u64,
    pub failed_calls: u64,
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub uncached_input_tokens: u64,
    pub output_tokens: u64,
    pub response_cache_hits: u64,
    pub prompt_cache_hits: u64,
    pub prompt_cache_partial_hits: u64,
    pub prompt_cache_misses: u64,
    pub cost_micro_usd: Option<u64>,
    pub latency_ms_p50: Option<f64>,
}

/// Per-role usage breakdown.
#[derive(Clone, Debug, Default, Serialize)]
pub struct RoleStat {
    pub role: String,
    pub models: Vec<String>,
    pub calls: u64,
    pub failed_calls: u64,
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub uncached_input_tokens: u64,
    pub output_tokens: u64,
    pub response_cache_hits: u64,
    pub prompt_cache_hits: u64,
    pub prompt_cache_partial_hits: u64,
    pub prompt_cache_misses: u64,
    pub cost_micro_usd: Option<u64>,
    pub latency_ms_p50: Option<f64>,
    pub latency_ms_p95: Option<f64>,
}

/// Aggregate rollup over all model calls in a run.
#[derive(Clone, Debug, Default, Serialize)]
pub struct ModelTraceRollup {
    pub calls: u64,
    pub failed_calls: u64,
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub uncached_input_tokens: u64,
    pub output_tokens: u64,
    pub response_cache_hits: u64,
    pub prompt_cache_hits: u64,
    pub prompt_cache_partial_hits: u64,
    pub prompt_cache_misses: u64,
    /// Total cost when at least one call reported a price; `None` otherwise.
    pub cost_micro_usd: Option<u64>,
    pub latency_ms_p50: Option<f64>,
    pub latency_ms_p95: Option<f64>,
    pub models: Vec<ModelStat>,
    pub roles_detail: Vec<RoleStat>,
    /// role_binding -> concrete model id (e.g. `bench.judge` -> `deepseek-v4-flash`).
    pub roles: BTreeMap<String, String>,
}

#[derive(Default)]
struct ModelAcc {
    operator: String,
    operation: String,
    calls: u64,
    failed_calls: u64,
    input_tokens: u64,
    cached_input_tokens: u64,
    uncached_input_tokens: u64,
    output_tokens: u64,
    response_cache_hits: u64,
    prompt_cache_hits: u64,
    prompt_cache_partial_hits: u64,
    prompt_cache_misses: u64,
    cost_micro_usd: u64,
    saw_cost: bool,
    latencies: Vec<i64>,
}

#[derive(Default)]
struct RoleAcc {
    models: BTreeMap<String, ()>,
    calls: u64,
    failed_calls: u64,
    input_tokens: u64,
    cached_input_tokens: u64,
    uncached_input_tokens: u64,
    output_tokens: u64,
    response_cache_hits: u64,
    prompt_cache_hits: u64,
    prompt_cache_partial_hits: u64,
    prompt_cache_misses: u64,
    cost_micro_usd: u64,
    saw_cost: bool,
    latencies: Vec<i64>,
}

fn percentile(sorted: &[i64], pct: f64) -> Option<f64> {
    if sorted.is_empty() {
        return None;
    }
    let rank = pct / 100.0 * (sorted.len() as f64 - 1.0);
    let lo = rank.floor() as usize;
    let hi = rank.ceil() as usize;
    if lo == hi {
        return Some(sorted[lo] as f64);
    }
    let weight = rank - lo as f64;
    Some(sorted[lo] as f64 * (1.0 - weight) + sorted[hi] as f64 * weight)
}

/// Compute the rollup for a run, or `None` when `model-traces.jsonl` is absent
/// or empty.
pub fn rollup_model_traces(run_root: &Path) -> Option<ModelTraceRollup> {
    let path = run_root.join("artifacts").join("model-traces.jsonl");
    rollup_model_trace_file(&path)
}

/// Compute the rollup for a specific model trace file.
pub fn rollup_model_trace_file(path: &Path) -> Option<ModelTraceRollup> {
    let raw = std::fs::read_to_string(path).ok()?;

    let mut rollup = ModelTraceRollup::default();
    let mut per_model: BTreeMap<String, ModelAcc> = BTreeMap::new();
    let mut per_role: BTreeMap<String, RoleAcc> = BTreeMap::new();
    let mut all_latencies: Vec<i64> = Vec::new();
    let mut total_cost = 0u64;
    let mut saw_any = false;

    for line in raw.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let Ok(trace) = serde_json::from_str::<RawModelTrace>(line) else {
            continue;
        };
        saw_any = true;
        rollup.calls += 1;
        let failed = trace
            .outcome
            .as_deref()
            .is_some_and(|outcome| outcome != "succeeded");
        if failed {
            rollup.failed_calls += 1;
        }

        let usage = trace.usage.unwrap_or_default();
        let cache = trace.cache.unwrap_or_default();
        let input = usage.input_tokens.unwrap_or(0);
        let cached = cache.cached_input_tokens.unwrap_or(0).min(input);
        let uncached = input.saturating_sub(cached);
        let output = usage.output_tokens.unwrap_or(0);
        rollup.input_tokens += input;
        rollup.cached_input_tokens += cached;
        rollup.uncached_input_tokens += uncached;
        rollup.output_tokens += output;
        if cache.response_cache.as_deref() == Some("hit") {
            rollup.response_cache_hits += 1;
        }
        match cache.prompt_cache.as_deref() {
            Some("hit") => rollup.prompt_cache_hits += 1,
            Some("partial_hit") => rollup.prompt_cache_partial_hits += 1,
            Some("miss") => rollup.prompt_cache_misses += 1,
            _ => {}
        }
        if let Some(cost) = usage.cost_micro_usd {
            total_cost += cost;
            rollup.cost_micro_usd = Some(total_cost);
        }

        let latency = trace.timing.and_then(|timing| timing.total_ms);
        if let Some(latency) = latency {
            all_latencies.push(latency);
        }

        let acc = per_model.entry(trace.model.model.clone()).or_default();
        acc.operator = trace.model.operator.clone();
        acc.operation = trace.model.operation.clone();
        acc.calls += 1;
        if failed {
            acc.failed_calls += 1;
        }
        acc.input_tokens += input;
        acc.cached_input_tokens += cached;
        acc.uncached_input_tokens += uncached;
        acc.output_tokens += output;
        if cache.response_cache.as_deref() == Some("hit") {
            acc.response_cache_hits += 1;
        }
        match cache.prompt_cache.as_deref() {
            Some("hit") => acc.prompt_cache_hits += 1,
            Some("partial_hit") => acc.prompt_cache_partial_hits += 1,
            Some("miss") => acc.prompt_cache_misses += 1,
            _ => {}
        }
        if let Some(cost) = usage.cost_micro_usd {
            acc.cost_micro_usd += cost;
            acc.saw_cost = true;
        }
        if let Some(latency) = latency {
            acc.latencies.push(latency);
        }

        if let Some(role) = trace.role_binding {
            rollup
                .roles
                .entry(role.clone())
                .or_insert_with(|| trace.model.model.clone());
            let role_acc = per_role.entry(role).or_default();
            role_acc.models.insert(trace.model.model.clone(), ());
            role_acc.calls += 1;
            if failed {
                role_acc.failed_calls += 1;
            }
            role_acc.input_tokens += input;
            role_acc.cached_input_tokens += cached;
            role_acc.uncached_input_tokens += uncached;
            role_acc.output_tokens += output;
            if cache.response_cache.as_deref() == Some("hit") {
                role_acc.response_cache_hits += 1;
            }
            match cache.prompt_cache.as_deref() {
                Some("hit") => role_acc.prompt_cache_hits += 1,
                Some("partial_hit") => role_acc.prompt_cache_partial_hits += 1,
                Some("miss") => role_acc.prompt_cache_misses += 1,
                _ => {}
            }
            if let Some(cost) = usage.cost_micro_usd {
                role_acc.cost_micro_usd += cost;
                role_acc.saw_cost = true;
            }
            if let Some(latency) = latency {
                role_acc.latencies.push(latency);
            }
        }
    }

    if !saw_any {
        return None;
    }

    all_latencies.sort_unstable();
    rollup.latency_ms_p50 = percentile(&all_latencies, 50.0);
    rollup.latency_ms_p95 = percentile(&all_latencies, 95.0);

    rollup.models = per_model
        .into_iter()
        .map(|(model, mut acc)| {
            acc.latencies.sort_unstable();
            ModelStat {
                model,
                operator: acc.operator,
                operation: acc.operation,
                calls: acc.calls,
                failed_calls: acc.failed_calls,
                input_tokens: acc.input_tokens,
                cached_input_tokens: acc.cached_input_tokens,
                uncached_input_tokens: acc.uncached_input_tokens,
                output_tokens: acc.output_tokens,
                response_cache_hits: acc.response_cache_hits,
                prompt_cache_hits: acc.prompt_cache_hits,
                prompt_cache_partial_hits: acc.prompt_cache_partial_hits,
                prompt_cache_misses: acc.prompt_cache_misses,
                cost_micro_usd: acc.saw_cost.then_some(acc.cost_micro_usd),
                latency_ms_p50: percentile(&acc.latencies, 50.0),
            }
        })
        .collect();
    rollup
        .models
        .sort_by(|left, right| right.calls.cmp(&left.calls));

    rollup.roles_detail = per_role
        .into_iter()
        .map(|(role, mut acc)| {
            acc.latencies.sort_unstable();
            RoleStat {
                role,
                models: acc.models.into_keys().collect(),
                calls: acc.calls,
                failed_calls: acc.failed_calls,
                input_tokens: acc.input_tokens,
                cached_input_tokens: acc.cached_input_tokens,
                uncached_input_tokens: acc.uncached_input_tokens,
                output_tokens: acc.output_tokens,
                response_cache_hits: acc.response_cache_hits,
                prompt_cache_hits: acc.prompt_cache_hits,
                prompt_cache_partial_hits: acc.prompt_cache_partial_hits,
                prompt_cache_misses: acc.prompt_cache_misses,
                cost_micro_usd: acc.saw_cost.then_some(acc.cost_micro_usd),
                latency_ms_p50: percentile(&acc.latencies, 50.0),
                latency_ms_p95: percentile(&acc.latencies, 95.0),
            }
        })
        .collect();
    rollup
        .roles_detail
        .sort_by(|left, right| right.calls.cmp(&left.calls));

    Some(rollup)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rolls_up_calls_tokens_and_latency() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("artifacts").join("model-traces.jsonl");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            &path,
            "{\"model\":{\"operation\":\"chat\",\"operator\":\"deepseek\",\"model\":\"deepseek-v4-flash\"},\"role_binding\":\"bench.judge\",\"cache\":{\"response_cache\":\"miss\",\"prompt_cache\":\"partial_hit\",\"cached_input_tokens\":128},\"usage\":{\"input_tokens\":170,\"output_tokens\":1},\"timing\":{\"total_ms\":1441},\"outcome\":\"succeeded\"}\n{\"model\":{\"operation\":\"chat\",\"operator\":\"deepseek\",\"model\":\"deepseek-v4-flash\"},\"role_binding\":\"bench.judge\",\"cache\":{\"response_cache\":\"hit\",\"prompt_cache\":\"hit\",\"cached_input_tokens\":100},\"usage\":{\"input_tokens\":100,\"output_tokens\":2},\"timing\":{\"total_ms\":1000},\"outcome\":\"succeeded\"}\n",
        )
        .unwrap();

        let rollup = rollup_model_traces(dir.path()).unwrap();
        assert_eq!(rollup.calls, 2);
        assert_eq!(rollup.input_tokens, 270);
        assert_eq!(rollup.cached_input_tokens, 228);
        assert_eq!(rollup.uncached_input_tokens, 42);
        assert_eq!(rollup.output_tokens, 3);
        assert_eq!(rollup.response_cache_hits, 1);
        assert_eq!(rollup.prompt_cache_hits, 1);
        assert_eq!(rollup.prompt_cache_partial_hits, 1);
        assert_eq!(rollup.prompt_cache_misses, 0);
        assert_eq!(rollup.cost_micro_usd, None);
        assert_eq!(
            rollup.roles.get("bench.judge").map(String::as_str),
            Some("deepseek-v4-flash")
        );
        assert_eq!(rollup.latency_ms_p50, Some(1220.5));
        assert_eq!(rollup.models.len(), 1);
        assert_eq!(rollup.roles_detail.len(), 1);
        assert_eq!(rollup.roles_detail[0].role, "bench.judge");
        assert_eq!(rollup.roles_detail[0].cached_input_tokens, 228);
    }

    #[test]
    fn absent_traces_yield_none() {
        let dir = tempfile::tempdir().unwrap();
        assert!(rollup_model_traces(dir.path()).is_none());
    }
}
