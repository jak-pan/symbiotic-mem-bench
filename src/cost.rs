//! Roll up `model-traces.jsonl` into cost, token, and latency summaries.
//!
//! Model traces are emitted per provider call with usage, timing, and the role
//! binding that issued the call (e.g. `bench.judge`, `bench.answer`). We derive:
//! - total cost (reported by the provider, or estimated from the built-in
//!   pricing catalog when token buckets are present),
//! - token totals,
//! - latency percentiles over `timing.total_ms`,
//! - per-model and per-role breakdowns (the per-role map also tells us which
//!   concrete model served the judge/answer/distill/embed roles).
//!
//! Trace files for large runs can be tens of thousands of lines, so this is
//! computed lazily for run detail and persisted into the report at finalize
//! time — never on the bulk index path.

use chrono::{DateTime, Datelike, Timelike, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

pub const PRICING_TABLE_VERSION: &str = "official-pricing-2026-09-17";

#[derive(Clone, Debug, Deserialize)]
struct RawModelTrace {
    #[serde(default)]
    timestamp: Option<String>,
    #[serde(default)]
    queue_id: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
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
    #[serde(default)]
    cost_micro_usd: Option<u64>,
    #[serde(default)]
    input_units: Option<u64>,
}

#[derive(Clone, Debug, Default, Deserialize)]
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
    cached_input_tokens: Option<u64>,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct RawUsage {
    #[serde(default, alias = "prompt_tokens")]
    input_tokens: Option<u64>,
    #[serde(default, alias = "completion_tokens")]
    output_tokens: Option<u64>,
    #[serde(default)]
    cache_hit_tokens: Option<u64>,
    #[serde(default)]
    cache_miss_tokens: Option<u64>,
    #[serde(default)]
    cost_micro_usd: Option<u64>,
    #[serde(default)]
    provider: Option<RawProviderUsage>,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct RawProviderUsage {
    #[serde(default)]
    reported_cost_usd: Option<String>,
}

impl RawProviderUsage {
    fn reported_cost_micro_usd(&self) -> Option<u64> {
        let dollars = self
            .reported_cost_usd
            .as_deref()?
            .trim()
            .parse::<f64>()
            .ok()?;
        let micro_usd = (dollars * 1_000_000.0).round();
        // The floating representation of u64::MAX rounds to 2^64, so the upper
        // bound must be exclusive; casting first would silently saturate.
        (dollars.is_finite() && dollars >= 0.0 && micro_usd < u64::MAX as f64)
            .then_some(micro_usd as u64)
    }
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
    /// Input tokens without a trustworthy provider cache split.
    pub unknown_cache_input_tokens: u64,
    /// Calls for which neither reported cost nor a complete estimate is available.
    pub unpriced_calls: u64,
    pub output_tokens: u64,
    pub response_cache_hits: u64,
    pub prompt_cache_hits: u64,
    pub prompt_cache_partial_hits: u64,
    pub prompt_cache_misses: u64,
    pub cost_micro_usd: Option<u64>,
    pub cost_estimated: bool,
    pub pricing_source: Option<String>,
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
    /// Input tokens without a trustworthy provider cache split.
    pub unknown_cache_input_tokens: u64,
    /// Calls for which neither reported cost nor a complete estimate is available.
    pub unpriced_calls: u64,
    pub output_tokens: u64,
    pub response_cache_hits: u64,
    pub prompt_cache_hits: u64,
    pub prompt_cache_partial_hits: u64,
    pub prompt_cache_misses: u64,
    pub cost_micro_usd: Option<u64>,
    pub cost_estimated: bool,
    pub pricing_source: Option<String>,
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
    /// Input tokens without a trustworthy provider cache split.
    pub unknown_cache_input_tokens: u64,
    /// Calls for which neither reported cost nor a complete estimate is available.
    pub unpriced_calls: u64,
    pub output_tokens: u64,
    pub response_cache_hits: u64,
    pub prompt_cache_hits: u64,
    pub prompt_cache_partial_hits: u64,
    pub prompt_cache_misses: u64,
    /// Total new cost only when every call is priced; `None` if any call is unpriced.
    pub cost_micro_usd: Option<u64>,
    pub cost_estimated: bool,
    pub pricing_table_version: Option<String>,
    pub pricing_sources: Vec<String>,
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
    unknown_cache_input_tokens: u64,
    unpriced_calls: u64,
    output_tokens: u64,
    response_cache_hits: u64,
    prompt_cache_hits: u64,
    prompt_cache_partial_hits: u64,
    prompt_cache_misses: u64,
    cost_micro_usd: u64,
    saw_cost: bool,
    estimated_cost: bool,
    pricing_sources: BTreeMap<String, ()>,
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
    unknown_cache_input_tokens: u64,
    unpriced_calls: u64,
    output_tokens: u64,
    response_cache_hits: u64,
    prompt_cache_hits: u64,
    prompt_cache_partial_hits: u64,
    prompt_cache_misses: u64,
    cost_micro_usd: u64,
    saw_cost: bool,
    estimated_cost: bool,
    pricing_sources: BTreeMap<String, ()>,
    latencies: Vec<i64>,
}

#[derive(Clone, Copy)]
struct Pricing {
    input_per_million_usd: Option<f64>,
    cached_input_per_million_usd: Option<f64>,
    output_per_million_usd: Option<f64>,
    source: &'static str,
}

const PRICING_CATALOG_SOURCE: &str =
    "OpenRouter /models catalog (config/pricing/openrouter-pricing.json)";

/// Lazily-loaded OpenRouter pricing catalog: model id -> (input_per_M_usd, output_per_M_usd).
/// Sourced from `config/pricing/openrouter-pricing.json`, a snapshot of OpenRouter's `/models`
/// pricing refreshed by `scripts/refresh-pricing.sh`. Path overridable via `MEMBENCH_PRICING_CACHE`.
/// Missing/unparsable file -> empty catalog (callers fall back to the static table).
fn openrouter_pricing_catalog() -> &'static std::collections::HashMap<String, (f64, f64)> {
    static CATALOG: std::sync::OnceLock<std::collections::HashMap<String, (f64, f64)>> =
        std::sync::OnceLock::new();
    CATALOG.get_or_init(|| load_pricing_catalog().unwrap_or_default())
}

fn pricing_catalog_path() -> std::path::PathBuf {
    match std::env::var("MEMBENCH_PRICING_CACHE") {
        Ok(p) if !p.trim().is_empty() => std::path::PathBuf::from(p),
        _ => std::path::PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/config/pricing/openrouter-pricing.json"
        )),
    }
}

fn load_pricing_catalog() -> Option<std::collections::HashMap<String, (f64, f64)>> {
    #[derive(serde::Deserialize)]
    struct CatalogEntry {
        input_per_million_usd: Option<f64>,
        output_per_million_usd: Option<f64>,
    }
    #[derive(serde::Deserialize)]
    struct CatalogFile {
        models: std::collections::HashMap<String, CatalogEntry>,
    }
    let raw = std::fs::read_to_string(pricing_catalog_path()).ok()?;
    let parsed: CatalogFile = serde_json::from_str(&raw).ok()?;
    Some(
        parsed
            .models
            .into_iter()
            .map(|(id, e)| {
                (
                    id,
                    (
                        e.input_per_million_usd.unwrap_or(0.0),
                        e.output_per_million_usd.unwrap_or(0.0),
                    ),
                )
            })
            .collect(),
    )
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

// Exact transitions published by DeepSeek; the intervening August tariff is deliberately
// unpriced until a historical USD snapshot is available. Never apply current rates to June runs.
// https://api-docs.deepseek.com/news/news260813/
// https://api-docs.deepseek.com/news/news260910/
// Earliest dated snapshot we retain; this is not a claimed model-launch timestamp.
const DEEPSEEK_HISTORICAL_SNAPSHOT_START: i64 = 1_782_172_800; // 2026-06-23 00:00 UTC
const DEEPSEEK_AUGUST_TRANSITION: i64 = 1_786_896_000; // 2026-08-16 16:00 UTC
const DEEPSEEK_FLASH_TRANSITION: i64 = 1_789_012_800; // 2026-09-10 04:00 UTC

fn deepseek_pricing(model: &str, timestamp: Option<&str>) -> Option<Pricing> {
    let at = DateTime::parse_from_rfc3339(timestamp?)
        .ok()?
        .with_timezone(&Utc);
    if at.timestamp() >= DEEPSEEK_FLASH_TRANSITION
        && matches!(
            model,
            "deepseek-flash" | "deepseek-v4-flash" | "deepseek-v4-flash-vision-exp"
        )
    {
        let peak = at.weekday().num_days_from_monday() < 5
            && ((1..4).contains(&at.hour()) || (6..10).contains(&at.hour()));
        return Some(Pricing {
            input_per_million_usd: Some(if peak { 0.30 } else { 0.15 }),
            cached_input_per_million_usd: Some(if peak { 0.006 } else { 0.003 }),
            output_per_million_usd: Some(if peak { 1.20 } else { 0.60 }),
            source: if peak {
                "DeepSeek Flash 2026-09-10 peak UTC: https://api-docs.deepseek.com/quick_start/pricing/"
            } else {
                "DeepSeek Flash 2026-09-10 off-peak UTC: https://api-docs.deepseek.com/quick_start/pricing/"
            },
        });
    }
    if !(DEEPSEEK_HISTORICAL_SNAPSHOT_START..DEEPSEEK_AUGUST_TRANSITION).contains(&at.timestamp()) {
        return None;
    }
    let (input, cached, output) = match model {
        "deepseek-v4-flash" => (0.14, 0.0028, 0.28),
        "deepseek-v4-pro" => (0.435, 0.003625, 0.87),
        _ => return None,
    };
    Some(Pricing {
        input_per_million_usd: Some(input),
        cached_input_per_million_usd: Some(cached),
        output_per_million_usd: Some(output),
        source: "DeepSeek historical 2026-06-23 snapshot: https://api-docs.deepseek.com/quick_start/pricing",
    })
}

fn pricing_for(
    operator: &str,
    operation: &str,
    model: &str,
    timestamp: Option<&str>,
) -> Option<Pricing> {
    let operator = operator.trim();
    let operation = operation.trim();
    let model = model.trim();
    // Static table: native-API pricing (DeepSeek/Gemini) + explicit overrides, version-controlled
    // here. OpenRouter-routed models fall through to the cached `/models` catalog below.
    let static_hit = match (operator, operation, model) {
        ("deepseek", "chat", model) => deepseek_pricing(model, timestamp),
        ("gemini", "embedding", "gemini-embedding-2") => Some(Pricing {
            input_per_million_usd: Some(0.20),
            cached_input_per_million_usd: None,
            output_per_million_usd: None,
            source: "Gemini API pricing: https://ai.google.dev/gemini-api/docs/pricing",
        }),
        ("gemini", "embedding", "gemini-embedding-2-batch") => Some(Pricing {
            input_per_million_usd: Some(0.10),
            cached_input_per_million_usd: None,
            output_per_million_usd: None,
            source: "Gemini API pricing batch: https://ai.google.dev/gemini-api/docs/pricing",
        }),
        ("openrouter", "embedding", "qwen/qwen3-embedding-8b") => Some(Pricing {
            input_per_million_usd: Some(0.01),
            cached_input_per_million_usd: None,
            output_per_million_usd: None,
            source: "OpenRouter model pricing: https://openrouter.ai/qwen/qwen3-embedding-8b",
        }),
        ("openrouter", "embedding", "qwen/qwen3-embedding-4b") => Some(Pricing {
            input_per_million_usd: Some(0.01),
            cached_input_per_million_usd: None,
            output_per_million_usd: None,
            source: "OpenRouter model pricing: https://openrouter.ai/qwen/qwen3-embedding-4b",
        }),
        // Embeddings are not listed in OpenRouter's /models catalog, so they are priced here.
        ("openrouter", "embedding", "openai/text-embedding-3-small") => Some(Pricing {
            input_per_million_usd: Some(0.02),
            cached_input_per_million_usd: None,
            output_per_million_usd: None,
            source: "OpenAI embedding pricing: https://platform.openai.com/docs/pricing",
        }),
        _ => None,
    };
    if static_hit.is_some() {
        return static_hit;
    }
    // OpenRouter-routed models: price from the cached OpenRouter /models catalog
    // (config/pricing/openrouter-pricing.json, refreshed by `scripts/refresh-pricing.sh`).
    if operator == "openrouter"
        && let Some(&(input, output)) = openrouter_pricing_catalog().get(model)
    {
        return Some(Pricing {
            input_per_million_usd: Some(input),
            cached_input_per_million_usd: None,
            output_per_million_usd: Some(output),
            source: PRICING_CATALOG_SOURCE,
        });
    }
    None
}

/// Per-search price for a rerank operation, in micro-USD. OpenRouter rerankers are billed per
/// SEARCH (one rerank query = one search) regardless of document count — NOT per token or per
/// document. Source: https://openrouter.ai/cohere/rerank-4-fast = $0.002/search.
fn rerank_search_price_micro_usd(operator: &str, model: &str) -> Option<u64> {
    match (operator, model) {
        ("openrouter", "cohere/rerank-4-fast") => Some(2000), // $0.002/search
        _ => None,
    }
}

fn estimate_cost_micro_usd(
    pricing: Pricing,
    input_tokens: u64,
    cached_input_tokens: u64,
    output_tokens: u64,
) -> Option<u64> {
    let uncached_input_tokens = input_tokens.saturating_sub(cached_input_tokens);
    let mut usd = 0.0;
    let mut observed_price = false;
    if let Some(price) = pricing.input_per_million_usd {
        usd += (uncached_input_tokens as f64 / 1_000_000.0) * price;
        observed_price = true;
    }
    if let Some(price) = pricing
        .cached_input_per_million_usd
        .or(pricing.input_per_million_usd)
    {
        usd += (cached_input_tokens as f64 / 1_000_000.0) * price;
        observed_price = true;
    }
    if let Some(price) = pricing.output_per_million_usd {
        usd += (output_tokens as f64 / 1_000_000.0) * price;
        observed_price = true;
    }
    observed_price.then_some((usd * 1_000_000.0).round() as u64)
}

// Missing or contradictory counters are unknown, never automatically a cache miss.
// Normalized prompt_cache labels are not evidence: older emitters synthesize "miss"
// from absent counters. Only numeric provider counters establish the split.
fn cache_split(input: u64, usage: &RawUsage, cache: &RawCache) -> Option<(u64, u64)> {
    if let (Some(saved), Some(reported)) = (cache.cached_input_tokens, usage.cache_hit_tokens)
        && saved != reported
    {
        return None;
    }
    let hit = cache.cached_input_tokens.or(usage.cache_hit_tokens);
    let miss = usage.cache_miss_tokens;
    match (hit, miss) {
        (Some(hit), Some(miss)) if hit.checked_add(miss) == Some(input) => Some((hit, miss)),
        (Some(hit), None) if hit <= input => Some((hit, input - hit)),
        (None, Some(miss)) if miss <= input => Some((input - miss, miss)),
        _ => None,
    }
}

/// Compute the rollup for a run, or `None` when `model-traces.jsonl` is absent
/// or empty.
pub fn rollup_model_traces(run_root: &Path) -> Option<ModelTraceRollup> {
    [
        run_root.join("artifacts").join("model-traces.jsonl"),
        run_root
            .join("provider-queue")
            .join("model-queue-traces.jsonl"),
        run_root.join("raw").join("model-traces.jsonl"),
        run_root.join("model-traces.jsonl"),
    ]
    .into_iter()
    .find_map(|path| rollup_model_trace_file(&path))
}

/// Compute the rollup for a specific model trace file.
pub fn rollup_model_trace_file(path: &Path) -> Option<ModelTraceRollup> {
    let raw = std::fs::read_to_string(path).ok()?;

    let mut rollup = ModelTraceRollup::default();
    let mut per_model: BTreeMap<String, ModelAcc> = BTreeMap::new();
    let mut per_role: BTreeMap<String, RoleAcc> = BTreeMap::new();
    let mut all_latencies: Vec<i64> = Vec::new();
    let mut total_cost = 0u64;
    let mut estimated_any_cost = false;
    let mut pricing_sources: BTreeMap<String, ()> = BTreeMap::new();
    let mut saw_any = false;

    for line in raw.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let Ok(trace) = serde_json::from_str::<RawModelTrace>(line) else {
            continue;
        };
        if trace.queue_id.is_some()
            && matches!(
                trace.status.as_deref(),
                Some("queued" | "pending" | "running")
            )
        {
            continue;
        }
        saw_any = true;
        rollup.calls += 1;
        let failed = trace
            .outcome
            .as_deref()
            .or(trace.status.as_deref())
            .is_some_and(|outcome| outcome != "succeeded");
        if failed {
            rollup.failed_calls += 1;
        }

        let (operation, operator, model) = model_identity(&trace);
        let usage_input_fallback = (operation == "embedding" || operation == "rerank")
            .then(|| {
                trace
                    .usage
                    .as_ref()
                    .and_then(|usage| usage.input_tokens)
                    .is_none()
                    .then_some(trace.input_units)
                    .flatten()
            })
            .flatten();
        let usage = trace.usage.unwrap_or_default();
        let cache = trace.cache.unwrap_or_default();
        let observed_input = usage.input_tokens.or(usage_input_fallback);
        let input = observed_input.unwrap_or(0);
        let output = usage.output_tokens.unwrap_or(0);
        let split = observed_input.and_then(|input| cache_split(input, &usage, &cache));
        let (cached, uncached) = split.unwrap_or((0, 0));
        let unknown_cached = if split.is_none() { input } else { 0 };
        rollup.input_tokens += input;
        rollup.cached_input_tokens += cached;
        rollup.uncached_input_tokens += uncached;
        rollup.unknown_cache_input_tokens += unknown_cached;
        rollup.output_tokens += output;
        let response_replayed = cache.response_cache.as_deref() == Some("hit");
        if response_replayed {
            rollup.response_cache_hits += 1;
        }
        let prompt_cache = split.map(|(hit, miss)| match (hit, miss) {
            (0, _) => "miss",
            (_, 0) => "hit",
            _ => "partial_hit",
        });
        match prompt_cache {
            Some("hit") => rollup.prompt_cache_hits += 1,
            Some("partial_hit") => rollup.prompt_cache_partial_hits += 1,
            Some("miss") => rollup.prompt_cache_misses += 1,
            _ => {}
        }
        // A local replay carries original usage (including old provider-reported cost).
        // It makes no provider request, so it contributes zero new cost.
        let reported_cost = response_replayed
            .then_some(0)
            .or(usage.cost_micro_usd)
            .or_else(|| usage.provider.as_ref()?.reported_cost_micro_usd())
            // Queue-level prices are estimates from the producer's configured tariff,
            // not provider receipts; recalculate them using this dated catalog.
            .or(trace.cost_micro_usd.filter(|_| trace.queue_id.is_none()));
        let pricing = pricing_for(&operator, &operation, &model, trace.timestamp.as_deref());
        let estimated_cost = reported_cost.or_else(|| {
            if operation == "rerank" {
                // One query is one search, independent of document/token count.
                rerank_search_price_micro_usd(&operator, &model)
            } else {
                let pricing = pricing?;
                observed_input?;
                if pricing.output_per_million_usd.is_some() {
                    usage.output_tokens?;
                }
                if pricing.cached_input_per_million_usd.is_some() {
                    split?;
                }
                estimate_cost_micro_usd(pricing, input, cached, output)
            }
        });
        let unpriced = u64::from(estimated_cost.is_none());
        rollup.unpriced_calls += unpriced;
        let cost_was_estimated = reported_cost.is_none() && estimated_cost.is_some();
        if let Some(cost) = estimated_cost {
            total_cost += cost;
            rollup.cost_micro_usd = Some(total_cost);
            if cost_was_estimated {
                estimated_any_cost = true;
            }
            if cost_was_estimated && let Some(pricing) = pricing {
                pricing_sources.insert(pricing.source.to_string(), ());
            }
        }

        let latency = trace.timing.as_ref().and_then(|timing| timing.total_ms);
        if let Some(latency) = latency {
            all_latencies.push(latency);
        }

        let acc = per_model.entry(model.clone()).or_default();
        acc.operator = operator.clone();
        acc.operation = operation.clone();
        acc.calls += 1;
        if failed {
            acc.failed_calls += 1;
        }
        acc.input_tokens += input;
        acc.cached_input_tokens += cached;
        acc.uncached_input_tokens += uncached;
        acc.unknown_cache_input_tokens += unknown_cached;
        acc.unpriced_calls += unpriced;
        acc.output_tokens += output;
        if cache.response_cache.as_deref() == Some("hit") {
            acc.response_cache_hits += 1;
        }
        match prompt_cache {
            Some("hit") => acc.prompt_cache_hits += 1,
            Some("partial_hit") => acc.prompt_cache_partial_hits += 1,
            Some("miss") => acc.prompt_cache_misses += 1,
            _ => {}
        }
        if let Some(cost) = estimated_cost {
            acc.cost_micro_usd += cost;
            acc.saw_cost = true;
            if cost_was_estimated {
                acc.estimated_cost = true;
                if let Some(pricing) = pricing {
                    acc.pricing_sources.insert(pricing.source.to_string(), ());
                }
            }
        }
        if let Some(latency) = latency {
            acc.latencies.push(latency);
        }

        if let Some(role) = trace.role_binding {
            rollup
                .roles
                .entry(role.clone())
                .or_insert_with(|| model.clone());
            let role_acc = per_role.entry(role).or_default();
            role_acc.models.insert(model.clone(), ());
            role_acc.calls += 1;
            if failed {
                role_acc.failed_calls += 1;
            }
            role_acc.input_tokens += input;
            role_acc.cached_input_tokens += cached;
            role_acc.uncached_input_tokens += uncached;
            role_acc.unknown_cache_input_tokens += unknown_cached;
            role_acc.unpriced_calls += unpriced;
            role_acc.output_tokens += output;
            if cache.response_cache.as_deref() == Some("hit") {
                role_acc.response_cache_hits += 1;
            }
            match prompt_cache {
                Some("hit") => role_acc.prompt_cache_hits += 1,
                Some("partial_hit") => role_acc.prompt_cache_partial_hits += 1,
                Some("miss") => role_acc.prompt_cache_misses += 1,
                _ => {}
            }
            if let Some(cost) = estimated_cost {
                role_acc.cost_micro_usd += cost;
                role_acc.saw_cost = true;
                if cost_was_estimated {
                    role_acc.estimated_cost = true;
                    if let Some(pricing) = pricing {
                        role_acc
                            .pricing_sources
                            .insert(pricing.source.to_string(), ());
                    }
                }
            }
            if let Some(latency) = latency {
                role_acc.latencies.push(latency);
            }
        }
    }

    if !saw_any {
        return None;
    }

    if rollup.unpriced_calls > 0 {
        rollup.cost_micro_usd = None;
    }
    all_latencies.sort_unstable();
    rollup.latency_ms_p50 = percentile(&all_latencies, 50.0);
    rollup.latency_ms_p95 = percentile(&all_latencies, 95.0);
    rollup.cost_estimated = estimated_any_cost;
    rollup.pricing_sources = pricing_sources.into_keys().collect();
    if !rollup.pricing_sources.is_empty() {
        rollup.pricing_table_version = Some(PRICING_TABLE_VERSION.to_string());
    }

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
                unknown_cache_input_tokens: acc.unknown_cache_input_tokens,
                unpriced_calls: acc.unpriced_calls,
                output_tokens: acc.output_tokens,
                response_cache_hits: acc.response_cache_hits,
                prompt_cache_hits: acc.prompt_cache_hits,
                prompt_cache_partial_hits: acc.prompt_cache_partial_hits,
                prompt_cache_misses: acc.prompt_cache_misses,
                cost_micro_usd: (acc.saw_cost && acc.unpriced_calls == 0)
                    .then_some(acc.cost_micro_usd),
                cost_estimated: acc.estimated_cost,
                pricing_source: (!acc.pricing_sources.is_empty()).then(|| {
                    acc.pricing_sources
                        .into_keys()
                        .collect::<Vec<_>>()
                        .join("; ")
                }),
                latency_ms_p50: percentile(&acc.latencies, 50.0),
            }
        })
        .collect();
    rollup
        .models
        .sort_by_key(|stat| std::cmp::Reverse(stat.calls));

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
                unknown_cache_input_tokens: acc.unknown_cache_input_tokens,
                unpriced_calls: acc.unpriced_calls,
                output_tokens: acc.output_tokens,
                response_cache_hits: acc.response_cache_hits,
                prompt_cache_hits: acc.prompt_cache_hits,
                prompt_cache_partial_hits: acc.prompt_cache_partial_hits,
                prompt_cache_misses: acc.prompt_cache_misses,
                cost_micro_usd: (acc.saw_cost && acc.unpriced_calls == 0)
                    .then_some(acc.cost_micro_usd),
                cost_estimated: acc.estimated_cost,
                pricing_source: (!acc.pricing_sources.is_empty()).then(|| {
                    acc.pricing_sources
                        .into_keys()
                        .collect::<Vec<_>>()
                        .join("; ")
                }),
                latency_ms_p50: percentile(&acc.latencies, 50.0),
                latency_ms_p95: percentile(&acc.latencies, 95.0),
            }
        })
        .collect();
    rollup
        .roles_detail
        .sort_by_key(|stat| std::cmp::Reverse(stat.calls));

    Some(rollup)
}

fn model_identity(trace: &RawModelTrace) -> (String, String, String) {
    if !trace.model.model.is_empty() {
        return (
            trace.model.operation.clone(),
            trace.model.operator.clone(),
            trace.model.model.clone(),
        );
    }
    if let Some(queue_id) = &trace.queue_id {
        let mut parts = queue_id.splitn(3, ':');
        let operation = parts.next().unwrap_or("").to_string();
        let operator = parts.next().unwrap_or("").to_string();
        let model = parts.next().unwrap_or(queue_id).to_string();
        return (operation, operator, model);
    }
    (
        trace.model.operation.clone(),
        trace.model.operator.clone(),
        trace.model.model.clone(),
    )
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
            "{\"model\":{\"operation\":\"chat\",\"operator\":\"deepseek\",\"model\":\"deepseek-v4-flash\"},\"timestamp\":\"2026-06-23T00:00:00Z\",\"role_binding\":\"bench.judge\",\"cache\":{\"response_cache\":\"miss\",\"prompt_cache\":\"partial_hit\",\"cached_input_tokens\":128},\"usage\":{\"input_tokens\":170,\"output_tokens\":1},\"timing\":{\"total_ms\":1441},\"outcome\":\"succeeded\"}\n{\"model\":{\"operation\":\"chat\",\"operator\":\"deepseek\",\"model\":\"deepseek-v4-flash\"},\"timestamp\":\"2026-06-23T00:00:00Z\",\"role_binding\":\"bench.judge\",\"cache\":{\"response_cache\":\"hit\",\"prompt_cache\":\"hit\",\"cached_input_tokens\":100},\"usage\":{\"input_tokens\":100,\"output_tokens\":2},\"timing\":{\"total_ms\":1000},\"outcome\":\"succeeded\"}\n",
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
        assert_eq!(rollup.cost_micro_usd, Some(7));
        assert!(rollup.cost_estimated);
        assert_eq!(
            rollup.pricing_table_version.as_deref(),
            Some(PRICING_TABLE_VERSION)
        );
        assert!(rollup.pricing_sources.iter().any(|source| {
            source == "DeepSeek historical 2026-06-23 snapshot: https://api-docs.deepseek.com/quick_start/pricing"
        }));
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

    #[test]
    fn rolls_up_provider_queue_traces_when_model_artifact_is_absent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir
            .path()
            .join("provider-queue")
            .join("model-queue-traces.jsonl");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            &path,
            "{\"queue_id\":\"chat:deepseek:deepseek-v4-flash\",\"item_id\":\"a\",\"operation\":\"chat\",\"status\":\"queued\",\"attempt\":0,\"timestamp\":\"2026-06-23T00:00:00Z\"}\n\
             {\"queue_id\":\"chat:deepseek:deepseek-v4-flash\",\"item_id\":\"a\",\"operation\":\"chat\",\"status\":\"running\",\"attempt\":1,\"timestamp\":\"2026-06-23T00:00:01Z\"}\n\
             {\"queue_id\":\"chat:deepseek:deepseek-v4-flash\",\"item_id\":\"a\",\"operation\":\"chat\",\"status\":\"succeeded\",\"attempt\":1,\"timestamp\":\"2026-06-23T00:00:02Z\",\"usage\":{\"prompt_tokens\":100,\"cache_hit_tokens\":90,\"cache_miss_tokens\":10,\"completion_tokens\":4}}\n\
             {\"queue_id\":\"embedding:gemini:gemini-embedding-2\",\"item_id\":\"b\",\"operation\":\"embedding\",\"status\":\"succeeded\",\"attempt\":1,\"timestamp\":\"2026-06-23T00:00:03Z\",\"usage\":{\"prompt_tokens\":1000,\"cache_miss_tokens\":1000,\"completion_tokens\":0}}\n\
             {\"queue_id\":\"embedding:openrouter:qwen/qwen3-embedding-8b\",\"item_id\":\"c\",\"operation\":\"embedding\",\"status\":\"succeeded\",\"attempt\":1,\"timestamp\":\"2026-06-23T00:00:04Z\",\"input_units\":100000,\"request_units\":8}\n",
        )
        .unwrap();

        let rollup = rollup_model_traces(dir.path()).unwrap();
        assert_eq!(rollup.calls, 3);
        assert_eq!(rollup.input_tokens, 101100);
        assert_eq!(rollup.cached_input_tokens, 90);
        assert_eq!(rollup.uncached_input_tokens, 1010);
        assert_eq!(rollup.unknown_cache_input_tokens, 100000);
        assert_eq!(rollup.output_tokens, 4);
        assert_eq!(rollup.cost_micro_usd, Some(1203));
        assert!(rollup.cost_estimated);
        assert_eq!(rollup.prompt_cache_partial_hits, 1);
        assert_eq!(rollup.prompt_cache_misses, 1);
        assert_eq!(rollup.models.len(), 3);
        assert!(rollup.models.iter().any(|stat| {
            stat.model == "deepseek-v4-flash"
                && stat.operator == "deepseek"
                && stat.operation == "chat"
        }));
        assert!(rollup.models.iter().any(|stat| {
            stat.model == "gemini-embedding-2"
                && stat.operator == "gemini"
                && stat.operation == "embedding"
                && stat.input_tokens == 1000
                && stat.cost_micro_usd == Some(200)
        }));
        assert!(rollup.models.iter().any(|stat| {
            stat.model == "qwen/qwen3-embedding-8b"
                && stat.operator == "openrouter"
                && stat.operation == "embedding"
                && stat.input_tokens == 100000
                && stat.cost_micro_usd == Some(1000)
        }));
    }

    #[test]
    fn prices_cohere_rerank_from_input_units() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("artifacts").join("model-traces.jsonl");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        // A rerank call carries `input_units` (reranked doc tokens) and no `usage` block,
        // exactly like an OpenRouter embedding call — it must be priced, not dropped to $0.
        std::fs::write(
            &path,
            "{\"queue_id\":\"rerank:openrouter:cohere/rerank-4-fast\",\"item_id\":\"r\",\"operation\":\"rerank\",\"status\":\"succeeded\",\"attempt\":1,\"timestamp\":\"2026-01-01T00:00:00Z\",\"input_units\":17930,\"request_units\":221}\n",
        )
        .unwrap();

        let rollup = rollup_model_traces(dir.path()).unwrap();
        assert_eq!(rollup.calls, 1);
        assert_eq!(rollup.input_tokens, 17930);
        assert_eq!(rollup.output_tokens, 0);
        // Rerank is billed per SEARCH: 1 query = 1 search = $0.002 = 2000 micro-USD (doc count irrelevant)
        assert_eq!(rollup.cost_micro_usd, Some(2000));
        assert!(rollup.cost_estimated);
        assert!(rollup.models.iter().any(|stat| {
            stat.model == "cohere/rerank-4-fast"
                && stat.operator == "openrouter"
                && stat.operation == "rerank"
                && stat.input_tokens == 17930
                && stat.cost_micro_usd == Some(2000)
        }));
    }
    fn rollup_one(value: serde_json::Value) -> ModelTraceRollup {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("model-traces.jsonl");
        std::fs::write(&path, value.to_string()).unwrap();
        rollup_model_trace_file(&path).unwrap()
    }

    fn flash_trace(model: &str, timestamp: &str) -> serde_json::Value {
        serde_json::json!({
            "model": {"operator": "deepseek", "operation": "chat", "model": model},
            "timestamp": timestamp,
            "role_binding": "bench.answer",
            "usage": {"input_tokens": 1000000, "output_tokens": 1000000,
                      "cache_hit_tokens": 500000, "cache_miss_tokens": 500000},
            "outcome": "succeeded"
        })
    }

    #[test]
    fn prices_current_flash_aliases_at_weekday_utc_boundaries() {
        for model in [
            "deepseek-flash",
            "deepseek-v4-flash",
            "deepseek-v4-flash-vision-exp",
        ] {
            for (timestamp, cost) in [
                ("2026-09-17T00:59:59Z", 676500),
                ("2026-09-17T01:00:00Z", 1353000),
                ("2026-09-17T03:59:59Z", 1353000),
                ("2026-09-17T04:00:00Z", 676500),
                ("2026-09-17T05:59:59Z", 676500),
                ("2026-09-17T06:00:00Z", 1353000),
                ("2026-09-17T09:59:59Z", 1353000),
                ("2026-09-17T10:00:00Z", 676500),
                ("2026-09-19T01:00:00Z", 676500),
                ("2026-09-20T06:00:00Z", 676500),
                ("2026-09-17T09:00:00+08:00", 1353000),
            ] {
                assert_eq!(
                    rollup_one(flash_trace(model, timestamp)).cost_micro_usd,
                    Some(cost),
                    "{model} {timestamp}"
                );
            }
        }
    }

    #[test]
    fn preserves_historical_rates_without_guessing_missing_rate_periods() {
        assert_eq!(
            rollup_one(flash_trace("deepseek-v4-flash", "2026-06-23T01:00:00Z")).cost_micro_usd,
            Some(351400)
        );
        assert_eq!(
            rollup_one(flash_trace("deepseek-v4-pro", "2026-06-23T01:00:00Z")).cost_micro_usd,
            Some(1089313)
        );
        assert_eq!(
            rollup_one(flash_trace("deepseek-v4-flash", "2026-08-16T15:59:59Z")).cost_micro_usd,
            Some(351400)
        );
        for timestamp in [
            "2026-08-16T16:00:00Z",
            "2026-09-10T03:59:59Z",
            "",
            "not-a-date",
        ] {
            assert_eq!(
                rollup_one(flash_trace("deepseek-v4-flash", timestamp)).cost_micro_usd,
                None,
                "{timestamp}"
            );
        }
        assert_eq!(
            rollup_one(flash_trace("deepseek-v4-flash", "2026-09-10T04:00:00Z")).cost_micro_usd,
            Some(676500)
        );
        assert_eq!(
            rollup_one(flash_trace("deepseek-flash", "2026-06-23T01:00:00Z")).cost_micro_usd,
            None
        );
        assert_eq!(
            rollup_one(flash_trace("unknown", "2026-09-17T01:00:00Z")).cost_micro_usd,
            None
        );
    }

    #[test]
    fn unknown_cache_does_not_become_a_miss_or_a_known_price() {
        let mut trace = flash_trace("deepseek-flash", "2026-09-17T01:00:00Z");
        trace["usage"]
            .as_object_mut()
            .unwrap()
            .remove("cache_hit_tokens");
        trace["usage"]
            .as_object_mut()
            .unwrap()
            .remove("cache_miss_tokens");
        let rollup = rollup_one(trace);
        assert_eq!(rollup.cost_micro_usd, None);
        assert_eq!(rollup.prompt_cache_misses, 0);
        assert_eq!(rollup.uncached_input_tokens, 0);
    }

    #[test]
    fn replay_has_zero_new_cost_even_when_old_usage_reports_a_price() {
        let mut trace = flash_trace("deepseek-flash", "2026-09-17T01:00:00Z");
        trace["cache"] = serde_json::json!({"response_cache":"hit"});
        trace["usage"]["cost_micro_usd"] = 12345.into();
        let rollup = rollup_one(trace);
        assert_eq!(rollup.cost_micro_usd, Some(0));
        assert_eq!(rollup.models[0].cost_micro_usd, Some(0));
        assert_eq!(rollup.roles_detail[0].cost_micro_usd, Some(0));
        assert!(!rollup.cost_estimated);
    }

    #[test]
    fn reported_cost_wins_over_missing_rates_and_usage() {
        let mut trace = flash_trace("unknown", "");
        trace["usage"] = serde_json::json!({"cost_micro_usd": 12345});
        let rollup = rollup_one(trace);
        assert_eq!(rollup.cost_micro_usd, Some(12345));
        assert!(!rollup.cost_estimated);
    }
    #[test]
    fn incomplete_usage_and_inconsistent_cache_remain_unpriced() {
        for usage in [
            serde_json::json!({}),
            serde_json::json!({"input_tokens": 1000000, "cache_hit_tokens": 0}),
            serde_json::json!({"input_tokens": 1000000, "output_tokens": 10,
                               "cache_hit_tokens": 900000, "cache_miss_tokens": 200000}),
            serde_json::json!({"input_tokens": 100, "output_tokens": 10,
                               "cache_hit_tokens": 101}),
        ] {
            let mut trace = flash_trace("deepseek-flash", "2026-09-17T01:00:00Z");
            trace["usage"] = usage;
            let rollup = rollup_one(trace);
            assert_eq!(rollup.cost_micro_usd, None);
            assert_eq!(rollup.unpriced_calls, 1);
            assert_eq!(rollup.models[0].unpriced_calls, 1);
            assert_eq!(rollup.roles_detail[0].unpriced_calls, 1);
        }
    }

    #[test]
    fn a_single_cache_counter_completes_the_split() {
        for usage in [
            serde_json::json!({"input_tokens": 1000000, "output_tokens": 1000000,
                               "cache_hit_tokens": 500000}),
            serde_json::json!({"input_tokens": 1000000, "output_tokens": 1000000,
                               "cache_miss_tokens": 500000}),
        ] {
            let mut trace = flash_trace("deepseek-flash", "2026-09-17T01:00:00Z");
            trace["usage"] = usage;
            let rollup = rollup_one(trace);
            assert_eq!(rollup.cost_micro_usd, Some(1353000));
            assert_eq!(rollup.cached_input_tokens, 500000);
            assert_eq!(rollup.uncached_input_tokens, 500000);
            assert_eq!(rollup.unknown_cache_input_tokens, 0);
            assert_eq!(rollup.unpriced_calls, 0);
        }
    }

    #[test]
    fn a_partial_sum_is_not_a_total_cost() {
        let known = flash_trace("deepseek-flash", "2026-09-17T01:00:00Z");
        let unknown = flash_trace("deepseek-flash", "");
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("model-traces.jsonl");
        std::fs::write(&path, format!("{known}\n{unknown}\n")).unwrap();
        let rollup = rollup_model_trace_file(&path).unwrap();
        assert_eq!(rollup.cost_micro_usd, None);
        assert_eq!(rollup.unpriced_calls, 1);
        assert_eq!(rollup.models[0].cost_micro_usd, None);
        assert_eq!(rollup.roles_detail[0].cost_micro_usd, None);
    }
    #[test]
    fn absent_timestamp_and_conflicting_cache_sources_remain_unknown() {
        let mut trace = flash_trace("deepseek-flash", "2026-09-17T01:00:00Z");
        trace.as_object_mut().unwrap().remove("timestamp");
        assert_eq!(rollup_one(trace).cost_micro_usd, None);
        let mut trace = flash_trace("deepseek-flash", "2026-09-17T01:00:00Z");
        trace["cache"] = serde_json::json!({"cached_input_tokens": 100});
        let rollup = rollup_one(trace);
        assert_eq!(rollup.cost_micro_usd, None);
        assert_eq!(rollup.unknown_cache_input_tokens, 1000000);
        assert_eq!(rollup.prompt_cache_misses, 0);
    }
    #[test]
    fn june_snapshot_does_not_reprice_earlier_history() {
        for model in ["deepseek-v4-flash", "deepseek-v4-pro"] {
            for timestamp in ["2026-01-01T00:00:00Z", "2026-06-22T23:59:59Z"] {
                let rollup = rollup_one(flash_trace(model, timestamp));
                assert_eq!(rollup.cost_micro_usd, None, "{model} {timestamp}");
                assert_eq!(rollup.unpriced_calls, 1);
            }
        }
        assert_eq!(
            rollup_one(flash_trace("deepseek-v4-flash", "2026-06-23T00:00:00Z")).cost_micro_usd,
            Some(351400)
        );
    }
    #[test]
    fn normalized_cache_labels_without_counters_do_not_establish_a_split() {
        // Foundation emits these labels even when the provider omitted both counters.
        for label in ["miss", "hit", "partial_hit"] {
            let trace = serde_json::json!({
                "model": {"operation":"chat", "operator":"deepseek", "model":"deepseek-flash"},
                "timestamp":"2026-09-17T01:00:00Z",
                "cache": {"response_cache":"miss", "prompt_cache":label,
                          "cached_input_tokens":null},
                "usage": {"input_tokens":1000000, "output_tokens":1000000,
                          "reasoning_tokens":null, "media_units":null, "cost_micro_usd":null},
                "outcome":"succeeded"
            });
            let rollup = rollup_one(trace);
            assert_eq!(rollup.cost_micro_usd, None, "{label}");
            assert_eq!(rollup.unknown_cache_input_tokens, 1000000);
            assert_eq!(rollup.prompt_cache_misses, 0);
            assert_eq!(rollup.prompt_cache_hits, 0);
        }
    }

    #[test]
    fn queue_estimated_cost_does_not_override_dated_provider_rates() {
        let mut trace = serde_json::json!({
            "queue_id":"chat:deepseek:deepseek-flash", "status":"succeeded",
            "timestamp":"2026-09-17T01:00:00Z", "cost_micro_usd":351400,
            "usage": {"prompt_tokens":1000000, "completion_tokens":1000000,
                      "cache_hit_tokens":500000, "cache_miss_tokens":500000}
        });
        let rollup = rollup_one(trace.clone());
        assert_eq!(rollup.cost_micro_usd, Some(1353000));
        assert!(rollup.cost_estimated);
        trace["timestamp"] = "".into();
        assert_eq!(rollup_one(trace.clone()).cost_micro_usd, None);
        trace["usage"]["cost_micro_usd"] = 12345.into();
        let rollup = rollup_one(trace);
        assert_eq!(rollup.cost_micro_usd, Some(12345));
        assert!(!rollup.cost_estimated);
    }
    #[test]
    fn provider_decimal_receipt_is_reported_and_respects_cost_precedence() {
        let mut trace = serde_json::json!({
            "queue_id":"chat:unknown:unknown", "status":"succeeded",
            "cost_micro_usd":351400,
            "usage": {"provider": {"response_id":"synthetic-response", "served_model":"unknown",
                       "created":1789600000, "reasoning_tokens":42,
                       "reported_cost_usd":"0.00000349"}}
        });
        let rollup = rollup_one(trace.clone());
        assert_eq!(rollup.cost_micro_usd, Some(3));
        assert!(!rollup.cost_estimated);
        assert_eq!(rollup.unpriced_calls, 0);
        trace["usage"]["cost_micro_usd"] = 9.into();
        assert_eq!(rollup_one(trace.clone()).cost_micro_usd, Some(9));
        trace["cache"] = serde_json::json!({"response_cache":"hit"});
        assert_eq!(rollup_one(trace).cost_micro_usd, Some(0));
    }

    #[test]
    fn provider_decimal_receipt_rejects_invalid_costs_and_rounds_to_micro_usd() {
        for (dollars, expected) in [
            ("0", Some(0)),
            ("0.00000049", Some(0)),
            ("0.0000005", Some(1)),
            ("0.1234567", Some(123457)),
            ("1e-6", Some(1)),
            ("NaN", None),
            ("inf", None),
            ("-0.01", None),
            ("not-cost", None),
            ("1e308", None),
            ("18446744073709551616", None),
        ] {
            let rollup = rollup_one(serde_json::json!({
                "queue_id":"chat:unknown:unknown", "status":"succeeded",
                "usage":{"provider":{"reported_cost_usd":dollars}}
            }));
            assert_eq!(rollup.cost_micro_usd, expected, "{dollars}");
            assert_eq!(rollup.calls, 1);
            assert!(!rollup.cost_estimated);
        }
    }
}
