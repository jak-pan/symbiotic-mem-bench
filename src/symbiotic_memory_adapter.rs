use chrono::{DateTime, Local, NaiveDateTime, SecondsFormat, TimeZone, Utc};
#[cfg(feature = "symbiotic-memory-adapter")]
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[cfg(feature = "symbiotic-memory-adapter")]
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fs;
#[cfg(feature = "symbiotic-memory-adapter")]
use std::io::Read;
use std::io::{BufRead, Write};
use std::path::Path;
#[cfg(feature = "symbiotic-memory-adapter")]
use std::path::PathBuf;
#[cfg(feature = "symbiotic-memory-adapter")]
use std::sync::{Arc, Mutex};
#[cfg(feature = "symbiotic-memory-adapter")]
use std::time::{Duration, Instant};
#[cfg(feature = "symbiotic-memory-adapter")]
use symbiotic_core::{QueueId, QueueItemId};
#[cfg(feature = "symbiotic-memory-adapter")]
use symbiotic_memory::ingest::{
    DEFAULT_DISTILL_WINDOW_MAX_INPUT_TOKENS, DEFAULT_EMBED_MAX_INPUT_TOKENS, raw_unit_fingerprint,
};
use symbiotic_memory::ingest::{Distiller, IngestDiagnosticMode, IngestPipeline};
#[cfg(feature = "symbiotic-memory-adapter")]
use symbiotic_memory::manifest::{MemoryRunManifest, MemoryStage, stable_hash_json};
use symbiotic_memory::providers::{ChatProvider, EmbeddingProvider, Reranker};
use symbiotic_memory::recall::RecallEngine;
#[cfg(feature = "symbiotic-memory-adapter")]
use symbiotic_memory::recall::{QueryPlanner, RecallAnswerDebug, RecallTraceContext};
use symbiotic_memory::storage::MemoryStore;
#[cfg(feature = "symbiotic-memory-adapter")]
use symbiotic_memory::trace::{
    MemoryTraceEvent, MemoryTraceEventKind, MemoryTraceOperation, MemoryTraceSink,
};
#[cfg(feature = "symbiotic-memory-adapter")]
use symbiotic_memory::types::{FactEvidence, RawTurnEvidence};
use symbiotic_memory::types::{SourceDocument, SourceTurn};
#[cfg(feature = "symbiotic-memory-adapter")]
use symbiotic_queue::{
    EnqueueDisposition, EnqueueRequest, QueueBackend, QueueError, QueueItem,
    QueueStatus as DurableQueueStatus, SqliteQueue,
};

/// The reranker configuration threaded from the harness into the `RecallEngine`. Carries the main
/// (stage-2) reranker plus an optional cheap stage-1 prefilter reranker and its top-x cut. Built by
/// `membench`'s `reranker()` from the `SYMEM_RERANK*` env knobs; a `None` cascade (or a cascade with
/// `main == None`) means rerank is disabled.
#[cfg(feature = "symbiotic-memory-adapter")]
#[derive(Clone, Default)]
pub struct RerankCascade {
    /// Main (stage-2) reranker. `None` when SYMEM_RERANK is off.
    pub main: Option<Arc<dyn Reranker>>,
    /// Optional cheap stage-1 prefilter reranker (enabled when SYMEM_RERANK_STAGE1_MODEL is set).
    pub stage1: Option<Arc<dyn Reranker>>,
    /// Stage-1 -> stage-2 count cut (SYMEM_RERANK_STAGE1_TOP_X, default 20). Only meaningful when
    /// `stage1` is present.
    pub stage1_top_x: usize,
}

fn current_reference_datetime() -> String {
    Local::now().to_rfc3339_opts(SecondsFormat::Secs, false)
}

fn longmemeval_answer_reference_datetime(row: &LongMemEvalRecord) -> String {
    let benchmark_datetime = row
        .question_date
        .as_deref()
        .and_then(parse_longmemeval_datetime)
        .map(|datetime| datetime.to_rfc3339_opts(SecondsFormat::Secs, false));
    select_answer_reference_datetime(
        |key| std::env::var(key).ok(),
        benchmark_datetime,
        current_reference_datetime(),
    )
}

fn select_answer_reference_datetime<F>(
    env: F,
    benchmark_datetime: Option<String>,
    default_datetime: String,
) -> String
where
    F: Fn(&str) -> Option<String>,
{
    [
        "MEMBENCH_REFERENCE_DATETIME",
        "SYMEM_REFERENCE_DATETIME",
        // Compatibility aliases for explicitly pinned benchmark clocks.
        "MEMBENCH_REFERENCE_DATE",
        "SYMEM_REFERENCE_DATE",
    ]
    .into_iter()
    .find_map(|key| {
        env(key)
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    })
    .or(benchmark_datetime)
    .unwrap_or(default_datetime)
}

#[derive(Clone, Debug, Deserialize)]
pub struct LongMemEvalRecord {
    pub question_id: String,
    pub question_type: Option<String>,
    pub question: String,
    pub question_date: Option<String>,
    pub answer: Option<Value>,
    /// Gold evidence session ids (`answer_<hash>[_<N>]`); the pieces a correct
    /// answer must draw on. Used by `gold-eval` to ground retrieval coverage.
    #[serde(default)]
    pub answer_session_ids: Vec<String>,
    pub haystack_dates: Vec<String>,
    pub haystack_session_ids: Vec<String>,
    pub haystack_sessions: Vec<Vec<LongMemEvalMessage>>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct LongMemEvalMessage {
    pub role: String,
    pub content: String,
    /// LongMemEval ground-truth turn-level evidence flag: true on the exact turns that contain the
    /// answer. Most turns are false (assistant lectures, chit-chat, adjacent topics = noise).
    #[serde(default)]
    pub has_answer: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BenchHypothesis {
    pub question_id: String,
    pub question_type: Option<String>,
    pub question: String,
    pub hypothesis: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub debug_artifact: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub router_initial: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub router_final: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub router_reason: Option<String>,
}

#[cfg(feature = "symbiotic-memory-adapter")]
#[derive(Clone, Debug, Serialize)]
pub struct ScoreRecordReport {
    pub hypotheses: usize,
    pub verdicts: usize,
    pub debug_files_updated: usize,
    pub summary_path: String,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct BenchDebugMetadata {
    pub capabilities: BenchTraceCapabilities,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub models: Vec<BenchModelDebug>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub trace_artifacts: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pricing_table_version: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct BenchTraceCapabilities {
    pub supported: BenchSupportedCapabilities,
    pub observed: BenchObservedCapabilities,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct BenchSupportedCapabilities {
    pub reset: bool,
    pub durable_state: bool,
    pub ingest: bool,
    pub flush: bool,
    pub retrieve: bool,
    pub answer: bool,
    pub provider_injection: bool,
    pub embedding_injection: bool,
    pub raw_context: bool,
    pub score_explain: bool,
    pub retry_trace: bool,
    pub token_usage: bool,
    pub cache_usage: bool,
    pub cost_usage: bool,
    pub queue_events: bool,
    pub state_export: bool,
    pub native_stage_trace: bool,
    pub wrapped_api_trace: bool,
    pub provider_trace: bool,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct BenchObservedCapabilities {
    pub ingest_input: bool,
    pub ingest_output: bool,
    pub model_calls: bool,
    pub embedding_calls: bool,
    pub retrieval_queries: bool,
    pub retrieval_candidates: bool,
    pub retrieval_scores: bool,
    pub raw_context: bool,
    pub answer_prompt: bool,
    pub answer_output: bool,
    pub errors: bool,
    pub retries: bool,
    pub token_usage: bool,
    pub cache_usage: bool,
    pub timing: bool,
    pub cost: bool,
    pub scoring_verdict: bool,
    pub native_stage_trace: bool,
    pub wrapped_api_trace: bool,
    pub provider_trace: bool,
    pub memory_stage_events: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct BenchModelDebug {
    pub label: String,
    pub operation: String,
    pub operator: String,
    pub model: String,
    pub queue_id: String,
    pub role_binding: String,
    pub max_in_flight: usize,
    pub lease_seconds: u64,
    pub retry_attempts: u32,
    pub logical_retry_attempts: u32,
    pub retry_jitter_seconds: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_seconds: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requests_per_minute: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_units_per_minute: Option<u64>,
    pub response_cache_enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
}

pub fn load_longmemeval(
    path: impl AsRef<Path>,
    limit: Option<usize>,
) -> anyhow::Result<Vec<LongMemEvalRecord>> {
    let raw = fs::read_to_string(path)?;
    let mut rows: Vec<LongMemEvalRecord> = serde_json::from_str(&raw)?;
    if let Some(limit) = limit {
        rows.truncate(limit);
    }
    Ok(rows)
}

pub fn longmemeval_to_source(record: &LongMemEvalRecord) -> SourceDocument {
    let mut turns = Vec::new();
    let mut first_event_time = None;
    for (session_idx, session) in record.haystack_sessions.iter().enumerate() {
        let session_id = record
            .haystack_session_ids
            .get(session_idx)
            .cloned()
            .unwrap_or_else(|| format!("session-{session_idx}"));
        let session_time = record
            .haystack_dates
            .get(session_idx)
            .and_then(|date| parse_longmemeval_datetime(date));
        if first_event_time.is_none() {
            first_event_time = session_time;
        }
        for (msg_idx, msg) in session.iter().enumerate() {
            turns.push(SourceTurn {
                turn_id: format!("{session_id}:{msg_idx}"),
                source_id: record.question_id.clone(),
                speaker: Some(msg.role.clone()),
                // MUST stay None: actor is skip_serializing_if-None and ingest_source_hash covers
                // the whole SourceDocument, so any value here would change every golden vault's
                // source hash and trip the answer-only staleness gate. The LongMemEval role
                // already lives in `speaker`.
                actor: None,
                // The LongMemEval haystack date is when the session was held — i.e. the capture
                // timestamp. A raw turn is never a resolved event, so event_time is None and
                // ingested_at is stamped at persist time.
                captured_at: session_time,
                event_time: None,
                ingested_at: None,
                text: msg.content.clone(),
                ordinal: turns.len(),
                locator: None,
                scope: Default::default(),
            });
        }
    }
    SourceDocument {
        source_id: record.question_id.clone(),
        source_kind: "longmemeval".to_string(),
        captured_at: first_event_time.unwrap_or_else(Utc::now),
        turns,
        raw_payload: None,
        locator: None,
    }
}

pub fn parse_longmemeval_datetime(input: &str) -> Option<DateTime<Utc>> {
    let without_weekday = regex::Regex::new(r"\s*\([^)]+\)")
        .ok()?
        .replace_all(input.trim(), "")
        .to_string();
    NaiveDateTime::parse_from_str(&without_weekday, "%Y/%m/%d %H:%M")
        .ok()
        .map(|dt| Utc.from_utc_datetime(&dt))
}

pub async fn run_longmemeval_slice<S, E, D, C>(
    rows: &[LongMemEvalRecord],
    store_factory: impl Fn() -> S,
    embedder_factory: impl Fn() -> E + Send + Sync + 'static,
    distiller_factory: impl Fn() -> D + Send + Sync + 'static,
    chat_factory: impl Fn() -> C + Send + Sync + 'static,
    policy: symbiotic_memory::config::RecallPolicy,
    out_path: impl AsRef<Path>,
) -> anyhow::Result<Vec<BenchHypothesis>>
where
    S: MemoryStore + Clone,
    E: EmbeddingProvider + Clone + 'static,
    D: Distiller,
    C: ChatProvider,
{
    if let Some(parent) = out_path.as_ref().parent() {
        fs::create_dir_all(parent)?;
    }
    let mut out = read_existing_hypotheses(out_path.as_ref())?;
    let mut completed: BTreeSet<_> = out.iter().map(|h| h.question_id.clone()).collect();
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(out_path.as_ref())?;

    for (idx, row) in rows.iter().enumerate() {
        if completed.contains(&row.question_id) {
            eprintln!(
                "[longmemeval] {}/{} {} skipped",
                idx + 1,
                rows.len(),
                row.question_id
            );
            continue;
        }
        eprintln!(
            "[longmemeval] {}/{} {} ingest+answer",
            idx + 1,
            rows.len(),
            row.question_id
        );
        let store = store_factory();
        let embedder = embedder_factory();
        let ingest = IngestPipeline::new(store.clone(), embedder.clone(), distiller_factory());
        ingest.ingest(longmemeval_to_source(row)).await?;
        let engine = RecallEngine::new(store, embedder, chat_factory(), policy.clone());
        let reference_date = longmemeval_answer_reference_datetime(row);
        let answer = engine
            .answer_with_reference_date(&row.question, Some(reference_date.as_str()))
            .await?;
        let hypothesis = BenchHypothesis {
            question_id: row.question_id.clone(),
            question_type: row.question_type.clone(),
            question: row.question.clone(),
            hypothesis: answer.text,
            debug_artifact: None,
            router_initial: None,
            router_final: None,
            router_reason: None,
        };
        writeln!(file, "{}", serde_json::to_string(&hypothesis)?)?;
        file.flush()?;
        completed.insert(hypothesis.question_id.clone());
        out.push(hypothesis);
    }

    Ok(out)
}

/// Process-global redo stage. Reuse/redo decision tree over a source vault (set once per run):
///   embed   → re-embed facts+briefs (reuse distill+reweave content); cheapest, no LLM
///   reweave → re-run consolidation (reuse distill + fact vectors); consolidator LLM only
///   distill → re-distill onward (reuse captured turns); full $$ except capture
///   index   → rebuild the recall index only (reuse all embeddings)
/// embed is handled in-adapter; reweave/distill/index invalidate manifest stages and let the
/// existing ingest path re-run them, so the real pipeline reuses every valid upstream stage.
static REDO_STAGE: std::sync::OnceLock<String> = std::sync::OnceLock::new();

pub fn set_redo_stage(stage: Option<String>) {
    if let Some(stage) = stage {
        let _ = REDO_STAGE.set(stage);
    }
}

fn redo_stage() -> Option<&'static str> {
    REDO_STAGE.get().map(String::as_str)
}

fn reembed_mode() -> bool {
    redo_stage() == Some("embed")
}

/// Whether `--re-embed` should also recompute raw-turn embeddings. Default FALSE: a re-embed only
/// changes fact `search_text` (via the distill/enrich metadata logic) — raw-turn text is identical,
/// so re-embedding turns re-calls the embedding API for every turn just to produce byte-identical
/// vectors. That bulk turn re-embed is the dominant cost of a full re-embed. With this off, the prep
/// COPIES the source `zvec-hybrid` index (preserving the existing turn vectors) and only facts are
/// re-embedded + upserted. Set `SYMEM_REEMBED_TURNS=1` to force a full turn re-embed — required when
/// the embedding model or dimensions change (the copied index would then be stale).
pub fn reembed_turns() -> bool {
    std::env::var("SYMEM_REEMBED_TURNS")
        .ok()
        .is_some_and(|value| matches!(value.trim(), "1" | "true" | "TRUE" | "yes" | "YES" | "on"))
}

/// Embed texts in bounded batches, firing batches CONCURRENTLY through the provider queue (which is
/// sized for ~1000 in-flight) rather than awaiting one at a time. `buffered` preserves input order,
/// so the returned vectors line up with `texts`. Chunk size bounds per-request size (e.g. Gemini
/// caps embed batches at 100); concurrency bounds in-flight batches per vault.
async fn embed_texts_in_chunks<E: EmbeddingProvider>(
    embedder: &E,
    texts: &[String],
) -> anyhow::Result<Vec<Vec<f32>>> {
    let chunk = std::env::var("SYMEM_REEMBED_CHUNK")
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|size| *size > 0)
        .unwrap_or(100);
    let concurrency = std::env::var("SYMEM_REEMBED_CONCURRENCY")
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|size| *size > 0)
        .unwrap_or(16);
    let batches: Vec<Vec<String>> = texts.chunks(chunk).map(<[String]>::to_vec).collect();
    let results: Vec<Result<Vec<Vec<f32>>, _>> = futures::stream::iter(batches)
        .map(|batch| async move { embedder.embed_many(&batch).await })
        .buffered(concurrency)
        .collect()
        .await;
    let mut out = Vec::with_capacity(texts.len());
    for result in results {
        out.extend(result.map_err(|err| anyhow::anyhow!("re-embed embedding failed: {err}"))?);
    }
    Ok(out)
}

#[cfg(feature = "symbiotic-memory-adapter")]
pub async fn run_longmemeval_sqlite<E, D, C>(
    rows: &[LongMemEvalRecord],
    run_root: impl AsRef<Path>,
    embedder_factory: impl Fn() -> E + Send + Sync + 'static,
    distiller_factory: impl Fn() -> D + Send + Sync + 'static,
    chat_factory: impl Fn() -> C + Send + Sync + 'static,
    policy: symbiotic_memory::config::RecallPolicy,
    out_path: impl AsRef<Path>,
    routed: bool,
    answer_only: bool,
    consolidate_briefs: bool,
    stop_after_raw_embed: bool,
    ingest_diagnostic_mode: IngestDiagnosticMode,
    allow_terminal_reenqueue: bool,
) -> anyhow::Result<Vec<BenchHypothesis>>
where
    E: EmbeddingProvider + Clone + Send + Sync + 'static,
    D: Distiller + 'static,
    C: ChatProvider + 'static,
{
    run_longmemeval_sqlite_with_planner(
        rows,
        run_root,
        embedder_factory,
        distiller_factory,
        None,
        chat_factory,
        None,
        None,
        RerankCascade::default(),
        None,
        None,
        policy,
        out_path,
        routed,
        answer_only,
        consolidate_briefs,
        stop_after_raw_embed,
        ingest_diagnostic_mode,
        None,
        allow_terminal_reenqueue,
    )
    .await
}

#[cfg(feature = "symbiotic-memory-adapter")]
pub async fn run_longmemeval_sqlite_with_planner<E, D, C>(
    rows: &[LongMemEvalRecord],
    run_root: impl AsRef<Path>,
    embedder_factory: impl Fn() -> E + Send + Sync + 'static,
    distiller_factory: impl Fn() -> D + Send + Sync + 'static,
    consolidator_factory: Option<Arc<dyn Fn() -> Arc<dyn Distiller> + Send + Sync>>,
    chat_factory: impl Fn() -> C + Send + Sync + 'static,
    answer_retry_factory: Option<Arc<dyn Fn() -> Arc<dyn ChatProvider> + Send + Sync>>,
    planner_factory: Option<Arc<dyn Fn() -> Arc<dyn QueryPlanner> + Send + Sync>>,
    reranker: RerankCascade,
    debug_metadata: Option<BenchDebugMetadata>,
    memory_trace_sink: Option<Arc<dyn MemoryTraceSink>>,
    policy: symbiotic_memory::config::RecallPolicy,
    out_path: impl AsRef<Path>,
    routed: bool,
    answer_only: bool,
    consolidate_briefs: bool,
    stop_after_raw_embed: bool,
    ingest_diagnostic_mode: IngestDiagnosticMode,
    workflow_max_in_flight_override: Option<usize>,
    allow_terminal_reenqueue: bool,
) -> anyhow::Result<Vec<BenchHypothesis>>
where
    E: EmbeddingProvider + Clone + Send + Sync + 'static,
    D: Distiller + 'static,
    C: ChatProvider + 'static,
{
    fs::create_dir_all(run_root.as_ref())?;
    let workflow_queue_path = run_root
        .as_ref()
        .join("workflow")
        .join("longmemeval")
        .join("queue.sqlite");
    let workflow_queue: Arc<dyn QueueBackend> = Arc::new(SqliteQueue::open(&workflow_queue_path)?);
    let workflow_queue_id = QueueId::new("workflow:longmemeval");
    let _legacy_event_path = run_root
        .as_ref()
        .join("workflow")
        .join("longmemeval")
        .join("events.jsonl");
    if _legacy_event_path.exists() {
        eprintln!(
            "[longmemeval] ignoring legacy workflow event log at {}",
            _legacy_event_path.display()
        );
    }
    fs::create_dir_all(run_root.as_ref().join("workflow").join("longmemeval"))?;
    run_longmemeval_sqlite_with_workflow_queue(
        rows,
        run_root,
        workflow_queue,
        workflow_queue_id,
        embedder_factory,
        distiller_factory,
        consolidator_factory,
        chat_factory,
        answer_retry_factory,
        planner_factory,
        reranker,
        debug_metadata,
        memory_trace_sink,
        policy,
        out_path,
        routed,
        answer_only,
        consolidate_briefs,
        stop_after_raw_embed,
        ingest_diagnostic_mode,
        workflow_max_in_flight_override,
        allow_terminal_reenqueue,
    )
    .await
}

#[cfg(feature = "symbiotic-memory-adapter")]
pub async fn run_longmemeval_sqlite_with_workflow_queue<E, D, C>(
    rows: &[LongMemEvalRecord],
    run_root: impl AsRef<Path>,
    workflow_queue: Arc<dyn QueueBackend>,
    workflow_queue_id: QueueId,
    embedder_factory: impl Fn() -> E + Send + Sync + 'static,
    distiller_factory: impl Fn() -> D + Send + Sync + 'static,
    consolidator_factory: Option<Arc<dyn Fn() -> Arc<dyn Distiller> + Send + Sync>>,
    chat_factory: impl Fn() -> C + Send + Sync + 'static,
    answer_retry_factory: Option<Arc<dyn Fn() -> Arc<dyn ChatProvider> + Send + Sync>>,
    planner_factory: Option<Arc<dyn Fn() -> Arc<dyn QueryPlanner> + Send + Sync>>,
    reranker: RerankCascade,
    debug_metadata: Option<BenchDebugMetadata>,
    memory_trace_sink: Option<Arc<dyn MemoryTraceSink>>,
    policy: symbiotic_memory::config::RecallPolicy,
    out_path: impl AsRef<Path>,
    routed: bool,
    answer_only: bool,
    consolidate_briefs: bool,
    stop_after_raw_embed: bool,
    ingest_diagnostic_mode: IngestDiagnosticMode,
    workflow_max_in_flight_override: Option<usize>,
    allow_terminal_reenqueue: bool,
) -> anyhow::Result<Vec<BenchHypothesis>>
where
    E: EmbeddingProvider + Clone + Send + Sync + 'static,
    D: Distiller + 'static,
    C: ChatProvider + 'static,
{
    if let Some(parent) = out_path.as_ref().parent() {
        fs::create_dir_all(parent)?;
    }
    fs::create_dir_all(run_root.as_ref())?;
    let completed = if answer_only && !allow_terminal_reenqueue {
        reset_hypotheses_for_answer_only(out_path.as_ref())?;
        BTreeSet::new()
    } else {
        read_existing_hypothesis_ids(out_path.as_ref())?
    };
    let debug_run_id = debug_run_id(out_path.as_ref());
    let file = Arc::new(Mutex::new(
        fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(out_path.as_ref())?,
    ));
    let run_root = run_root.as_ref().to_path_buf();
    let embedder_factory = Arc::new(embedder_factory);
    let distiller_factory = Arc::new(distiller_factory);
    let consolidator_factory = Arc::new(consolidator_factory);
    let chat_factory = Arc::new(chat_factory);
    let answer_retry_factory = Arc::new(answer_retry_factory);
    let planner_factory = Arc::new(planner_factory);
    let reranker = Arc::new(reranker);
    let debug_metadata = Arc::new(debug_metadata);
    let memory_trace_sink = Arc::new(memory_trace_sink);
    let policy = Arc::new(policy);
    let consolidate_briefs = Arc::new(consolidate_briefs);
    let stop_after_raw_embed = Arc::new(stop_after_raw_embed);
    let ingest_diagnostic_mode = Arc::new(ingest_diagnostic_mode);
    let total = rows.len();
    let question_timeout = question_timeout();
    let workflow_max_in_flight = workflow_max_in_flight(workflow_max_in_flight_override);
    let workflow_max_attempts = workflow_max_attempts();
    let workflow_lease_seconds = question_timeout
        .map(|timeout| timeout.as_secs().saturating_add(60).max(60))
        .unwrap_or(660);

    let pending_rows = rows
        .iter()
        .enumerate()
        .filter(|(_, row)| !completed.contains(&row.question_id))
        .map(|(idx, row)| (idx, row.clone()))
        .collect::<Vec<_>>();

    eprintln!(
        "[longmemeval] pending={} total={} workflow_queue={} question_timeout={} workflow_max_in_flight={} workflow_max_attempts={}",
        pending_rows.len(),
        total,
        workflow_queue_id.0,
        question_timeout
            .map(|timeout| format!("{}s", timeout.as_secs()))
            .unwrap_or_else(|| "disabled".to_string()),
        workflow_max_in_flight,
        workflow_max_attempts
    );

    let row_buffer = workflow_max_in_flight.min(pending_rows.len().max(1));
    let completed_hyps = futures::stream::iter(pending_rows)
        .map(|(idx, row)| {
            let file = file.clone();
            let run_root = run_root.clone();
            let embedder_factory = embedder_factory.clone();
            let distiller_factory = distiller_factory.clone();
            let consolidator_factory = consolidator_factory.clone();
            let chat_factory = chat_factory.clone();
            let answer_retry_factory = answer_retry_factory.clone();
            let planner_factory = planner_factory.clone();
            let reranker = reranker.clone();
            let debug_metadata = debug_metadata.clone();
            let memory_trace_sink = memory_trace_sink.clone();
            let policy = policy.clone();
            let consolidate_briefs = consolidate_briefs.clone();
            let stop_after_raw_embed = stop_after_raw_embed.clone();
            let ingest_diagnostic_mode = ingest_diagnostic_mode.clone();
            let workflow_queue = workflow_queue.clone();
            let workflow_queue_id = workflow_queue_id.clone();
            let debug_run_id = debug_run_id.clone();
            async move {
                let question_id = row.question_id.clone();
                let input_hash = workflow_input_hash(
                    &row,
                    routed,
                    answer_only,
                    *consolidate_briefs,
                    *ingest_diagnostic_mode,
                    &policy,
                );
                let started = Instant::now();
                eprintln!(
                    "[longmemeval] {}/{} {} {}",
                    idx + 1,
                    total,
                    question_id,
                    if routed { "process-routed" } else { "process" }
                );
                let worker_id = workflow_worker_id(&question_id);
                let queue_item = enqueue_and_claim_workflow_row(
                    workflow_queue.as_ref(),
                    workflow_queue_id.clone(),
                    &worker_id,
                    &question_id,
                    &input_hash,
                    routed,
                    workflow_lease_seconds,
                    workflow_max_in_flight,
                    workflow_max_attempts,
                    allow_terminal_reenqueue || answer_only,
                )
                .await?;
                let heartbeat = spawn_workflow_heartbeat(
                    workflow_queue.clone(),
                    queue_item.item_id.clone(),
                    worker_id.clone(),
                    workflow_lease_seconds,
                );
                let row_result = process_sqlite_row(
                    &row,
                    &run_root,
                    &*embedder_factory,
                    &*distiller_factory,
                    consolidator_factory.as_ref().clone(),
                    &*chat_factory,
                    answer_retry_factory.as_ref().clone(),
                    planner_factory.as_ref().clone(),
                    reranker.as_ref().clone(),
                    debug_metadata.as_ref().clone(),
                    memory_trace_sink.as_ref().clone(),
                    (*policy).clone(),
                    routed,
                    answer_only,
                    *consolidate_briefs,
                    *stop_after_raw_embed,
                    *ingest_diagnostic_mode,
                    &debug_run_id,
                );
                let hypothesis = match run_workflow_row(row_result, question_timeout).await {
                    Ok(hypothesis) => hypothesis,
                    Err(err) => {
                        // A single failed question (e.g. a transient empty/timed-out embedding that
                        // leaves recall with no usable query vector, or any other per-question recall
                        // error) must cost at most this one question — never abort the whole run. We
                        // record the queue item as FAILED (so a later pass can retry it), emit an
                        // "unavailable" hypothesis so scoring sees a row for this question, and return
                        // Ok so the buffered stream keeps draining the remaining questions. We do NOT
                        // call `complete` here — the item stays in the failed state recorded below.
                        heartbeat.abort();
                        fail_workflow_item(
                            workflow_queue.as_ref(),
                            &queue_item.item_id,
                            &worker_id,
                            &err.to_string(),
                            queue_item.attempt,
                        )
                        .await?;
                        eprintln!(
                            "[longmemeval] {}/{} {} recall-unavailable: {err}; marking question unavailable and continuing",
                            idx + 1,
                            total,
                            question_id
                        );
                        let hypothesis = BenchHypothesis {
                            question_id: question_id.clone(),
                            question_type: row.question_type.clone(),
                            question: row.question.clone(),
                            hypothesis: "UNAVAILABLE: recall failed for this question".to_string(),
                            debug_artifact: None,
                            router_initial: Some("recall-unavailable".to_string()),
                            router_final: Some("recall-unavailable".to_string()),
                            router_reason: Some(err.to_string()),
                        };
                        {
                            let mut file = file.lock().expect("hypothesis file lock");
                            writeln!(file, "{}", serde_json::to_string(&hypothesis)?)?;
                            file.flush()?;
                        }
                        return Ok::<_, anyhow::Error>(hypothesis);
                    }
                };
                {
                    let mut file = file.lock().expect("hypothesis file lock");
                    writeln!(file, "{}", serde_json::to_string(&hypothesis)?)?;
                    file.flush()?;
                }
                let complete_result = workflow_queue
                    .complete(&queue_item.item_id, &worker_id)
                    .await;
                heartbeat.abort();
                complete_result.map_err(queue_error)?;
                eprintln!(
                    "[longmemeval] {}/{} {} done elapsed={}s",
                    idx + 1,
                    total,
                    question_id,
                    started.elapsed().as_secs()
                );
                Ok::<_, anyhow::Error>(hypothesis)
            }
        })
        .map(tokio::spawn)
        .buffer_unordered(row_buffer)
        .map(|join_result| match join_result {
            Ok(row_result) => row_result,
            Err(err) => Err(anyhow::anyhow!("workflow task join failed: {err}")),
        })
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .collect::<anyhow::Result<Vec<_>>>()?;

    Ok(completed_hyps)
}

#[cfg(feature = "symbiotic-memory-adapter")]
#[derive(Clone)]
enum BenchMemoryStore {
    Sqlite(symbiotic_memory::storage::sqlite::SqliteStore),
    ZvecHybrid(symbiotic_memory::storage::zvec::ZvecHybridIndexedSqliteStore),
}

#[cfg(feature = "symbiotic-memory-adapter")]
#[derive(Clone, Debug, Deserialize, Serialize)]
struct ZvecIndexManifest {
    schema_version: u32,
    backend: String,
    source_hash: String,
    sqlite_sha256: String,
    dimensions: usize,
    record_count: usize,
    built_at: DateTime<Utc>,
}

#[cfg(feature = "symbiotic-memory-adapter")]
struct ZvecIndexCacheState {
    valid: bool,
    trusted_manifest: bool,
}

#[cfg(feature = "symbiotic-memory-adapter")]
#[derive(Clone, Debug)]
struct RecallIndexEnsureReport {
    action: &'static str,
    record_count: Option<usize>,
    metrics: BTreeMap<String, serde_json::Value>,
}

#[cfg(feature = "symbiotic-memory-adapter")]
impl BenchMemoryStore {
    async fn open_with_metrics(
        vault_dir: PathBuf,
        backend: String,
        dimensions: usize,
    ) -> anyhow::Result<(Self, BTreeMap<String, serde_json::Value>)> {
        tokio::task::spawn_blocking(move || {
            Self::open_with_metrics_blocking(&vault_dir, &backend, dimensions)
        })
        .await?
    }

    fn open_with_metrics_blocking(
        vault_dir: &Path,
        backend: &str,
        dimensions: usize,
    ) -> anyhow::Result<(Self, BTreeMap<String, serde_json::Value>)> {
        let sqlite_path = vault_dir.join("memory.sqlite");
        match backend {
            "zvec-hybrid" => {
                let (store, report) =
                    symbiotic_memory::storage::zvec::ZvecHybridIndexedSqliteStore::open_with_report(
                    &sqlite_path,
                    vault_dir.join("zvec-hybrid"),
                    dimensions,
                    )?;
                let mut metrics = BTreeMap::new();
                metrics.insert(
                    "store_sqlite_open_ms".to_string(),
                    serde_json::json!(report.sqlite_open_ms),
                );
                metrics.insert(
                    "store_zvec_total_ms".to_string(),
                    serde_json::json!(report.zvec_total_ms),
                );
                metrics.insert(
                    "store_zvec_init_ms".to_string(),
                    serde_json::json!(report.zvec_init_ms),
                );
                metrics.insert(
                    "store_zvec_collection_open_ms".to_string(),
                    serde_json::json!(report.collection_open_ms),
                );
                metrics.insert(
                    "store_zvec_schema_ms".to_string(),
                    serde_json::json!(report.schema_ms),
                );
                metrics.insert(
                    "store_zvec_collection_create_ms".to_string(),
                    serde_json::json!(report.collection_create_ms),
                );
                metrics.insert(
                    "store_zvec_collection_created".to_string(),
                    serde_json::json!(report.collection_created),
                );
                Ok((Self::ZvecHybrid(store), metrics))
            }
            _ => Ok((
                Self::Sqlite(symbiotic_memory::storage::sqlite::SqliteStore::open(
                    sqlite_path,
                )?),
                BTreeMap::new(),
            )),
        }
    }

    async fn ensure_recall_index(
        &self,
        vault_dir: &Path,
        source_hash: &str,
        cache_state: Option<&ZvecIndexCacheState>,
        incremental_ingest_completed: bool,
    ) -> anyhow::Result<RecallIndexEnsureReport> {
        match self {
            Self::Sqlite(_) => Ok(RecallIndexEnsureReport {
                action: "sqlite-noop",
                record_count: None,
                metrics: BTreeMap::new(),
            }),
            Self::ZvecHybrid(store) => {
                let Some(cache_state) = cache_state else {
                    let record_count = store.rebuild_index().await?;
                    return Ok(RecallIndexEnsureReport {
                        action: "rebuild-no-cache-state",
                        record_count: Some(record_count),
                        metrics: BTreeMap::new(),
                    });
                };
                if cache_state.valid {
                    return Ok(RecallIndexEnsureReport {
                        action: if cache_state.trusted_manifest {
                            "manifest-trusted"
                        } else {
                            "manifest-validated"
                        },
                        record_count: None,
                        metrics: BTreeMap::new(),
                    });
                }
                let mut metrics = BTreeMap::new();
                if incremental_ingest_completed {
                    if zvec_flush_before_recall() {
                        let step_started = Instant::now();
                        let store_for_flush = store.clone();
                        tokio::task::spawn_blocking(move || store_for_flush.index().flush())
                            .await??;
                        insert_elapsed_ms(
                            &mut metrics,
                            "recall_index_flush_ms",
                            step_started.elapsed(),
                        );
                        let step_started = Instant::now();
                        let store_for_count = store.clone();
                        let record_count = tokio::task::spawn_blocking(move || {
                            store_for_count.index().doc_count()
                        })
                        .await??;
                        insert_elapsed_ms(
                            &mut metrics,
                            "recall_index_doc_count_ms",
                            step_started.elapsed(),
                        );
                        if record_count == 0 {
                            anyhow::bail!(
                                "zvec-hybrid incremental ingest completed but produced an empty index"
                            );
                        }
                        let step_started = Instant::now();
                        let sqlite_sha256 = sha256_file(&vault_dir.join("memory.sqlite"))?;
                        insert_elapsed_ms(
                            &mut metrics,
                            "recall_index_sqlite_sha256_ms",
                            step_started.elapsed(),
                        );
                        let step_started = Instant::now();
                        write_zvec_index_manifest(
                            vault_dir,
                            &ZvecIndexManifest {
                                schema_version: ZVEC_INDEX_MANIFEST_SCHEMA_VERSION,
                                backend: "zvec-hybrid".to_string(),
                                source_hash: source_hash.to_string(),
                                sqlite_sha256,
                                dimensions: store.index().dimensions(),
                                record_count,
                                built_at: Utc::now(),
                            },
                        )?;
                        insert_elapsed_ms(
                            &mut metrics,
                            "recall_index_manifest_write_ms",
                            step_started.elapsed(),
                        );
                        return Ok(RecallIndexEnsureReport {
                            action: "incremental-flush",
                            record_count: Some(record_count),
                            metrics,
                        });
                    }
                    let step_started = Instant::now();
                    let store_for_count = store.clone();
                    let record_count =
                        tokio::task::spawn_blocking(move || store_for_count.index().doc_count())
                            .await??;
                    insert_elapsed_ms(
                        &mut metrics,
                        "recall_index_doc_count_ms",
                        step_started.elapsed(),
                    );
                    if record_count == 0 {
                        metrics.insert(
                            "recall_index_doc_count_zero_before_flush".to_string(),
                            serde_json::json!(true),
                        );
                    }
                    return Ok(RecallIndexEnsureReport {
                        action: "incremental-live",
                        record_count: Some(record_count),
                        metrics,
                    });
                }
                let step_started = Instant::now();
                let record_count = store.rebuild_index().await?;
                insert_elapsed_ms(
                    &mut metrics,
                    "recall_index_rebuild_ms",
                    step_started.elapsed(),
                );
                let step_started = Instant::now();
                let store_for_flush = store.clone();
                tokio::task::spawn_blocking(move || store_for_flush.index().flush()).await??;
                insert_elapsed_ms(
                    &mut metrics,
                    "recall_index_flush_ms",
                    step_started.elapsed(),
                );
                let step_started = Instant::now();
                let sqlite_sha256 = sha256_file(&vault_dir.join("memory.sqlite"))?;
                insert_elapsed_ms(
                    &mut metrics,
                    "recall_index_sqlite_sha256_ms",
                    step_started.elapsed(),
                );
                let step_started = Instant::now();
                write_zvec_index_manifest(
                    vault_dir,
                    &ZvecIndexManifest {
                        schema_version: ZVEC_INDEX_MANIFEST_SCHEMA_VERSION,
                        backend: "zvec-hybrid".to_string(),
                        source_hash: source_hash.to_string(),
                        sqlite_sha256,
                        dimensions: store.index().dimensions(),
                        record_count,
                        built_at: Utc::now(),
                    },
                )?;
                insert_elapsed_ms(
                    &mut metrics,
                    "recall_index_manifest_write_ms",
                    step_started.elapsed(),
                );
                Ok(RecallIndexEnsureReport {
                    action: "rebuild",
                    record_count: Some(record_count),
                    metrics,
                })
            }
        }
    }
}

#[cfg(feature = "symbiotic-memory-adapter")]
const ZVEC_INDEX_MANIFEST_SCHEMA_VERSION: u32 = 1;

#[cfg(feature = "symbiotic-memory-adapter")]
fn zvec_index_dir(vault_dir: &Path) -> PathBuf {
    vault_dir.join("zvec-hybrid")
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn zvec_index_manifest_path(vault_dir: &Path) -> PathBuf {
    zvec_index_dir(vault_dir).join("index-manifest.json")
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn prepare_zvec_index_cache(
    vault_dir: &Path,
    source_hash: &str,
    dimensions: usize,
    trust_valid_manifest: bool,
) -> anyhow::Result<ZvecIndexCacheState> {
    let manifest = read_zvec_index_manifest(vault_dir)?;
    let manifest_shape_valid = manifest
        .as_ref()
        .map(|manifest| {
            manifest.schema_version == ZVEC_INDEX_MANIFEST_SCHEMA_VERSION
                && manifest.backend == "zvec-hybrid"
                && manifest.source_hash == source_hash
                && manifest.dimensions == dimensions
                && manifest.record_count > 0
                && zvec_index_dir(vault_dir).is_dir()
        })
        .unwrap_or(false);
    if trust_valid_manifest && manifest_shape_valid {
        return Ok(ZvecIndexCacheState {
            valid: true,
            trusted_manifest: true,
        });
    }

    let sqlite_path = vault_dir.join("memory.sqlite");
    let sqlite_sha256 = if sqlite_path.is_file() {
        Some(sha256_file(&sqlite_path)?)
    } else {
        None
    };
    let valid = manifest_shape_valid
        && manifest
            .as_ref()
            .zip(sqlite_sha256.as_ref())
            .map(|(manifest, sqlite_sha256)| &manifest.sqlite_sha256 == sqlite_sha256)
            .unwrap_or(false);
    if !valid {
        remove_path_if_exists(&zvec_index_dir(vault_dir))?;
    }
    Ok(ZvecIndexCacheState {
        valid,
        trusted_manifest: false,
    })
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn strict_zvec_manifest_validation() -> bool {
    std::env::var("SYMEM_ZVEC_STRICT_MANIFEST")
        .ok()
        .map(|value| matches!(value.trim(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false)
}

/// SYMEM_IGNORE_SOURCE_HASH opt-out for the answer-only manifest gate. A field-name rename in
/// `SourceDocument` changes the serialized shape (and thus `source_shape_hash`) even when the
/// underlying source DATA is unchanged, which makes the answer-only path refuse to reuse a golden
/// vault. When this is set, the gate logs a warning and proceeds instead of refusing. Off by default.
#[cfg(feature = "symbiotic-memory-adapter")]
fn ignore_source_hash() -> bool {
    std::env::var("SYMEM_IGNORE_SOURCE_HASH")
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "on" | "yes"
            )
        })
        .unwrap_or(false)
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn zvec_flush_before_recall() -> bool {
    std::env::var("SYMEM_ZVEC_SKIP_FLUSH_BEFORE_RECALL")
        .ok()
        .map(|value| !matches!(value.trim(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(true)
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn read_zvec_index_manifest(vault_dir: &Path) -> anyhow::Result<Option<ZvecIndexManifest>> {
    let path = zvec_index_manifest_path(vault_dir);
    if !path.is_file() {
        return Ok(None);
    }
    let raw = fs::read_to_string(path)?;
    Ok(Some(serde_json::from_str(&raw)?))
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn write_zvec_index_manifest(vault_dir: &Path, manifest: &ZvecIndexManifest) -> anyhow::Result<()> {
    let path = zvec_index_manifest_path(vault_dir);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_vec_pretty(manifest)?)?;
    fs::rename(tmp, path)?;
    Ok(())
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn sha256_file(path: &Path) -> anyhow::Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 128 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn remove_path_if_exists(path: &Path) -> anyhow::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_dir() && !metadata.file_type().is_symlink() => {
            fs::remove_dir_all(path)?;
        }
        Ok(_) => {
            fs::remove_file(path)?;
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => return Err(err.into()),
    }
    Ok(())
}

#[cfg(feature = "symbiotic-memory-adapter")]
async fn record_adapter_stage(
    sink: Option<&Arc<dyn MemoryTraceSink>>,
    run_id: &str,
    question_id: &str,
    stage: &str,
    started_at: DateTime<Utc>,
    duration: Duration,
    metrics: BTreeMap<String, serde_json::Value>,
) {
    let Some(sink) = sink else {
        return;
    };
    let mut event = MemoryTraceEvent::native_stage(
        run_id.to_string(),
        question_id.to_string(),
        MemoryStage::Index,
        MemoryTraceEventKind::OperationSucceeded,
    );
    event.question_id = Some(question_id.to_string());
    event.source_id = Some(question_id.to_string());
    event.operation = MemoryTraceOperation::AdapterCall;
    event.stage = Some(stage.to_string());
    event.started_at = Some(started_at);
    event.finished_at = Some(Utc::now());
    event.duration_ms = Some(duration.as_millis().min(i64::MAX as u128) as i64);
    event.metrics = serde_json::Value::Object(metrics.into_iter().collect());
    if let Err(err) = sink.record_memory_event(event).await {
        eprintln!("[longmemeval] memory trace write failed for {question_id}: {err}");
    }
}

#[cfg(feature = "symbiotic-memory-adapter")]
async fn record_adapter_stage_started(
    sink: Option<&Arc<dyn MemoryTraceSink>>,
    run_id: &str,
    question_id: &str,
    stage: &str,
    metrics: BTreeMap<String, serde_json::Value>,
) {
    let Some(sink) = sink else {
        return;
    };
    let mut event = MemoryTraceEvent::native_stage(
        run_id.to_string(),
        question_id.to_string(),
        MemoryStage::Index,
        MemoryTraceEventKind::OperationStarted,
    );
    event.question_id = Some(question_id.to_string());
    event.source_id = Some(question_id.to_string());
    event.operation = MemoryTraceOperation::AdapterCall;
    event.stage = Some(stage.to_string());
    event.started_at = Some(Utc::now());
    event.metrics = serde_json::Value::Object(metrics.into_iter().collect());
    if let Err(err) = sink.record_memory_event(event).await {
        eprintln!("[longmemeval] memory trace write failed for {question_id}: {err}");
    }
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn insert_elapsed_ms(
    metrics: &mut BTreeMap<String, serde_json::Value>,
    key: &str,
    elapsed: Duration,
) {
    metrics.insert(
        key.to_string(),
        serde_json::json!(elapsed.as_millis().min(i64::MAX as u128) as i64),
    );
}

#[cfg(feature = "symbiotic-memory-adapter")]
#[async_trait::async_trait]
impl MemoryStore for BenchMemoryStore {
    async fn upsert_receipt(
        &self,
        receipt: symbiotic_memory::types::RawArchiveReceipt,
    ) -> Result<(), symbiotic_memory::storage::StoreError> {
        match self {
            Self::Sqlite(store) => store.upsert_receipt(receipt).await,
            Self::ZvecHybrid(store) => store.upsert_receipt(receipt).await,
        }
    }

    async fn upsert_turns(
        &self,
        turns: Vec<SourceTurn>,
        embeddings: Vec<Vec<f32>>,
    ) -> Result<(), symbiotic_memory::storage::StoreError> {
        match self {
            Self::Sqlite(store) => store.upsert_turns(turns, embeddings).await,
            Self::ZvecHybrid(store) => store.upsert_turns(turns, embeddings).await,
        }
    }

    async fn upsert_facts(
        &self,
        facts: Vec<symbiotic_memory::types::MemoryFact>,
        embeddings: Vec<Vec<f32>>,
    ) -> Result<(), symbiotic_memory::storage::StoreError> {
        match self {
            Self::Sqlite(store) => store.upsert_facts(facts, embeddings).await,
            Self::ZvecHybrid(store) => store.upsert_facts(facts, embeddings).await,
        }
    }

    async fn receipts(
        &self,
    ) -> Result<
        Vec<symbiotic_memory::types::RawArchiveReceipt>,
        symbiotic_memory::storage::StoreError,
    > {
        match self {
            Self::Sqlite(store) => store.receipts().await,
            Self::ZvecHybrid(store) => store.receipts().await,
        }
    }

    async fn active_facts(
        &self,
    ) -> Result<Vec<symbiotic_memory::types::MemoryFact>, symbiotic_memory::storage::StoreError>
    {
        match self {
            Self::Sqlite(store) => store.active_facts().await,
            Self::ZvecHybrid(store) => store.active_facts().await,
        }
    }

    async fn active_base_facts(
        &self,
    ) -> Result<Vec<symbiotic_memory::types::MemoryFact>, symbiotic_memory::storage::StoreError>
    {
        match self {
            Self::Sqlite(store) => store.active_base_facts().await,
            Self::ZvecHybrid(store) => store.active_base_facts().await,
        }
    }

    async fn clear_briefs(&self) -> Result<u64, symbiotic_memory::storage::StoreError> {
        match self {
            Self::Sqlite(store) => store.clear_briefs().await,
            Self::ZvecHybrid(store) => store.clear_briefs().await,
        }
    }

    async fn turns(&self) -> Result<Vec<SourceTurn>, symbiotic_memory::storage::StoreError> {
        match self {
            Self::Sqlite(store) => store.turns().await,
            Self::ZvecHybrid(store) => store.turns().await,
        }
    }

    async fn fact_search(
        &self,
        query_embedding: &[f32],
        query: &str,
        top_k: usize,
    ) -> Result<Vec<FactEvidence>, symbiotic_memory::storage::StoreError> {
        match self {
            Self::Sqlite(store) => store.fact_search(query_embedding, query, top_k).await,
            Self::ZvecHybrid(store) => store.fact_search(query_embedding, query, top_k).await,
        }
    }

    async fn raw_turn_search(
        &self,
        query_embedding: &[f32],
        query: &str,
        top_k: usize,
    ) -> Result<Vec<RawTurnEvidence>, symbiotic_memory::storage::StoreError> {
        match self {
            Self::Sqlite(store) => store.raw_turn_search(query_embedding, query, top_k).await,
            Self::ZvecHybrid(store) => store.raw_turn_search(query_embedding, query, top_k).await,
        }
    }

    async fn get_turns_by_turn_ids(
        &self,
        ids: &[String],
    ) -> Result<Vec<RawTurnEvidence>, symbiotic_memory::storage::StoreError> {
        match self {
            Self::Sqlite(store) => store.get_turns_by_turn_ids(ids).await,
            Self::ZvecHybrid(store) => store.get_turns_by_turn_ids(ids).await,
        }
    }

    async fn get_facts_by_source_turn_ids(
        &self,
        turn_ids: &[String],
    ) -> Result<Vec<FactEvidence>, symbiotic_memory::storage::StoreError> {
        match self {
            Self::Sqlite(store) => store.get_facts_by_source_turn_ids(turn_ids).await,
            Self::ZvecHybrid(store) => store.get_facts_by_source_turn_ids(turn_ids).await,
        }
    }

    async fn get_facts_by_memory_ids(
        &self,
        ids: &[String],
    ) -> Result<Vec<FactEvidence>, symbiotic_memory::storage::StoreError> {
        match self {
            Self::Sqlite(store) => store.get_facts_by_memory_ids(ids).await,
            Self::ZvecHybrid(store) => store.get_facts_by_memory_ids(ids).await,
        }
    }
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn store_backend_label(run_root: &Path) -> &'static str {
    let marker = run_root.join(".store-zvec");
    if !marker.exists() {
        return "zvec-hybrid";
    }
    let Ok(raw) = std::fs::read_to_string(marker) else {
        return "zvec-hybrid";
    };
    match raw.trim() {
        "sqlite" => "sqlite",
        "zvec-hybrid" | "" => "zvec-hybrid",
        _ => "zvec-hybrid",
    }
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn spawn_workflow_heartbeat(
    queue: Arc<dyn QueueBackend>,
    item_id: QueueItemId,
    worker_id: String,
    lease_seconds: u64,
) -> tokio::task::JoinHandle<()> {
    let interval_seconds = (lease_seconds / 3).clamp(5, 60);
    tokio::spawn(async move {
        let interval = Duration::from_secs(interval_seconds);
        loop {
            tokio::time::sleep(interval).await;
            if let Err(err) = queue.heartbeat(&item_id, &worker_id, lease_seconds).await {
                eprintln!(
                    "[longmemeval] workflow heartbeat stopped for {}: {}",
                    item_id.0, err
                );
                break;
            }
        }
    })
}

#[cfg(feature = "symbiotic-memory-adapter")]
async fn process_sqlite_row<E, D, C>(
    row: &LongMemEvalRecord,
    run_root: &Path,
    embedder_factory: &(impl Fn() -> E + Send + Sync),
    distiller_factory: &(impl Fn() -> D + Send + Sync),
    consolidator_factory: Option<Arc<dyn Fn() -> Arc<dyn Distiller> + Send + Sync>>,
    chat_factory: &(impl Fn() -> C + Send + Sync),
    answer_retry_factory: Option<Arc<dyn Fn() -> Arc<dyn ChatProvider> + Send + Sync>>,
    planner_factory: Option<Arc<dyn Fn() -> Arc<dyn QueryPlanner> + Send + Sync>>,
    reranker: RerankCascade,
    debug_metadata: Option<BenchDebugMetadata>,
    memory_trace_sink: Option<Arc<dyn MemoryTraceSink>>,
    policy: symbiotic_memory::config::RecallPolicy,
    routed: bool,
    answer_only: bool,
    consolidate_briefs: bool,
    stop_after_raw_embed: bool,
    ingest_diagnostic_mode: IngestDiagnosticMode,
    debug_run_id: &str,
) -> anyhow::Result<BenchHypothesis>
where
    E: EmbeddingProvider + Clone + Send + Sync + 'static,
    D: Distiller + 'static,
    C: ChatProvider + 'static,
{
    let setup_started_at = Utc::now();
    let setup_started = Instant::now();
    let mut setup_metrics = BTreeMap::<String, serde_json::Value>::new();
    let store_backend = store_backend_label(run_root);
    setup_metrics.insert(
        "store_backend".to_string(),
        serde_json::json!(store_backend),
    );
    record_adapter_stage_started(
        memory_trace_sink.as_ref(),
        debug_run_id,
        &row.question_id,
        "pre_capture_setup",
        setup_metrics.clone(),
    )
    .await;
    let ingest_diagnostic_mode = if stop_after_raw_embed || raw_embed_only_diagnostic() {
        IngestDiagnosticMode::RawEmbedOnly
    } else {
        ingest_diagnostic_mode
    };
    let vault_dir = run_root.join("vaults").join(&row.question_id);
    let step_started = Instant::now();
    fs::create_dir_all(&vault_dir)?;
    insert_elapsed_ms(
        &mut setup_metrics,
        "create_vault_dir_ms",
        step_started.elapsed(),
    );

    let step_started = Instant::now();
    let source = longmemeval_to_source(&row);
    let source_hash = source_shape_hash(&source)?;
    insert_elapsed_ms(&mut setup_metrics, "source_hash_ms", step_started.elapsed());

    let step_started = Instant::now();
    let manifest_path = vault_dir.join("manifest.json");
    let loaded_manifest = MemoryRunManifest::load(&manifest_path)?;
    if answer_only && loaded_manifest.is_none() {
        anyhow::bail!(
            "answer-only run requires an existing manifest for vault {}",
            row.question_id
        );
    }
    let mut manifest = loaded_manifest.unwrap_or_else(|| {
        let mut manifest = MemoryRunManifest::new(
            row.question_id.clone(),
            source_hash.clone(),
            "longmemeval-v1",
        );
        manifest.index_backend = Some(store_backend.to_string());
        manifest
    });
    if manifest.source_hash != source_hash {
        // SYMEM_IGNORE_SOURCE_HASH opt-out: a field-name rename in SourceDocument changes the
        // serialized shape (and thus this hash) even though the underlying source DATA is unchanged.
        // When set, log a warning and PROCEED reusing the existing vault instead of refusing. The
        // stored manifest source_hash is left untouched (we do not rewrite it), so the check still
        // fires for anyone who has not opted out.
        if ignore_source_hash() {
            eprintln!(
                "[longmemeval] WARNING: vault {} manifest source hash mismatch (stored={} computed={}); SYMEM_IGNORE_SOURCE_HASH set, proceeding with existing vault",
                row.question_id, manifest.source_hash, source_hash
            );
        } else {
            anyhow::bail!(
                "vault {} manifest source hash changed; use a fresh run root",
                row.question_id
            );
        }
    }
    manifest.index_backend = Some(store_backend.to_string());
    manifest.save(&manifest_path)?;
    insert_elapsed_ms(&mut setup_metrics, "manifest_ms", step_started.elapsed());

    let step_started = Instant::now();
    let dimensions = embedder_factory().dimensions();
    insert_elapsed_ms(
        &mut setup_metrics,
        "embedding_dimensions_ms",
        step_started.elapsed(),
    );

    let step_started = Instant::now();
    let mut zvec_cache_state = if store_backend == "zvec-hybrid" {
        Some(prepare_zvec_index_cache(
            &vault_dir,
            &source_hash,
            dimensions,
            answer_only && !strict_zvec_manifest_validation(),
        )?)
    } else {
        None
    };
    insert_elapsed_ms(&mut setup_metrics, "zvec_cache_ms", step_started.elapsed());
    setup_metrics.insert(
        "zvec_manifest_trusted".to_string(),
        serde_json::json!(
            zvec_cache_state
                .as_ref()
                .map(|state| state.trusted_manifest)
                .unwrap_or(false)
        ),
    );

    let step_started = Instant::now();
    let (store, store_open_metrics) = BenchMemoryStore::open_with_metrics(
        vault_dir.clone(),
        store_backend.to_string(),
        dimensions,
    )
    .await?;
    insert_elapsed_ms(&mut setup_metrics, "store_open_ms", step_started.elapsed());
    setup_metrics.extend(store_open_metrics);

    let step_started = Instant::now();
    let mut existing_turns = store.turns().await?;
    let mut existing_facts = store.active_facts().await?;
    // Capture the counts we actually need downstream, then for answer-only — which only ever reads
    // these counts/emptiness below, never the data itself — free the embeddings immediately instead
    // of carrying ~100MB/vault through recall+answer. Under buffer_unordered concurrency that
    // retention was the multi-GB blowup. Re-embed needs the full vectors, so keep them when reembed.
    let existing_turn_count = existing_turns.len();
    let existing_fact_count = existing_facts.len();
    if answer_only && !reembed_mode() {
        existing_turns = Vec::new();
        existing_facts = Vec::new();
    }
    insert_elapsed_ms(
        &mut setup_metrics,
        "load_existing_ms",
        step_started.elapsed(),
    );
    setup_metrics.insert(
        "store_backend".to_string(),
        serde_json::json!(store_backend),
    );
    setup_metrics.insert(
        "existing_turn_count".to_string(),
        serde_json::json!(existing_turn_count),
    );
    setup_metrics.insert(
        "existing_fact_count".to_string(),
        serde_json::json!(existing_fact_count),
    );
    record_adapter_stage(
        memory_trace_sink.as_ref(),
        debug_run_id,
        &row.question_id,
        "pre_capture_setup",
        setup_started_at,
        setup_started.elapsed(),
        setup_metrics,
    )
    .await;

    // Non-embed redo stages: invalidate the target stage(s) so the ingest path below re-runs them,
    // reusing every valid upstream stage. reweave re-runs consolidation (slotted ledgers are cleanly
    // superseded by slot-key on upsert); index rebuilds only. embed is handled in its own branch.
    let mut redo_invalidated_stage = false;
    match redo_stage() {
        Some("reweave") => {
            // Re-running consolidation requires removing the prior briefs first (otherwise the
            // consolidator reweaves over its own old output and stale briefs contaminate recall).
            // Briefs now live in a dedicated table, so clear them wholesale and invalidate the
            // consolidate + index stages; the ingest path below re-runs consolidation with the new
            // prompt and reuses the base distilled facts untouched.
            let cleared = store.clear_briefs().await?;
            eprintln!(
                "[longmemeval] {} SYMEM_REDO=reweave cleared {cleared} prior briefs",
                row.question_id
            );
            manifest.stages.remove(&MemoryStage::Consolidate);
            manifest.stages.remove(&MemoryStage::Index);
            redo_invalidated_stage = true;
        }
        Some("index") => {
            manifest.stages.remove(&MemoryStage::Index);
            redo_invalidated_stage = true;
            // Invalidate the cached zvec recall index so ensure_recall_index below does NOT trust
            // the stale on-disk index (prepare_zvec_index_cache trusts it when the sqlite hash is
            // unchanged, which it is for an index-only redo) and instead does a FULL rebuild. The
            // full-rebuild path (rebuild_index, which re-upserts every recall record from sqlite,
            // the source of truth) avoids the empty-index hard-bail that the incremental-flush path
            // hits when the linked-vault prep had deleted the zvec dir on top of an empty index.
            if let Some(state) = zvec_cache_state.as_mut() {
                state.valid = false;
                state.trusted_manifest = false;
            }
        }
        Some("distill") => {
            anyhow::bail!(
                "--redo distill on an existing vault is not supported (base facts would duplicate); \
                 run a fresh ingest to re-distill"
            );
        }
        _ => {}
    }
    if redo_invalidated_stage {
        // Persist the invalidation: the ingest pipeline below reloads the manifest FROM DISK and
        // skips any stage still marked Succeeded there. Without this save the on-disk stage stays
        // Succeeded, the rebuild is skipped, and for reweave the briefs are wiped (clear_briefs
        // above) WITHOUT being re-woven. Saving forces the pipeline to actually re-run the stage.
        manifest.save(&manifest_path)?;
    }

    let mut incremental_ingest_completed = false;
    if reembed_mode() {
        // Re-embed an already-distilled vault: rebuild each fact's embedding text from its metadata
        // (content + subjects + slot_key + tags + event_time/valid_from dates) and re-embed facts +
        // raw turns with the current embedder, overwriting the stored vectors. Reuses the persisted
        // distill/consolidation — no LLM re-distill. The index then rebuilds at the current dims.
        if existing_facts.is_empty() {
            anyhow::bail!(
                "--re-embed requires an ingested vault with facts for {}",
                row.question_id
            );
        }
        let reembed_started_at = Utc::now();
        let reembed_started = Instant::now();
        let embedder = embedder_factory();
        let dims_profile = format!("dimensions:{}", embedder.dimensions());
        let mut facts = existing_facts.clone();
        for fact in facts.iter_mut() {
            // Clear the distill-time search_text so enrich rebuilds it from current metadata logic.
            fact.search_text = None;
            fact.embedding_profile = Some(dims_profile.clone());
            symbiotic_memory::enrich_fact_search_metadata(fact);
        }
        let fact_texts: Vec<String> = facts
            .iter()
            .map(symbiotic_memory::fact_retrieval_text)
            .collect();
        let fact_embeddings = embed_texts_in_chunks(&embedder, &fact_texts).await?;
        anyhow::ensure!(
            fact_embeddings.len() == facts.len(),
            "re-embed produced {} embeddings for {} facts",
            fact_embeddings.len(),
            facts.len()
        );
        let reembedded_fact_count = facts.len();
        store.upsert_facts(facts, fact_embeddings).await?;
        // Raw turns are only re-embedded when explicitly requested (dims/model change). By default
        // the prep copied the source zvec index forward, so the existing turn vectors are already
        // present — recomputing them would re-call the embedding API for identical vectors (the bulk
        // cost of a re-embed). See `reembed_turns`.
        let mut reembedded_turn_count = 0usize;
        if reembed_turns() && !existing_turns.is_empty() {
            let turn_texts: Vec<String> = existing_turns
                .iter()
                .map(|turn| turn.text.clone())
                .collect();
            let turn_embeddings = embed_texts_in_chunks(&embedder, &turn_texts).await?;
            anyhow::ensure!(
                turn_embeddings.len() == existing_turns.len(),
                "re-embed produced {} embeddings for {} turns",
                turn_embeddings.len(),
                existing_turns.len()
            );
            reembedded_turn_count = existing_turns.len();
            store
                .upsert_turns(existing_turns.clone(), turn_embeddings)
                .await?;
        }
        record_adapter_stage(
            memory_trace_sink.as_ref(),
            debug_run_id,
            &row.question_id,
            "re_embed",
            reembed_started_at,
            reembed_started.elapsed(),
            BTreeMap::from([
                (
                    "reembedded_fact_count".to_string(),
                    serde_json::json!(reembedded_fact_count),
                ),
                (
                    "reembedded_turn_count".to_string(),
                    serde_json::json!(reembedded_turn_count),
                ),
                (
                    "embedding_dimensions".to_string(),
                    serde_json::json!(embedder.dimensions()),
                ),
            ]),
        )
        .await;
        // upsert_facts/upsert_turns already wrote the new vectors into the zvec index, so take the
        // fast incremental-flush path in ensure_recall_index instead of a full 12s rebuild.
        incremental_ingest_completed = true;
    } else if answer_only {
        if existing_turn_count == 0
            || existing_fact_count == 0
            || !post_ingest_complete(&manifest, consolidate_briefs)
        {
            anyhow::bail!(
                "answer-only run requires a complete ingested vault for {}",
                row.question_id
            );
        }
    } else if !post_ingest_complete(&manifest, consolidate_briefs) {
        let mut ingest =
            IngestPipeline::new(store.clone(), embedder_factory(), distiller_factory())
                .with_archive_root(&vault_dir)
                .with_manifest_path(&manifest_path, "longmemeval-v1")
                .with_optional_trace_sink(memory_trace_sink.clone())
                .with_diagnostic_mode(ingest_diagnostic_mode);
        if consolidate_briefs {
            // Briefs (extractive-brief-v1) are deprecated/killed — never generate them.
            if let Some(consolidator_factory) = consolidator_factory {
                ingest = ingest.with_consolidator(consolidator_factory());
            }
        }
        ingest.ingest(source.clone()).await?;
        incremental_ingest_completed = true;
        manifest = MemoryRunManifest::load(&manifest_path)?
            .ok_or_else(|| anyhow::anyhow!("ingest did not write {}", manifest_path.display()))?;
    }
    let pre_recall_started_at = Utc::now();
    let pre_recall_started = Instant::now();
    let mut pre_recall_metrics = BTreeMap::<String, serde_json::Value>::new();
    pre_recall_metrics.insert(
        "store_backend".to_string(),
        serde_json::json!(store_backend),
    );
    pre_recall_metrics.insert(
        "incremental_ingest_completed".to_string(),
        serde_json::json!(incremental_ingest_completed),
    );
    record_adapter_stage_started(
        memory_trace_sink.as_ref(),
        debug_run_id,
        &row.question_id,
        "pre_recall_setup",
        pre_recall_metrics.clone(),
    )
    .await;
    if ingest_diagnostic_mode != IngestDiagnosticMode::None {
        let step_started = Instant::now();
        let turn_count = store.turns().await?.len();
        let fact_count = manifest
            .stages
            .get(&MemoryStage::DistillWindow)
            .and_then(|stage| stage.metrics.get("fact_count"))
            .and_then(serde_json::Value::as_u64)
            .unwrap_or_default();
        insert_elapsed_ms(
            &mut pre_recall_metrics,
            "load_counts_ms",
            step_started.elapsed(),
        );
        pre_recall_metrics.insert("fact_count".to_string(), serde_json::json!(fact_count));
        pre_recall_metrics.insert("turn_count".to_string(), serde_json::json!(turn_count));
        pre_recall_metrics.insert(
            "store_backend".to_string(),
            serde_json::json!(store_backend),
        );
        pre_recall_metrics.insert(
            "ingest_diagnostic_mode".to_string(),
            serde_json::json!(ingest_diagnostic_mode.as_str()),
        );
        pre_recall_metrics.insert(
            "incremental_ingest_completed".to_string(),
            serde_json::json!(incremental_ingest_completed),
        );
        record_adapter_stage(
            memory_trace_sink.as_ref(),
            debug_run_id,
            &row.question_id,
            "pre_recall_setup",
            pre_recall_started_at,
            pre_recall_started.elapsed(),
            pre_recall_metrics,
        )
        .await;
        let hypothesis = BenchHypothesis {
            question_id: row.question_id.clone(),
            question_type: row.question_type.clone(),
            question: row.question.clone(),
            hypothesis: format!(
                "DIAGNOSTIC_STOP_AFTER_{}",
                ingest_diagnostic_mode
                    .as_str()
                    .replace('-', "_")
                    .to_ascii_uppercase()
            ),
            debug_artifact: None,
            router_initial: Some(format!("diagnostic-{}", ingest_diagnostic_mode.as_str())),
            router_final: Some(format!("diagnostic-{}", ingest_diagnostic_mode.as_str())),
            router_reason: Some(format!(
                "stopped after {} ingest isolate",
                ingest_diagnostic_mode.as_str()
            )),
        };
        write_json_atomic(&vault_dir.join("answer.json"), &hypothesis)?;
        manifest.save(&manifest_path)?;
        return Ok(hypothesis);
    }
    let step_started = Instant::now();
    let (fact_count, turn_count) = if answer_only {
        (existing_fact_count, existing_turn_count)
    } else {
        (
            store.active_facts().await?.len(),
            store.turns().await?.len(),
        )
    };
    insert_elapsed_ms(
        &mut pre_recall_metrics,
        "load_counts_ms",
        step_started.elapsed(),
    );
    if fact_count == 0 {
        anyhow::bail!(
            "vault {} produced zero active facts after ingest",
            row.question_id
        );
    }
    let step_started = Instant::now();
    // For SYMEM_REDO=index, force a FULL index rebuild (rebuild_index re-upserts every recall
    // record from sqlite) instead of the incremental-flush path. The re-ingest above set
    // incremental_ingest_completed = true, but incremental-flush hard-bails on an empty index if
    // the linked-vault prep deleted the zvec dir; the full rebuild is robust and is the whole point
    // of an index-only redo.
    let ensure_index_incremental = incremental_ingest_completed && redo_stage() != Some("index");
    let recall_index_report = store
        .ensure_recall_index(
            &vault_dir,
            &source_hash,
            zvec_cache_state.as_ref(),
            ensure_index_incremental,
        )
        .await?;
    insert_elapsed_ms(
        &mut pre_recall_metrics,
        "ensure_recall_index_ms",
        step_started.elapsed(),
    );
    pre_recall_metrics.insert(
        "recall_index_action".to_string(),
        serde_json::json!(recall_index_report.action),
    );
    pre_recall_metrics.extend(recall_index_report.metrics);
    if let Some(record_count) = recall_index_report.record_count {
        pre_recall_metrics.insert(
            "recall_index_record_count".to_string(),
            serde_json::json!(record_count),
        );
    }
    pre_recall_metrics.insert("fact_count".to_string(), serde_json::json!(fact_count));
    pre_recall_metrics.insert("turn_count".to_string(), serde_json::json!(turn_count));
    pre_recall_metrics.insert(
        "store_backend".to_string(),
        serde_json::json!(store_backend),
    );
    pre_recall_metrics.insert(
        "incremental_ingest_completed".to_string(),
        serde_json::json!(incremental_ingest_completed),
    );
    record_adapter_stage(
        memory_trace_sink.as_ref(),
        debug_run_id,
        &row.question_id,
        "pre_recall_setup",
        pre_recall_started_at,
        pre_recall_started.elapsed(),
        pre_recall_metrics,
    )
    .await;

    let answer_retry_chat = answer_retry_factory.as_ref().map(|factory| factory());
    let RerankCascade {
        main: reranker_main,
        stage1: reranker_stage1,
        stage1_top_x: rerank_stage1_top_x,
    } = reranker;
    // Gold-oracle mode: build the answerer's context from ONLY the gold-session turns and hand it to
    // the engine as a forced context. Recall still runs (so the question-debug profile/plan stay
    // populated) but its retrieved evidence is discarded before the answerer — the reader sees nothing
    // but clean gold. None outside oracle mode (or if the record has no resolvable gold sessions),
    // which leaves the normal recall→rerank→answer path untouched.
    let gold_oracle_context = if oracle_gold_enabled() {
        // Always Some in oracle mode — empty for abstention questions (forces an abstain), never a
        // fallback to the noisy recall path.
        build_gold_oracle_context(&row)
    } else {
        None
    };
    let mut engine = RecallEngine::new(store, embedder_factory(), chat_factory(), policy)
        .with_optional_answer_retry_chat(answer_retry_chat)
        .with_optional_reranker(reranker_main)
        .with_optional_reranker_stage1(reranker_stage1.clone())
        .with_rerank_stage1_top_x(reranker_stage1.is_some().then_some(rerank_stage1_top_x))
        .with_optional_trace_sink(memory_trace_sink.clone())
        .with_optional_forced_context(gold_oracle_context)
        .with_trace_context(RecallTraceContext::new(
            row.question_id.clone(),
            row.question_id.clone(),
            row.question_id.clone(),
        ));
    if let Some(planner_factory) = planner_factory {
        engine = engine.with_query_planner(planner_factory());
    }
    let workflow_hash = workflow_input_hash(
        &row,
        routed,
        answer_only,
        consolidate_briefs,
        IngestDiagnosticMode::None,
        &engine.policy,
    );
    manifest.begin(MemoryStage::Answer, workflow_hash.clone());
    manifest.save(&manifest_path)?;
    let reference_date = longmemeval_answer_reference_datetime(row);
    let recall_debug = match engine
        .answer_debug_with_reference_date(&row.question, Some(reference_date.as_str()))
        .await
    {
        Ok(value) => value,
        Err(err) => {
            manifest.fail(MemoryStage::Answer, err.to_string());
            manifest.save(&manifest_path)?;
            return Err(err.into());
        }
    };
    let answer_text = recall_debug.final_answer.text.clone();
    let recall_profile = recall_debug.recall_profile.clone();
    let recall_debug = Some(recall_debug);
    let debug_artifact = recall_debug.as_ref().map(|_| {
        let snapshot_path = question_debug_snapshot_path(&vault_dir, debug_run_id);
        snapshot_path
            .strip_prefix(run_root)
            .unwrap_or(&snapshot_path)
            .display()
            .to_string()
    });
    let hypothesis = BenchHypothesis {
        question_id: row.question_id.clone(),
        question_type: row.question_type.clone(),
        question: row.question.clone(),
        hypothesis: answer_text,
        debug_artifact,
        router_initial: Some(recall_profile.clone()),
        router_final: Some(recall_profile),
        router_reason: Some("memory recall profile".to_string()),
    };
    if let Some(recall_debug) = recall_debug.as_ref() {
        write_question_debug(
            run_root,
            &vault_dir,
            &row,
            source.turns.len(),
            turn_count,
            fact_count,
            routed,
            answer_only,
            consolidate_briefs,
            workflow_hash.clone(),
            recall_debug,
            &hypothesis,
            debug_metadata.as_ref(),
            debug_run_id,
        )?;
    }
    write_json_atomic(&vault_dir.join("answer.json"), &hypothesis)?;
    manifest.succeed(
        MemoryStage::Answer,
        stable_hash_json(&hypothesis)?,
        BTreeMap::from([("fact_count".to_string(), serde_json::json!(fact_count))]),
    );
    manifest.save(&manifest_path)?;
    Ok(hypothesis)
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn write_question_debug(
    run_root: &Path,
    vault_dir: &Path,
    row: &LongMemEvalRecord,
    source_turn_count: usize,
    indexed_turn_count: usize,
    fact_count: usize,
    routed: bool,
    answer_only: bool,
    consolidate_briefs: bool,
    workflow_input_hash: String,
    recall_debug: &RecallAnswerDebug,
    hypothesis: &BenchHypothesis,
    debug_metadata: Option<&BenchDebugMetadata>,
    debug_run_id: &str,
) -> anyhow::Result<String> {
    let debug_dir = vault_dir.join("debug");
    fs::create_dir_all(&debug_dir)?;
    let mut provider_trace_artifacts = serde_json::Map::new();
    if let Some(metadata) = debug_metadata {
        for key in [
            "model_traces_jsonl",
            "model_queue_sqlite",
            "model_queue_traces_jsonl",
            "response_cache_dir",
        ] {
            if let Some(path) = metadata.trace_artifacts.get(key) {
                provider_trace_artifacts
                    .insert(key.to_string(), serde_json::Value::String(path.clone()));
            }
        }
    }
    let debug = serde_json::json!({
        "schema_version": 1,
        "question": {
            "id": row.question_id,
            "type": row.question_type,
            "text": row.question,
            "date": row.question_date,
            "gold_answer": row.answer,
        },
        "source": {
            "haystack_dates": row.haystack_dates,
            "haystack_session_ids": row.haystack_session_ids,
            "haystack_session_count": row.haystack_sessions.len(),
            "source_turn_count": source_turn_count,
            "indexed_turn_count": indexed_turn_count,
        },
        "workflow": {
            "routed": routed,
            "answer_only": answer_only,
            "consolidate_briefs": consolidate_briefs,
            "input_hash": workflow_input_hash,
        },
        "ingest": {
            "active_fact_count": fact_count,
            "manifest_path": "manifest.json",
        },
        "runtime": {
            "bench_owned_metadata": debug_metadata,
            "trace_note": "Answerer calls inline their returned usage/cache tokens when the provider returns them; full provider queue/model events remain in the trace artifacts.",
        },
        "recall": recall_debug,
        "hypothesis": hypothesis,
        "provider_trace_artifacts": provider_trace_artifacts,
        "scoring": serde_json::Value::Null,
    });
    let latest_path = debug_dir.join("question-debug.json");
    write_json_atomic(&latest_path, &debug)?;

    let snapshot_path = question_debug_snapshot_path(vault_dir, debug_run_id);
    if let Some(parent) = snapshot_path.parent() {
        fs::create_dir_all(parent)?;
    }
    write_json_atomic(&snapshot_path, &debug)?;
    Ok(snapshot_path
        .strip_prefix(run_root)
        .unwrap_or(&snapshot_path)
        .display()
        .to_string())
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn question_debug_snapshot_path(vault_dir: &Path, debug_run_id: &str) -> std::path::PathBuf {
    vault_dir
        .join("debug")
        .join("hypotheses")
        .join(debug_run_id)
        .join("question-debug.json")
}

#[cfg(feature = "symbiotic-memory-adapter")]
#[allow(dead_code)]
async fn run_longmemeval_sqlite_sequential<E, D, C>(
    rows: &[LongMemEvalRecord],
    run_root: impl AsRef<Path>,
    embedder_factory: impl Fn() -> E,
    distiller_factory: impl Fn() -> D,
    chat_factory: impl Fn() -> C,
    policy: symbiotic_memory::config::RecallPolicy,
    out_path: impl AsRef<Path>,
    routed: bool,
) -> anyhow::Result<Vec<BenchHypothesis>>
where
    E: EmbeddingProvider + Clone + 'static,
    D: Distiller,
    C: ChatProvider,
{
    use symbiotic_memory::storage::sqlite::SqliteStore;

    if let Some(parent) = out_path.as_ref().parent() {
        fs::create_dir_all(parent)?;
    }
    fs::create_dir_all(run_root.as_ref())?;
    let mut out = read_existing_hypotheses(out_path.as_ref())?;
    let mut completed: BTreeSet<_> = out.iter().map(|h| h.question_id.clone()).collect();
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(out_path.as_ref())?;

    for (idx, row) in rows.iter().enumerate() {
        if completed.contains(&row.question_id) {
            eprintln!(
                "[longmemeval] {}/{} {} skipped",
                idx + 1,
                rows.len(),
                row.question_id
            );
            continue;
        }
        eprintln!(
            "[longmemeval] {}/{} {} {}",
            idx + 1,
            rows.len(),
            row.question_id,
            if routed { "process-routed" } else { "process" }
        );
        let vault_dir = run_root.as_ref().join("vaults").join(&row.question_id);
        fs::create_dir_all(&vault_dir)?;
        let store = SqliteStore::open(vault_dir.join("memory.sqlite"))?;
        let existing_turns = store.turns().await?;
        let existing_facts = store.active_facts().await?;
        if existing_turns.is_empty() {
            let ingest =
                IngestPipeline::new(store.clone(), embedder_factory(), distiller_factory())
                    .with_archive_root(&vault_dir)
                    .with_manifest_path(vault_dir.join("manifest.json"), "longmemeval-v1");
            ingest.ingest(longmemeval_to_source(row)).await?;
        } else if existing_facts.is_empty() {
            let ingest =
                IngestPipeline::new(store.clone(), embedder_factory(), distiller_factory())
                    .with_archive_root(&vault_dir)
                    .with_manifest_path(vault_dir.join("manifest.json"), "longmemeval-v1");
            ingest.ingest(longmemeval_to_source(row)).await?;
        }
        let engine = RecallEngine::new(store, embedder_factory(), chat_factory(), policy.clone());
        let reference_date = longmemeval_answer_reference_datetime(row);
        // One bad question (e.g. a transient empty/timed-out embedding that leaves recall with no
        // usable query vector) must cost at most this one question, never the whole run. Catch the
        // per-question error, log it, and emit an "unavailable" hypothesis so the loop continues and
        // the remaining questions still run. The recall engine already drops individual bad sub-query
        // vectors; this only triggers when an entire question's recall genuinely cannot proceed.
        let hypothesis = match engine
            .answer_with_reference_date(&row.question, Some(reference_date.as_str()))
            .await
        {
            Ok(answer) => BenchHypothesis {
                question_id: row.question_id.clone(),
                question_type: row.question_type.clone(),
                question: row.question.clone(),
                hypothesis: answer.text,
                debug_artifact: None,
                router_initial: None,
                router_final: None,
                router_reason: None,
            },
            Err(err) => {
                eprintln!(
                    "[longmemeval] {}/{} {} recall-unavailable: {err}; marking question unavailable and continuing",
                    idx + 1,
                    rows.len(),
                    row.question_id
                );
                BenchHypothesis {
                    question_id: row.question_id.clone(),
                    question_type: row.question_type.clone(),
                    question: row.question.clone(),
                    hypothesis: "UNAVAILABLE: recall failed for this question".to_string(),
                    debug_artifact: None,
                    router_initial: Some("recall-unavailable".to_string()),
                    router_final: Some("recall-unavailable".to_string()),
                    router_reason: Some(err.to_string()),
                }
            }
        };
        writeln!(file, "{}", serde_json::to_string(&hypothesis)?)?;
        file.flush()?;
        completed.insert(hypothesis.question_id.clone());
        out.push(hypothesis);
    }

    Ok(out)
}

fn read_existing_hypotheses(path: &Path) -> anyhow::Result<Vec<BenchHypothesis>> {
    if !path.is_file() {
        return Ok(Vec::new());
    }
    let file = fs::File::open(path)?;
    let mut out = Vec::new();
    for line in std::io::BufReader::new(file).lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        out.push(parse_bench_hypothesis_line(&line)?);
    }
    Ok(out)
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn read_existing_hypothesis_ids(path: &Path) -> anyhow::Result<BTreeSet<String>> {
    if !path.is_file() {
        return Ok(BTreeSet::new());
    }
    let file = fs::File::open(path)?;
    let mut out = BTreeSet::new();
    for line in std::io::BufReader::new(file).lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let value: Value = serde_json::from_str(&line)?;
        reject_forbidden_hypothesis_fields(&value)?;
        let Some(question_id) = value.get("question_id").and_then(Value::as_str) else {
            anyhow::bail!("hypothesis row missing question_id");
        };
        out.insert(question_id.to_string());
    }
    Ok(out)
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn reset_hypotheses_for_answer_only(path: &Path) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)?;
    Ok(())
}

fn parse_bench_hypothesis_line(line: &str) -> anyhow::Result<BenchHypothesis> {
    let value: Value = serde_json::from_str(line)?;
    reject_forbidden_hypothesis_fields(&value)?;
    Ok(serde_json::from_value(value)?)
}

fn reject_forbidden_hypothesis_fields(value: &Value) -> anyhow::Result<()> {
    let Some(object) = value.as_object() else {
        anyhow::bail!("benchmark hypothesis line must be a JSON object");
    };
    const FORBIDDEN: &[&str] = &["answer", "gold", "oracle", "label", "verdict"];
    if let Some(field) = FORBIDDEN.iter().find(|field| object.contains_key(**field)) {
        anyhow::bail!("benchmark hypothesis contains forbidden scoring field `{field}`");
    }
    Ok(())
}

#[cfg(feature = "symbiotic-memory-adapter")]
pub fn record_external_scores(
    run_root: impl AsRef<Path>,
    hypotheses_path: impl AsRef<Path>,
    scored_path: Option<impl AsRef<Path>>,
    verdicts_path: Option<impl AsRef<Path>>,
    scorer: &str,
) -> anyhow::Result<ScoreRecordReport> {
    let run_root = run_root.as_ref();
    let hypotheses_path = hypotheses_path.as_ref();
    let scored_path = scored_path.as_ref().map(|path| path.as_ref().to_path_buf());
    let verdicts_path = verdicts_path
        .as_ref()
        .map(|path| path.as_ref().to_path_buf());
    if scored_path.is_none() && verdicts_path.is_none() {
        anyhow::bail!("provide --scored or --verdicts");
    }

    let hypotheses = read_existing_hypotheses(hypotheses_path)?;
    if hypotheses.is_empty() {
        anyhow::bail!(
            "no hypotheses found in {}; score only generated hypotheses from the current run",
            hypotheses_path.display()
        );
    }
    let hypothesis_ids = hypotheses
        .iter()
        .map(|hypothesis| hypothesis.question_id.clone())
        .collect::<BTreeSet<_>>();

    let scores_dir = run_root.join("scores");
    fs::create_dir_all(&scores_dir)?;
    let scored_artifact = scored_path
        .as_ref()
        .map(|path| copy_score_artifact(path, &scores_dir))
        .transpose()?;
    let verdicts_artifact = verdicts_path
        .as_ref()
        .map(|path| copy_score_artifact(path, &scores_dir))
        .transpose()?;

    let scored_summary = scored_artifact
        .as_ref()
        .map(|artifact| read_json_file(&run_root.join(artifact)))
        .transpose()?;
    let verdicts = verdicts_artifact
        .as_ref()
        .map(|artifact| read_verdicts_jsonl(&run_root.join(artifact), &hypothesis_ids))
        .transpose()?
        .unwrap_or_default();
    let verdict_by_question = verdicts
        .iter()
        .map(|verdict| (verdict.question_id.clone(), verdict.clone()))
        .collect::<BTreeMap<_, _>>();
    let metrics = score_metrics(scored_summary.as_ref(), &verdicts);
    let artifact_hashes = score_artifact_hashes(
        run_root,
        scored_artifact.as_deref(),
        verdicts_artifact.as_deref(),
    )?;
    let score_input = serde_json::json!({
        "scorer": scorer,
        "hypotheses_hash": hash_file(hypotheses_path)?,
        "hypotheses": hypothesis_ids,
        "artifacts": artifact_hashes,
    });
    let score_summary = serde_json::json!({
        "schema_version": 1,
        "scorer": scorer,
        "hypotheses_file": hypotheses_path.display().to_string(),
        "hypotheses_hash": score_input["hypotheses_hash"],
        "hypotheses_count": hypotheses.len(),
        "scored_artifact": scored_artifact,
        "verdicts_artifact": verdicts_artifact,
        "artifact_hashes": artifact_hashes,
        "metrics": metrics,
        "scored_summary": scored_summary,
    });
    let summary_path = run_root.join("score-summary.json");
    write_json_atomic(&summary_path, &score_summary)?;

    let mut debug_files_updated = 0usize;
    for hypothesis in &hypotheses {
        let verdict = verdict_by_question.get(&hypothesis.question_id);
        debug_files_updated += update_question_debug_score(
            &run_root.join("vaults").join(&hypothesis.question_id),
            hypothesis.debug_artifact.as_deref(),
            scorer,
            scored_artifact.as_deref(),
            verdicts_artifact.as_deref(),
            verdict,
        )?;
    }

    Ok(ScoreRecordReport {
        hypotheses: hypotheses.len(),
        verdicts: verdicts.len(),
        debug_files_updated,
        summary_path: summary_path.display().to_string(),
    })
}

pub fn clear_score_artifacts(
    run_root: impl AsRef<Path>,
    hypotheses_path: impl AsRef<Path>,
) -> anyhow::Result<usize> {
    let run_root = run_root.as_ref();
    let hypotheses_path = hypotheses_path.as_ref();
    let mut removed = 0usize;

    removed += remove_score_artifact_if_exists(&run_root.join("score-summary.json"))?;
    removed += remove_score_artifact_if_exists(&run_root.join("scores"))?;

    let hypotheses = hypotheses_path.to_string_lossy();
    for suffix in [".scored.json", ".verdicts.jsonl", ".partial.verdicts.jsonl"] {
        removed += remove_score_artifact_if_exists(Path::new(&format!("{hypotheses}{suffix}")))?;
    }

    Ok(removed)
}

fn remove_score_artifact_if_exists(path: &Path) -> anyhow::Result<usize> {
    if !path.exists() {
        return Ok(0);
    }
    if path.is_dir() {
        fs::remove_dir_all(path)?;
    } else {
        fs::remove_file(path)?;
    }
    Ok(1)
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn debug_run_id(out_path: &Path) -> String {
    let name = out_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.trim().is_empty())
        .unwrap_or("hypotheses");
    let sanitized = name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>();
    let trimmed = sanitized
        .trim_matches('-')
        .chars()
        .take(96)
        .collect::<String>();
    if trimmed.is_empty() {
        "hypotheses".to_string()
    } else {
        trimmed
    }
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn update_question_debug_score(
    vault_dir: &Path,
    debug_artifact: Option<&str>,
    scorer: &str,
    scored_artifact: Option<&str>,
    verdicts_artifact: Option<&str>,
    verdict: Option<&ScoreVerdict>,
) -> anyhow::Result<usize> {
    let mut paths = vec![vault_dir.join("debug").join("question-debug.json")];
    if let Some(debug_artifact) = debug_artifact {
        if let Some(run_root) = vault_dir.parent().and_then(Path::parent) {
            paths.push(run_root.join(debug_artifact));
        }
    }
    let mut updated = 0usize;
    for debug_path in paths {
        if !debug_path.is_file() {
            continue;
        }
        let mut debug = read_json_file(&debug_path)?;
        if let Some(object) = debug.as_object_mut() {
            object.insert(
                "scoring".to_string(),
                serde_json::json!({
                    "scorer": scorer,
                    "scored_artifact": scored_artifact,
                    "verdicts_artifact": verdicts_artifact,
                    "verdict": verdict,
                }),
            );
        }
        write_json_atomic(&debug_path, &debug)?;
        updated += 1;
    }
    Ok(updated)
}

#[cfg(feature = "symbiotic-memory-adapter")]
async fn enqueue_and_claim_workflow_row(
    queue: &dyn QueueBackend,
    queue_id: QueueId,
    worker_id: &str,
    question_id: &str,
    input_hash: &str,
    routed: bool,
    lease_seconds: u64,
    max_in_flight: usize,
    max_attempts: u32,
    allow_terminal_reenqueue: bool,
) -> anyhow::Result<QueueItem> {
    let request = EnqueueRequest {
        queue_id: queue_id.clone(),
        kind: "longmemeval.row".to_string(),
        payload: serde_json::json!({
            "question_id": question_id,
            "input_hash": input_hash,
            "routed": routed,
        }),
        idempotency_key: Some(format!("{question_id}:{input_hash}")),
        run_after: None,
        max_attempts: Some(max_attempts.max(1)),
        force: false,
    };
    let mut outcome = queue.enqueue(request.clone()).await.map_err(queue_error)?;

    if matches!(outcome.disposition, EnqueueDisposition::TerminalDuplicate) {
        if !allow_terminal_reenqueue {
            anyhow::bail!(
                "workflow row {question_id} already reached terminal queue state without a matching hypothesis; use --resume to re-enqueue missing terminal rows or use a fresh run root"
            );
        }
        let mut forced = request;
        forced.force = true;
        outcome = queue.enqueue(forced).await.map_err(queue_error)?;
        if !matches!(outcome.disposition, EnqueueDisposition::Inserted) {
            anyhow::bail!(
                "workflow row {question_id} could not be force re-enqueued from terminal state"
            );
        }
    }

    for _ in 0..lease_claim_attempts(lease_seconds) {
        if let Some(claimed) = queue
            .claim_item(
                &outcome.item.item_id,
                worker_id,
                lease_seconds,
                Some(max_in_flight),
            )
            .await
            .map_err(queue_error)?
        {
            return Ok(claimed);
        }

        if let Some(current) = queue
            .get_item(&outcome.item.item_id)
            .await
            .map_err(queue_error)?
        {
            match current.status {
                DurableQueueStatus::Succeeded => {
                    anyhow::bail!(
                        "workflow row {question_id} was completed by another worker before this run wrote a hypothesis"
                    );
                }
                DurableQueueStatus::Dead => {
                    anyhow::bail!(
                        "workflow row {question_id} is dead-lettered: {}",
                        current
                            .last_error
                            .unwrap_or_else(|| "unknown error".to_string())
                    );
                }
                DurableQueueStatus::Running
                | DurableQueueStatus::Pending
                | DurableQueueStatus::Failed => {}
            }
        }

        tokio::time::sleep(Duration::from_millis(250)).await;
    }

    anyhow::bail!(
        "workflow row {question_id} could not acquire a lease within {}s",
        lease_seconds
    )
}

#[cfg(feature = "symbiotic-memory-adapter")]
async fn fail_workflow_item(
    queue: &dyn QueueBackend,
    item_id: &QueueItemId,
    worker_id: &str,
    error: &str,
    attempt: u32,
) -> anyhow::Result<()> {
    queue
        .fail(
            item_id,
            worker_id,
            error,
            Some(workflow_retry_delay_seconds(attempt)),
        )
        .await
        .map(|_| ())
        .map_err(queue_error)
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn workflow_worker_id(question_id: &str) -> String {
    let sanitized = question_id
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect::<String>();
    format!("symem-{}-{sanitized}", std::process::id())
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn lease_claim_attempts(lease_seconds: u64) -> usize {
    lease_seconds.saturating_mul(4).clamp(4, 2_400) as usize
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn queue_error(err: QueueError) -> anyhow::Error {
    anyhow::anyhow!("workflow queue error: {err}")
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn workflow_max_in_flight(configured: Option<usize>) -> usize {
    std::env::var("SYMEM_WORKFLOW_MAX_IN_FLIGHT")
        .ok()
        .and_then(|value| value.parse().ok())
        .or(configured)
        .unwrap_or(50)
        .max(1)
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn raw_embed_only_diagnostic() -> bool {
    std::env::var("SYMEM_INGEST_STOP_AFTER_RAW_EMBED")
        .ok()
        .is_some_and(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
}

/// Gold-oracle mode (`--oracle-gold` / `SYMEM_ORACLE_GOLD=1`). When on, the answerer is fed ONLY the
/// gold-session raw turns for each question (zero retrieval, zero noise) instead of the recall→rerank
/// output, isolating the reader so we can tell whether multi-session answers fail from noise/dilution
/// or because the reader genuinely cannot compile them even with perfect evidence.
#[cfg(feature = "symbiotic-memory-adapter")]
fn oracle_gold_enabled() -> bool {
    std::env::var("SYMEM_ORACLE_GOLD")
        .ok()
        .is_some_and(|value| matches!(value.trim(), "1" | "true" | "TRUE" | "yes" | "YES"))
}

/// Assemble the gold-oracle context: the raw turns of every gold session, in the order the dataset
/// lists them, formatted to mirror the recall `source_turn` context style so the answer prompt treats
/// them exactly as it would real retrieved turns. Each gold session id in `answer_session_ids` is
/// resolved to its slot in `haystack_session_ids`, and that session's turns are emitted one context
/// string per turn. `captured_at` is the session's haystack date (when the conversation was held);
/// `score` is fixed at 1.000 (these are gold, not ranked). Always returns `Some` — an EMPTY list for
/// abstention questions (no `has_answer` turns), which correctly forces the answerer to abstain instead
/// of falling back to noisy recall.
#[cfg(feature = "symbiotic-memory-adapter")]
fn build_gold_oracle_context(row: &LongMemEvalRecord) -> Option<Vec<String>> {
    // EXACT-evidence oracle: feed the answerer ONLY the dataset's `has_answer:true` turns — the
    // ground-truth marked evidence (typically 2-6 turns of ~480). NOT whole gold sessions: a gold
    // session is a long conversation dominated by assistant lectures / chit-chat / adjacent topics
    // that are noise. Scan all sessions and keep only the marked turns, so the answerer sees the
    // minimal exact list and nothing else.
    // Two env-gated context-shaping levers (both default OFF = original behavior):
    //   SYMEM_ORACLE_SORT_BY_DATE=1 → emit turns in chronological captured_at order (across sessions),
    //                                 not haystack-session order — a clean timeline to count along.
    //   SYMEM_ORACLE_DROP_SCORE=1   → omit the fixed "score: 1.000" tag, which is pure noise on gold.
    let sort_by_date = std::env::var("SYMEM_ORACLE_SORT_BY_DATE")
        .map(|v| matches!(v.trim(), "1" | "on" | "true" | "yes"))
        .unwrap_or(false);
    let drop_score = std::env::var("SYMEM_ORACLE_DROP_SCORE")
        .map(|v| matches!(v.trim(), "1" | "on" | "true" | "yes"))
        .unwrap_or(false);
    // (captured_at, original_seq, rendered_line) — captured_at is rfc3339 so it sorts chronologically;
    // "unknown" sorts last; original_seq is a stable tiebreaker so same-date turns keep their order.
    let mut items: Vec<(String, usize, String)> = Vec::new();
    let mut seq = 0usize;
    for (idx, session) in row.haystack_sessions.iter().enumerate() {
        let session_id = row
            .haystack_session_ids
            .get(idx)
            .map(|s| s.as_str())
            .unwrap_or("unknown");
        let captured_at = row
            .haystack_dates
            .get(idx)
            .and_then(|date| parse_longmemeval_datetime(date))
            .map(|t| t.to_rfc3339())
            .unwrap_or_else(|| "unknown".to_string());
        for (msg_idx, msg) in session.iter().enumerate() {
            if !msg.has_answer {
                continue;
            }
            let line = if drop_score {
                format!(
                    "[type: source_turn | source_id: {} | turn_id: {}:{} | ordinal: {} | speaker: {} | captured_at: {}] {}",
                    row.question_id,
                    session_id,
                    msg_idx,
                    msg_idx,
                    msg.role,
                    captured_at,
                    msg.content,
                )
            } else {
                format!(
                    "[type: source_turn | source_id: {} | turn_id: {}:{} | ordinal: {} | speaker: {} | captured_at: {} | score: {:.3}] {}",
                    row.question_id,
                    session_id,
                    msg_idx,
                    msg_idx,
                    msg.role,
                    captured_at,
                    1.0_f32,
                    msg.content,
                )
            };
            items.push((captured_at.clone(), seq, line));
            seq += 1;
        }
    }
    if sort_by_date {
        items.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    }
    let context: Vec<String> = items.into_iter().map(|(_, _, line)| line).collect();
    // Always force the oracle context — even when empty. An empty list is the CORRECT oracle input for
    // abstention questions (no `has_answer` turns): the answerer sees no evidence and abstains, which is
    // the right answer. Returning None instead fell back to normal noisy recall (briefs + ranked facts),
    // contaminating the abstention subset and triggering false answers. (The "briefs on abstention Qs" bug.)
    Some(context)
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn workflow_max_attempts() -> u32 {
    std::env::var("SYMEM_WORKFLOW_MAX_ATTEMPTS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(3)
        .max(1)
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn workflow_retry_delay_seconds(attempt: u32) -> u64 {
    let base = std::env::var("SYMEM_WORKFLOW_RETRY_DELAY_SECS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(2_u64)
        .max(1);
    let multiplier = 1_u64 << attempt.saturating_sub(1).min(5);
    base.saturating_mul(multiplier).min(60)
}

#[cfg(feature = "symbiotic-memory-adapter")]
async fn run_workflow_row<F, T>(row_result: F, timeout: Option<Duration>) -> anyhow::Result<T>
where
    F: std::future::Future<Output = anyhow::Result<T>>,
{
    if let Some(timeout) = timeout {
        match tokio::time::timeout(timeout, row_result).await {
            Ok(result) => result,
            Err(_) => anyhow::bail!(
                "workflow row timed out after {}s; this optional outer guard should stay disabled for normal benchmarks because model and step calls have their own timeouts",
                timeout.as_secs()
            ),
        }
    } else {
        row_result.await
    }
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn question_timeout() -> Option<Duration> {
    let seconds = std::env::var("SYMEM_QUESTION_TIMEOUT_SECS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    (seconds > 0).then(|| Duration::from_secs(seconds))
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn workflow_input_hash(
    row: &LongMemEvalRecord,
    routed: bool,
    answer_only: bool,
    consolidate_briefs: bool,
    ingest_diagnostic_mode: IngestDiagnosticMode,
    policy: &symbiotic_memory::config::RecallPolicy,
) -> String {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(row.question_id.as_bytes());
    hasher.update(b"\0");
    hasher.update(row.question.as_bytes());
    hasher.update(b"\0");
    hasher.update(row.question_date.as_deref().unwrap_or("").as_bytes());
    hasher.update(b"\0");
    hasher.update(if routed { b"routed" } else { b"direct" });
    hasher.update(b"\0");
    hasher.update(if answer_only {
        b"answer-only".as_slice()
    } else {
        b"full".as_slice()
    });
    hasher.update(b"\0");
    hasher.update(if consolidate_briefs {
        b"consolidate-briefs".as_slice()
    } else {
        b"base-index".as_slice()
    });
    hasher.update(b"\0");
    hasher.update(ingest_diagnostic_mode.as_str().as_bytes());
    hasher.update(b"\0");
    hasher.update(policy.version.as_bytes());
    hasher.update(b"\0");
    hasher.update(
        serde_json::to_string(policy)
            .unwrap_or_else(|_| "unserializable-policy".to_string())
            .as_bytes(),
    );
    hasher.update(b"\0");
    {
        let (window, raw_unit_tokens, _, _) = effective_shape();
        hasher.update(raw_unit_fingerprint(window, raw_unit_tokens).as_bytes());
    }
    hasher.update(b"\0");
    for session in &row.haystack_sessions {
        for message in session {
            hasher.update(message.role.as_bytes());
            hasher.update(b"\0");
            hasher.update(message.content.as_bytes());
            hasher.update(b"\0");
        }
    }
    hex::encode(hasher.finalize())
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn effective_shape() -> (Option<symbiotic_memory::ingest::RawWindowConfig>, usize, usize, usize) {
    let distill = symbiotic_memory_config::DistillSection::default();
    let embed = symbiotic_memory_config::EmbedSection::default();
    let window = symbiotic_memory::ingest::RawWindowConfig::from_values(
        distill.raw_window_size,
        distill.raw_window_stride,
    );
    (
        window,
        distill.raw_unit_max_input_tokens,
        distill.window_max_input_tokens,
        embed.max_input_tokens,
    )
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn source_shape_hash(source: &SourceDocument) -> anyhow::Result<String> {
    let (window, raw_unit_tokens, window_tokens, embed_tokens) = effective_shape();
    stable_hash_json(&serde_json::json!({
        "source": source,
        "raw_unit_shape": raw_unit_fingerprint(window, raw_unit_tokens),
        "distill_window_max_input_tokens": window_tokens,
        "embed_max_input_tokens": embed_tokens,
    }))
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn post_ingest_complete(manifest: &MemoryRunManifest, consolidate_briefs: bool) -> bool {
    let mut required = vec![
        MemoryStage::Capture,
        MemoryStage::DistillWindow,
        MemoryStage::WriteArchive,
        MemoryStage::EmbedRaw,
        MemoryStage::EmbedFacts,
        MemoryStage::Index,
    ];
    if consolidate_briefs {
        required.push(MemoryStage::Consolidate);
    }
    required
        .into_iter()
        .all(|stage| manifest.stage_succeeded(stage))
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_vec_pretty(value)?)?;
    fs::rename(tmp, path)?;
    Ok(())
}

#[cfg(feature = "symbiotic-memory-adapter")]
#[derive(Clone, Debug, Deserialize, Serialize)]
struct ScoreVerdict {
    question_id: String,
    #[serde(default)]
    question_type: Option<String>,
    #[serde(default)]
    label: Option<bool>,
    #[serde(default)]
    error: Option<Value>,
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn copy_score_artifact(source: &Path, scores_dir: &Path) -> anyhow::Result<String> {
    let file_name = source
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "score artifact has no UTF-8 file name: {}",
                source.display()
            )
        })?;
    let destination = scores_dir.join(file_name);
    let bytes = fs::read(source)?;
    let tmp = destination.with_extension("tmp");
    fs::write(&tmp, bytes)?;
    fs::rename(tmp, &destination)?;
    Ok(destination
        .strip_prefix(scores_dir.parent().unwrap_or(scores_dir))
        .unwrap_or(&destination)
        .display()
        .to_string())
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn read_json_file(path: &Path) -> anyhow::Result<Value> {
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn read_verdicts_jsonl(
    path: &Path,
    hypothesis_ids: &BTreeSet<String>,
) -> anyhow::Result<Vec<ScoreVerdict>> {
    let file = fs::File::open(path)?;
    let mut out = Vec::new();
    for (line_idx, line) in std::io::BufReader::new(file).lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let verdict: ScoreVerdict = serde_json::from_str(&line).map_err(|err| {
            anyhow::anyhow!(
                "invalid verdict JSON at {} line {}: {err}",
                path.display(),
                line_idx + 1
            )
        })?;
        if !hypothesis_ids.contains(&verdict.question_id) {
            anyhow::bail!(
                "verdict for {} is not present in current-run hypotheses",
                verdict.question_id
            );
        }
        out.push(verdict);
    }
    Ok(out)
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn score_metrics(scored_summary: Option<&Value>, verdicts: &[ScoreVerdict]) -> Value {
    let verdict_scored = verdicts.len() as u64;
    let verdict_correct = verdicts
        .iter()
        .filter(|verdict| verdict.label == Some(true))
        .count() as u64;
    let verdict_errors = verdicts
        .iter()
        .filter(|verdict| {
            verdict
                .error
                .as_ref()
                .map(|error| !error.is_null())
                .unwrap_or(false)
        })
        .count() as u64;
    let summary_counts = scored_summary.and_then(|value| value.get("counts"));
    let scored = summary_counts
        .and_then(|counts| counts.get("scored"))
        .and_then(Value::as_u64)
        .unwrap_or(verdict_scored);
    let correct = summary_counts
        .and_then(|counts| counts.get("total_correct"))
        .and_then(Value::as_u64)
        .unwrap_or(verdict_correct);
    let judge_errors = summary_counts
        .and_then(|counts| counts.get("judge_errors"))
        .and_then(Value::as_u64)
        .unwrap_or(verdict_errors);
    let overall_accuracy = scored_summary
        .and_then(|value| value.get("overall_accuracy"))
        .and_then(Value::as_f64)
        .or_else(|| (scored > 0).then_some(correct as f64 / scored as f64));
    serde_json::json!({
        "scored": scored,
        "correct": correct,
        "judge_errors": judge_errors,
        "overall_accuracy": overall_accuracy,
        "task_averaged_accuracy": scored_summary
            .and_then(|value| value.get("task_averaged_accuracy"))
            .and_then(Value::as_f64),
        "verdict_count": verdicts.len(),
    })
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn score_artifact_hashes(
    run_root: &Path,
    scored_artifact: Option<&str>,
    verdicts_artifact: Option<&str>,
) -> anyhow::Result<BTreeMap<String, String>> {
    let mut hashes = BTreeMap::new();
    if let Some(path) = scored_artifact {
        hashes.insert(path.to_string(), hash_file(&run_root.join(path))?);
    }
    if let Some(path) = verdicts_artifact {
        hashes.insert(path.to_string(), hash_file(&run_root.join(path))?);
    }
    Ok(hashes)
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn hash_file(path: &Path) -> anyhow::Result<String> {
    use sha2::{Digest, Sha256};

    let bytes = fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "symbiotic-memory-adapter")]
    use symbiotic_memory::types::{MemoryFact, RawArchiveReceipt};

    #[cfg(feature = "symbiotic-memory-adapter")]
    #[test]
    fn checked_in_symbiotic_memory_profiles_load_from_benchmark_repo() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let cases = [
            (
                "config/symbiotic-memory/longmemeval-raw-light.yaml",
                "memory-recall-v3-raw-light",
                10usize,
                10usize,
                1000usize,
                300u64,
            ),
            (
                "config/symbiotic-memory/longmemeval-raw-tiny.yaml",
                "memory-recall-v3-raw-tiny",
                5usize,
                50usize,
                1000usize,
                300u64,
            ),
            (
                "config/symbiotic-memory/longmemeval-raw-wide-diagnostic.yaml",
                "memory-recall-v3-raw-wide-diagnostic",
                80usize,
                50usize,
                1000usize,
                300u64,
            ),
        ];

        for (
            path,
            version,
            raw_top_k,
            workflow_max_in_flight,
            embedding_max_in_flight,
            embedding_timeout_seconds,
        ) in cases
        {
            let config = symbiotic_memory::MemoryConfig::load_yaml(root.join(path)).unwrap();
            let resolved = config
                .queue
                .resolve_provider_queue(&config.providers.embedding);
            let chat_resolved = config
                .queue
                .resolve_provider_queue(&config.providers.distill);

            assert_eq!(config.recall.version, version);
            assert_eq!(config.recall.fact_top_k, 20);
            assert_eq!(config.recall.raw_turn_top_k, raw_top_k);
            assert_eq!(config.queue.workflow_max_in_flight, workflow_max_in_flight);
            assert_eq!(resolved.queue_id, "embedding:gemini:gemini-embedding-2");
            assert_eq!(resolved.max_in_flight, embedding_max_in_flight);
            assert_eq!(resolved.timeout_seconds, embedding_timeout_seconds);
            if path == "config/symbiotic-memory/longmemeval-raw-light.yaml" {
                assert_eq!(chat_resolved.queue_id, "chat:deepseek:deepseek-v4-flash");
                assert_eq!(chat_resolved.max_in_flight, 2000);
                assert_eq!(chat_resolved.timeout_seconds, 600);
                assert_eq!(resolved.requests_per_minute, Some(4_500));
                assert_eq!(resolved.input_units_per_minute, Some(5_000_000));
            }
        }
    }

    #[cfg(feature = "symbiotic-memory-adapter")]
    #[test]
    fn zvec_cache_allows_fresh_vault_before_sqlite_exists() {
        let dir = tempfile::tempdir().unwrap();
        let state = prepare_zvec_index_cache(dir.path(), "source-hash", 768, false).unwrap();

        assert!(!state.valid);
        assert!(!state.trusted_manifest);
        assert!(!zvec_index_dir(dir.path()).exists());
    }

    #[test]
    fn longmemeval_loader_matches_real_shape() {
        let json = r#"[{
          "question_id":"q1",
          "question_type":"count",
          "question":"How many pens did I buy?",
          "question_date":"2023/01/02 (Mon) 00:00",
	          "answer":4,
          "answer_session_ids":["s1"],
          "haystack_dates":["2023/01/01 (Sun) 00:00"],
          "haystack_session_ids":["s1"],
          "haystack_sessions":[[
            {"role":"user","content":"I bought 4 pens."},
            {"role":"assistant","content":"Great."}
          ]]
        }]"#;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("lme.json");
        fs::write(&path, json).unwrap();
        let rows = load_longmemeval(&path, Some(1)).unwrap();
        assert_eq!(rows[0].question_id, "q1");
        let source = longmemeval_to_source(&rows[0]);
        assert_eq!(source.turns.len(), 2);
        assert_eq!(source.turns[0].turn_id, "s1:0");
        assert_eq!(
            source.turns[0].captured_at.unwrap().to_rfc3339(),
            "2023-01-01T00:00:00+00:00"
        );
    }

    #[test]
    fn parses_longmemeval_dates_with_weekday() {
        assert_eq!(
            parse_longmemeval_datetime("2023/05/09 (Tue) 13:45")
                .unwrap()
                .to_rfc3339(),
            "2023-05-09T13:45:00+00:00"
        );
    }

    #[test]
    fn answer_reference_datetime_defaults_to_current_rfc3339_timestamp() {
        let reference = select_answer_reference_datetime(
            |_| None,
            None,
            "2026-06-19T15:15:42+08:00".to_string(),
        );

        assert_eq!(reference, "2026-06-19T15:15:42+08:00");
    }

    #[test]
    fn answer_reference_datetime_uses_benchmark_reference_clock() {
        let reference = select_answer_reference_datetime(
            |_| None,
            Some("2023-05-30T21:35:00+00:00".to_string()),
            "2026-06-19T15:15:42+08:00".to_string(),
        );

        assert_eq!(reference, "2023-05-30T21:35:00+00:00");
    }

    #[test]
    fn answer_reference_datetime_uses_explicit_benchmark_override() {
        let reference = select_answer_reference_datetime(
            |key| {
                (key == "MEMBENCH_REFERENCE_DATETIME")
                    .then(|| "2026-06-19T15:15:42+08:00".to_string())
            },
            Some("2023-05-30T21:35:00+00:00".to_string()),
            "2026-06-19T16:00:00+08:00".to_string(),
        );

        assert_eq!(reference, "2026-06-19T15:15:42+08:00");
    }

    #[test]
    fn bench_hypothesis_has_no_gold_answer_surface() {
        let hypothesis = BenchHypothesis {
            question_id: "q1".to_string(),
            question_type: Some("count".to_string()),
            question: "How many pens?".to_string(),
            hypothesis: "4".to_string(),
            debug_artifact: None,
            router_initial: None,
            router_final: None,
            router_reason: None,
        };

        let json = serde_json::to_value(&hypothesis).unwrap();

        assert!(json.get("answer").is_none());
        assert_eq!(json["hypothesis"], serde_json::json!("4"));
    }

    #[test]
    fn hypothesis_reader_rejects_scoring_fields() {
        let err = parse_bench_hypothesis_line(
            r#"{"question_id":"q1","question":"Q?","hypothesis":"A","answer":"gold"}"#,
        )
        .unwrap_err()
        .to_string();

        assert!(err.contains("forbidden scoring field `answer`"), "{err}");
    }

    #[cfg(feature = "symbiotic-memory-adapter")]
    #[test]
    fn workflow_input_hash_includes_query_planner_mode() {
        let row = LongMemEvalRecord {
            question_id: "q-planner-hash".to_string(),
            question_type: Some("direct".to_string()),
            question: "Where did I buy the racket?".to_string(),
            question_date: Some("2023/01/02 (Mon) 00:00".to_string()),
            answer: None,
            haystack_dates: vec!["2023/01/01 (Sun) 00:00".to_string()],
            haystack_session_ids: vec!["s1".to_string()],
            haystack_sessions: vec![vec![LongMemEvalMessage {
                role: "user".to_string(),
                content: "I bought the racket downtown.".to_string(),
            }]],
        };
        let mut off = symbiotic_memory::config::RecallPolicy::default();
        off.query_planner = symbiotic_memory::config::QueryPlannerMode::Off;
        let mut flash = off.clone();
        flash.query_planner = symbiotic_memory::config::QueryPlannerMode::Flash;

        assert_ne!(
            workflow_input_hash(&row, false, false, false, IngestDiagnosticMode::None, &off),
            workflow_input_hash(
                &row,
                false,
                false,
                false,
                IngestDiagnosticMode::None,
                &flash
            )
        );
    }

    #[cfg(feature = "symbiotic-memory-adapter")]
    #[test]
    fn workflow_input_hash_includes_recall_policy_values() {
        let row = LongMemEvalRecord {
            question_id: "q-policy-hash".to_string(),
            question_type: Some("direct".to_string()),
            question: "Where did I buy the racket?".to_string(),
            question_date: Some("2023/01/02 (Mon) 00:00".to_string()),
            answer: None,
            haystack_dates: vec!["2023/01/01 (Sun) 00:00".to_string()],
            haystack_session_ids: vec!["s1".to_string()],
            haystack_sessions: vec![vec![LongMemEvalMessage {
                role: "user".to_string(),
                content: "I bought the racket downtown.".to_string(),
            }]],
        };
        let mut raw40 = symbiotic_memory::config::RecallPolicy::default();
        raw40.raw_turn_top_k = 40;
        let mut raw80 = raw40.clone();
        raw80.raw_turn_top_k = 80;

        assert_ne!(
            workflow_input_hash(&row, true, true, false, IngestDiagnosticMode::None, &raw40),
            workflow_input_hash(&row, true, true, false, IngestDiagnosticMode::None, &raw80)
        );
    }

    #[cfg(feature = "symbiotic-memory-adapter")]
    #[test]
    fn workflow_input_hash_includes_consolidation_flag() {
        let row = LongMemEvalRecord {
            question_id: "q-consolidate".to_string(),
            question_type: None,
            question: "What did I buy?".to_string(),
            answer: None,
            question_date: None,
            haystack_dates: Vec::new(),
            haystack_session_ids: Vec::new(),
            haystack_sessions: Vec::new(),
        };
        let policy = symbiotic_memory::config::RecallPolicy::default();

        assert_ne!(
            workflow_input_hash(
                &row,
                true,
                false,
                false,
                IngestDiagnosticMode::None,
                &policy
            ),
            workflow_input_hash(&row, true, false, true, IngestDiagnosticMode::None, &policy)
        );
    }

    #[cfg(feature = "symbiotic-memory-adapter")]
    #[test]
    fn workflow_input_hash_includes_ingest_diagnostic_mode() {
        let row = LongMemEvalRecord {
            question_id: "q-diagnostic-hash".to_string(),
            question_type: None,
            question: "What did I buy?".to_string(),
            answer: None,
            question_date: None,
            haystack_dates: Vec::new(),
            haystack_session_ids: Vec::new(),
            haystack_sessions: Vec::new(),
        };
        let policy = symbiotic_memory::config::RecallPolicy::default();

        assert_ne!(
            workflow_input_hash(
                &row,
                true,
                false,
                false,
                IngestDiagnosticMode::None,
                &policy
            ),
            workflow_input_hash(
                &row,
                true,
                false,
                false,
                IngestDiagnosticMode::DistillOnly,
                &policy
            )
        );
    }

    #[cfg(feature = "symbiotic-memory-adapter")]
    #[test]
    fn workflow_max_in_flight_uses_config_with_env_override() {
        unsafe {
            std::env::remove_var("SYMEM_WORKFLOW_MAX_IN_FLIGHT");
        }
        assert_eq!(workflow_max_in_flight(Some(37)), 37);
        assert_eq!(workflow_max_in_flight(None), 50);

        unsafe {
            std::env::set_var("SYMEM_WORKFLOW_MAX_IN_FLIGHT", "12");
        }
        assert_eq!(workflow_max_in_flight(Some(37)), 12);
        unsafe {
            std::env::remove_var("SYMEM_WORKFLOW_MAX_IN_FLIGHT");
        }
    }

    #[cfg(feature = "symbiotic-memory-adapter")]
    #[derive(Clone)]
    struct ConcurrencyTrackingDistiller {
        active: std::sync::Arc<std::sync::atomic::AtomicUsize>,
        max_seen: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    }

    #[cfg(feature = "symbiotic-memory-adapter")]
    #[async_trait::async_trait]
    impl Distiller for ConcurrencyTrackingDistiller {
        async fn distill(
            &self,
            source: &SourceDocument,
            receipt: &RawArchiveReceipt,
        ) -> anyhow::Result<Vec<MemoryFact>> {
            use std::sync::atomic::Ordering;

            let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
            let mut observed = self.max_seen.load(Ordering::SeqCst);
            while active > observed {
                match self.max_seen.compare_exchange(
                    observed,
                    active,
                    Ordering::SeqCst,
                    Ordering::SeqCst,
                ) {
                    Ok(_) => break,
                    Err(next) => observed = next,
                }
            }

            tokio::time::sleep(std::time::Duration::from_millis(40)).await;

            let turn = source
                .turns
                .iter()
                .find(|turn| turn.speaker.as_deref() == Some("user"))
                .expect("test user turn");
            let mut fact = MemoryFact::new(
                format!("The user said: {}", turn.text),
                vec![
                    symbiotic_memory::types::SourceRef::turn(
                        &source.source_id,
                        &receipt.receipt_id,
                        &turn.turn_id,
                    )
                    .with_captured_at(turn.captured_at),
                ],
            );
            fact.captured_at = turn.captured_at;
            fact.event_time = turn.captured_at;
            fact.valid_from = turn.captured_at;
            self.active.fetch_sub(1, Ordering::SeqCst);
            Ok(vec![fact])
        }
    }

    #[cfg(feature = "symbiotic-memory-adapter")]
    #[tokio::test]
    async fn sqlite_workflow_respects_configured_source_wip() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};
        use symbiotic_memory::config::RecallPolicy;
        use symbiotic_memory::providers::{DisabledChatProvider, HashEmbeddingProvider};

        unsafe {
            std::env::remove_var("SYMEM_WORKFLOW_MAX_IN_FLIGHT");
        }

        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("hyps.jsonl");
        let rows = (0..6)
            .map(|idx| LongMemEvalRecord {
                question_id: format!("q-wip-{idx}"),
                question_type: Some("direct".to_string()),
                question: format!("What did I say in item {idx}?"),
                question_date: Some("2023/01/02 (Mon) 00:00".to_string()),
                answer: Some(serde_json::json!(format!("item {idx}"))),
                haystack_dates: vec!["2023/01/01 (Sun) 00:00".to_string()],
                haystack_session_ids: vec![format!("s{idx}")],
                haystack_sessions: vec![vec![LongMemEvalMessage {
                    role: "user".to_string(),
                    content: format!("I mentioned item {idx}."),
                }]],
            })
            .collect::<Vec<_>>();
        let mut policy = RecallPolicy::default();
        policy.answerer_enabled = false;
        let active = Arc::new(AtomicUsize::new(0));
        let max_seen = Arc::new(AtomicUsize::new(0));

        let hypotheses = run_longmemeval_sqlite_with_planner(
            &rows,
            dir.path(),
            HashEmbeddingProvider::default,
            {
                let active = active.clone();
                let max_seen = max_seen.clone();
                move || ConcurrencyTrackingDistiller {
                    active: active.clone(),
                    max_seen: max_seen.clone(),
                }
            },
            None,
            || DisabledChatProvider,
            None,
            None,
            None,
            None,
            policy,
            &out,
            false,
            false,
            false,
            false,
            IngestDiagnosticMode::None,
            Some(2),
            false,
        )
        .await
        .unwrap();

        assert_eq!(hypotheses.len(), rows.len());
        assert_eq!(
            std::fs::read_to_string(&out).unwrap().lines().count(),
            rows.len()
        );
        assert!(
            max_seen.load(Ordering::SeqCst) <= 2,
            "configured source WIP should cap concurrent row processing"
        );
    }

    #[cfg(feature = "symbiotic-memory-adapter")]
    #[derive(Clone)]
    struct ControlledEmbeddingProvider {
        fail_fact_embeddings: std::sync::Arc<std::sync::atomic::AtomicBool>,
    }

    #[cfg(feature = "symbiotic-memory-adapter")]
    #[async_trait::async_trait]
    impl EmbeddingProvider for ControlledEmbeddingProvider {
        async fn embed(
            &self,
            text: &str,
        ) -> Result<Vec<f32>, symbiotic_memory::providers::ProviderError> {
            if text.contains("The user said:")
                && self
                    .fail_fact_embeddings
                    .load(std::sync::atomic::Ordering::SeqCst)
            {
                return Err(symbiotic_memory::providers::ProviderError::Unavailable(
                    "simulated fact embedding outage".to_string(),
                ));
            }
            Ok(vec![1.0, 0.0, 0.0])
        }

        async fn embed_query(
            &self,
            _text: &str,
        ) -> Result<Vec<f32>, symbiotic_memory::providers::ProviderError> {
            Ok(vec![1.0, 0.0, 0.0])
        }

        fn dimensions(&self) -> usize {
            3
        }
    }

    #[cfg(feature = "symbiotic-memory-adapter")]
    #[derive(Clone)]
    struct CountingDistiller {
        calls: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    }

    #[cfg(feature = "symbiotic-memory-adapter")]
    #[async_trait::async_trait]
    impl Distiller for CountingDistiller {
        async fn distill(
            &self,
            source: &SourceDocument,
            receipt: &RawArchiveReceipt,
        ) -> anyhow::Result<Vec<MemoryFact>> {
            self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let turn = source
                .turns
                .iter()
                .find(|turn| turn.speaker.as_deref() == Some("user"))
                .expect("test user turn");
            let mut fact = MemoryFact::new(
                format!("The user said: {}", turn.text),
                vec![
                    symbiotic_memory::types::SourceRef::turn(
                        &source.source_id,
                        &receipt.receipt_id,
                        &turn.turn_id,
                    )
                    .with_captured_at(turn.captured_at),
                ],
            );
            fact.captured_at = turn.captured_at;
            fact.event_time = turn.captured_at;
            fact.valid_from = turn.captured_at;
            Ok(vec![fact])
        }
    }

    #[cfg(feature = "symbiotic-memory-adapter")]
    #[tokio::test]
    async fn sqlite_benchmark_resumes_staged_distill_after_fact_embedding_failure() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
        use symbiotic_memory::config::RecallPolicy;
        use symbiotic_memory::providers::DisabledChatProvider;
        use symbiotic_memory::storage::sqlite::SqliteStore;

        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("hyps.jsonl");
        let row = LongMemEvalRecord {
            question_id: "q-staged-resume".to_string(),
            question_type: Some("count".to_string()),
            question: "How many pens did I buy?".to_string(),
            question_date: Some("2023/01/02 (Mon) 00:00".to_string()),
            answer: Some(serde_json::json!(4)),
            haystack_dates: vec!["2023/01/01 (Sun) 00:00".to_string()],
            haystack_session_ids: vec!["s1".to_string()],
            haystack_sessions: vec![vec![LongMemEvalMessage {
                role: "user".to_string(),
                content: "I bought 4 pens.".to_string(),
            }]],
        };
        let mut policy = RecallPolicy::default();
        policy.answerer_enabled = false;
        let fail_fact_embeddings = Arc::new(AtomicBool::new(true));
        let distill_calls = Arc::new(AtomicUsize::new(0));

        let first = run_longmemeval_sqlite(
            std::slice::from_ref(&row),
            dir.path(),
            {
                let fail_fact_embeddings = fail_fact_embeddings.clone();
                move || ControlledEmbeddingProvider {
                    fail_fact_embeddings: fail_fact_embeddings.clone(),
                }
            },
            {
                let distill_calls = distill_calls.clone();
                move || CountingDistiller {
                    calls: distill_calls.clone(),
                }
            },
            || DisabledChatProvider,
            policy.clone(),
            &out,
            false,
            false,
            false,
            false,
            IngestDiagnosticMode::None,
            false,
        )
        .await;
        assert!(first.is_err());
        assert_eq!(distill_calls.load(Ordering::SeqCst), 1);

        let vault_dir = dir.path().join("vaults").join("q-staged-resume");
        let staged_dir = vault_dir
            .join("archive")
            .join("staging")
            .join("q-staged-resume");
        assert_eq!(fs::read_dir(staged_dir).unwrap().count(), 1);
        let manifest = MemoryRunManifest::load(vault_dir.join("manifest.json"))
            .unwrap()
            .unwrap();
        assert!(manifest.stage_succeeded(MemoryStage::DistillWindow));
        assert!(manifest.stage_succeeded(MemoryStage::WriteArchive));
        assert!(!manifest.stage_succeeded(MemoryStage::EmbedFacts));
        assert_eq!(
            SqliteStore::open(vault_dir.join("memory.sqlite"))
                .unwrap()
                .active_facts()
                .await
                .unwrap()
                .len(),
            0
        );

        fail_fact_embeddings.store(false, Ordering::SeqCst);
        run_longmemeval_sqlite(
            &[row],
            dir.path(),
            {
                let fail_fact_embeddings = fail_fact_embeddings.clone();
                move || ControlledEmbeddingProvider {
                    fail_fact_embeddings: fail_fact_embeddings.clone(),
                }
            },
            {
                let distill_calls = distill_calls.clone();
                move || CountingDistiller {
                    calls: distill_calls.clone(),
                }
            },
            || DisabledChatProvider,
            policy,
            &out,
            false,
            false,
            false,
            false,
            IngestDiagnosticMode::None,
            true,
        )
        .await
        .unwrap();

        assert_eq!(distill_calls.load(Ordering::SeqCst), 1);
        let manifest = MemoryRunManifest::load(vault_dir.join("manifest.json"))
            .unwrap()
            .unwrap();
        assert!(manifest.stage_succeeded(MemoryStage::EmbedFacts));
        assert!(manifest.stage_succeeded(MemoryStage::Index));
        assert_eq!(
            SqliteStore::open(vault_dir.join("memory.sqlite"))
                .unwrap()
                .active_facts()
                .await
                .unwrap()
                .len(),
            1
        );
    }

    #[cfg(feature = "symbiotic-memory-adapter")]
    #[tokio::test]
    async fn sqlite_benchmark_writes_archive_before_manifest_success() {
        use symbiotic_memory::config::RecallPolicy;
        use symbiotic_memory::ingest::PassthroughDistiller;
        use symbiotic_memory::providers::{DisabledChatProvider, HashEmbeddingProvider};
        use symbiotic_memory::storage::sqlite::SqliteStore;

        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("hyps.jsonl");
        let row = LongMemEvalRecord {
            question_id: "q-archive".to_string(),
            question_type: Some("count".to_string()),
            question: "How many pens did I buy?".to_string(),
            question_date: Some("2023/01/02 (Mon) 00:00".to_string()),
            answer: Some(serde_json::json!(4)),
            haystack_dates: vec!["2023/01/01 (Sun) 00:00".to_string()],
            haystack_session_ids: vec!["s1".to_string()],
            haystack_sessions: vec![vec![
                LongMemEvalMessage {
                    role: "user".to_string(),
                    content: "I bought 4 pens.".to_string(),
                },
                LongMemEvalMessage {
                    role: "assistant".to_string(),
                    content: "Great.".to_string(),
                },
            ]],
        };
        let mut policy = RecallPolicy::default();
        policy.answerer_enabled = false;

        run_longmemeval_sqlite(
            &[row],
            dir.path(),
            HashEmbeddingProvider::default,
            || PassthroughDistiller,
            || DisabledChatProvider,
            policy,
            &out,
            false,
            false,
            false,
            false,
            IngestDiagnosticMode::None,
            false,
        )
        .await
        .unwrap();

        let vault_dir = dir.path().join("vaults").join("q-archive");
        let manifest = MemoryRunManifest::load(vault_dir.join("manifest.json"))
            .unwrap()
            .unwrap();
        assert!(manifest.stage_succeeded(MemoryStage::WriteArchive));
        let store = SqliteStore::open(vault_dir.join("memory.sqlite")).unwrap();
        let fact_count = store.active_facts().await.unwrap().len();
        let archive_count = std::fs::read_dir(vault_dir.join("archive").join("memories"))
            .unwrap()
            .count();
        assert_eq!(archive_count, fact_count);

        let conn = rusqlite::Connection::open(
            dir.path()
                .join("workflow")
                .join("longmemeval")
                .join("queue.sqlite"),
        )
        .unwrap();
        let status: String = conn
            .query_row("select status from queue_items limit 1", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(status, "succeeded");
        let embed_raw = manifest.stages.get(&MemoryStage::EmbedRaw).unwrap();
        assert_eq!(embed_raw.metrics["enabled"], serde_json::json!(true));
        assert_eq!(embed_raw.metrics["turn_count"], serde_json::json!(2));
        let capture = manifest.stages.get(&MemoryStage::Capture).unwrap();
        let artifact_path = capture.metrics["artifact_path"].as_str().unwrap();
        assert!(vault_dir.join(artifact_path).is_file());
        assert_eq!(
            capture.metrics["artifact_digest"]["algorithm"],
            serde_json::json!("sha256")
        );
    }

    #[cfg(feature = "symbiotic-memory-adapter")]
    #[tokio::test]
    async fn sqlite_answer_only_reuses_complete_vault_without_reingest() {
        use symbiotic_memory::config::RecallPolicy;
        use symbiotic_memory::ingest::PassthroughDistiller;
        use symbiotic_memory::providers::{DisabledChatProvider, HashEmbeddingProvider};

        let dir = tempfile::tempdir().unwrap();
        let first_out = dir.path().join("hyps-first.jsonl");
        let answer_only_out = dir.path().join("hyps-answer-only.jsonl");
        let row = LongMemEvalRecord {
            question_id: "q-answer-only".to_string(),
            question_type: Some("count".to_string()),
            question: "How many pens did I buy?".to_string(),
            question_date: Some("2023/01/02 (Mon) 00:00".to_string()),
            answer: Some(serde_json::json!(4)),
            haystack_dates: vec!["2023/01/01 (Sun) 00:00".to_string()],
            haystack_session_ids: vec!["s1".to_string()],
            haystack_sessions: vec![vec![LongMemEvalMessage {
                role: "user".to_string(),
                content: "I bought 4 pens.".to_string(),
            }]],
        };
        let mut policy = RecallPolicy::default();
        policy.answerer_enabled = false;

        run_longmemeval_sqlite(
            std::slice::from_ref(&row),
            dir.path(),
            HashEmbeddingProvider::default,
            || PassthroughDistiller,
            || DisabledChatProvider,
            policy.clone(),
            &first_out,
            false,
            false,
            false,
            false,
            IngestDiagnosticMode::None,
            false,
        )
        .await
        .unwrap();

        let vault_dir = dir.path().join("vaults").join("q-answer-only");
        let before = MemoryRunManifest::load(vault_dir.join("manifest.json"))
            .unwrap()
            .unwrap();
        let distill_finished_at = before.stages[&MemoryStage::DistillWindow].finished_at;
        fs::write(
            &answer_only_out,
            r#"{"question_id":"q-answer-only","question_type":"count","question":"stale","hypothesis":"STALE","debug_artifact":null,"router_initial":null,"router_final":null,"router_reason":null}"#,
        )
        .unwrap();

        run_longmemeval_sqlite(
            &[row],
            dir.path(),
            HashEmbeddingProvider::default,
            || PassthroughDistiller,
            || DisabledChatProvider,
            policy,
            &answer_only_out,
            false,
            true,
            false,
            false,
            IngestDiagnosticMode::None,
            false,
        )
        .await
        .unwrap();

        let after = MemoryRunManifest::load(vault_dir.join("manifest.json"))
            .unwrap()
            .unwrap();
        assert_eq!(
            after.stages[&MemoryStage::DistillWindow].finished_at,
            distill_finished_at
        );
        assert!(after.stage_succeeded(MemoryStage::Answer));
        assert!(answer_only_out.is_file());
        let hypotheses = read_existing_hypotheses(&answer_only_out).unwrap();
        assert_eq!(hypotheses.len(), 1);
        assert_ne!(hypotheses[0].hypothesis, "STALE");
    }

    #[cfg(feature = "symbiotic-memory-adapter")]
    #[tokio::test]
    async fn sqlite_benchmark_can_consolidate_extractive_briefs() {
        use symbiotic_memory::config::RecallPolicy;
        use symbiotic_memory::ingest::PassthroughDistiller;
        use symbiotic_memory::providers::{DisabledChatProvider, HashEmbeddingProvider};
        use symbiotic_memory::storage::sqlite::SqliteStore;

        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("hyps.jsonl");
        let row = LongMemEvalRecord {
            question_id: "q-briefs".to_string(),
            question_type: Some("direct".to_string()),
            question: "Where did I get the tennis racket?".to_string(),
            question_date: None,
            answer: Some(serde_json::json!("the sports store downtown")),
            haystack_dates: vec!["2023/01/01 (Sun) 00:00".to_string()],
            haystack_session_ids: vec!["s1".to_string()],
            haystack_sessions: vec![vec![LongMemEvalMessage {
                role: "user".to_string(),
                content: "I bought a tennis racket from the sports store downtown.".to_string(),
            }]],
        };
        let mut policy = RecallPolicy::default();
        policy.answerer_enabled = false;

        run_longmemeval_sqlite(
            &[row],
            dir.path(),
            HashEmbeddingProvider::default,
            || PassthroughDistiller,
            || DisabledChatProvider,
            policy,
            &out,
            false,
            false,
            true,
            false,
            IngestDiagnosticMode::None,
            false,
        )
        .await
        .unwrap();

        let vault_dir = dir.path().join("vaults").join("q-briefs");
        let manifest = MemoryRunManifest::load(vault_dir.join("manifest.json"))
            .unwrap()
            .unwrap();
        assert!(manifest.stage_succeeded(MemoryStage::Consolidate));
        assert_eq!(
            manifest.stages[&MemoryStage::Consolidate].metrics["brief_count"],
            serde_json::json!(1)
        );
        let active = SqliteStore::open(vault_dir.join("memory.sqlite"))
            .unwrap()
            .active_facts()
            .await
            .unwrap();
        assert!(active.iter().any(|fact| {
            fact.distillery_version.as_deref() == Some("extractive-brief-v1")
                && fact.content.contains("sports store downtown")
        }));
    }

    #[cfg(feature = "symbiotic-memory-adapter")]
    #[tokio::test]
    async fn workflow_row_rejects_terminal_duplicate_without_hypothesis() {
        use symbiotic_core::QueueId;
        use symbiotic_queue::{
            EnqueueRequest, QueueBackend, QueueStatus as DurableQueueStatus, SqliteQueue,
        };

        let queue = SqliteQueue::in_memory().unwrap();
        let queue_id = QueueId::new("workflow:longmemeval");
        let inserted = queue
            .enqueue(EnqueueRequest {
                queue_id: queue_id.clone(),
                kind: "longmemeval.row".to_string(),
                payload: serde_json::json!({"question_id": "q-terminal"}),
                idempotency_key: Some("q-terminal:hash".to_string()),
                run_after: None,
                max_attempts: Some(1),
                force: false,
            })
            .await
            .unwrap()
            .item;
        let claimed = queue
            .claim_item(&inserted.item_id, "worker-a", 60, Some(1))
            .await
            .unwrap()
            .unwrap();
        queue.complete(&claimed.item_id, "worker-a").await.unwrap();
        assert_eq!(
            queue
                .get_item(&claimed.item_id)
                .await
                .unwrap()
                .unwrap()
                .status,
            DurableQueueStatus::Succeeded
        );

        let err = enqueue_and_claim_workflow_row(
            &queue,
            queue_id,
            "worker-b",
            "q-terminal",
            "hash",
            false,
            60,
            1,
            3,
            false,
        )
        .await
        .unwrap_err()
        .to_string();

        assert!(
            err.contains("terminal queue state without a matching hypothesis"),
            "{err}"
        );
    }

    #[cfg(feature = "symbiotic-memory-adapter")]
    #[tokio::test]
    async fn workflow_row_failure_keeps_retryable_attempts() {
        use symbiotic_core::QueueId;
        use symbiotic_queue::{QueueBackend, QueueStatus as DurableQueueStatus, SqliteQueue};

        let queue = SqliteQueue::in_memory().unwrap();
        let queue_id = QueueId::new("workflow:longmemeval");
        let claimed = enqueue_and_claim_workflow_row(
            &queue, queue_id, "worker-a", "q-retry", "hash", false, 60, 1, 3, false,
        )
        .await
        .unwrap();

        fail_workflow_item(
            &queue,
            &claimed.item_id,
            "worker-a",
            "distiller response did not contain JSON",
            claimed.attempt,
        )
        .await
        .unwrap();

        let failed = queue
            .get_item(&claimed.item_id)
            .await
            .unwrap()
            .expect("workflow item remains available");
        assert_eq!(failed.status, DurableQueueStatus::Failed);
        assert_eq!(failed.attempt, 1);
        assert_eq!(failed.max_attempts, 3);
        assert_eq!(
            failed.last_error.as_deref(),
            Some("distiller response did not contain JSON")
        );
    }

    #[cfg(feature = "symbiotic-memory-adapter")]
    #[tokio::test]
    async fn workflow_row_resume_force_reenqueues_terminal_duplicate() {
        use symbiotic_core::QueueId;
        use symbiotic_queue::{
            EnqueueRequest, QueueBackend, QueueStatus as DurableQueueStatus, SqliteQueue,
        };

        let queue = SqliteQueue::in_memory().unwrap();
        let queue_id = QueueId::new("workflow:longmemeval");
        let inserted = queue
            .enqueue(EnqueueRequest {
                queue_id: queue_id.clone(),
                kind: "longmemeval.row".to_string(),
                payload: serde_json::json!({"question_id": "q-terminal"}),
                idempotency_key: Some("q-terminal:hash".to_string()),
                run_after: None,
                max_attempts: Some(1),
                force: false,
            })
            .await
            .unwrap()
            .item;
        let claimed = queue
            .claim_item(&inserted.item_id, "worker-a", 60, Some(1))
            .await
            .unwrap()
            .unwrap();
        queue
            .fail(&claimed.item_id, "worker-a", "timeout", Some(1))
            .await
            .unwrap();
        assert_eq!(
            queue
                .get_item(&claimed.item_id)
                .await
                .unwrap()
                .unwrap()
                .status,
            DurableQueueStatus::Dead
        );

        let resumed = enqueue_and_claim_workflow_row(
            &queue,
            queue_id,
            "worker-b",
            "q-terminal",
            "hash",
            false,
            60,
            1,
            3,
            true,
        )
        .await
        .unwrap();

        assert_ne!(resumed.item_id, inserted.item_id);
        assert_eq!(resumed.status, DurableQueueStatus::Running);
    }

    #[cfg(feature = "symbiotic-memory-adapter")]
    #[test]
    fn record_external_scores_writes_benchmark_artifacts_without_mutating_memory_manifest() {
        let dir = tempfile::tempdir().unwrap();
        let run_root = dir.path();
        let vault_dir = run_root.join("vaults").join("q-score");
        fs::create_dir_all(&vault_dir).unwrap();
        let mut manifest = MemoryRunManifest::new("q-score", "source-hash", "policy-v1");
        manifest.begin(MemoryStage::Answer, "answer-input");
        manifest.succeed(MemoryStage::Answer, "answer-output", BTreeMap::new());
        manifest.save(vault_dir.join("manifest.json")).unwrap();

        let hypotheses_path = run_root.join("hyp.jsonl");
        fs::write(
            &hypotheses_path,
            serde_json::to_string(&BenchHypothesis {
                question_id: "q-score".to_string(),
                question_type: Some("single-session-user".to_string()),
                question: "What did I buy?".to_string(),
                hypothesis: "A notebook".to_string(),
                debug_artifact: None,
                router_initial: None,
                router_final: None,
                router_reason: None,
            })
            .unwrap()
                + "\n",
        )
        .unwrap();
        let scored_path = run_root.join("hyp.jsonl.scored.json");
        fs::write(
            &scored_path,
            serde_json::json!({
                "counts": {
                    "scored": 1,
                    "total_correct": 1,
                    "judge_errors": 0
                },
                "overall_accuracy": 1.0,
                "task_averaged_accuracy": 1.0
            })
            .to_string(),
        )
        .unwrap();
        let verdicts_path = run_root.join("hyp.jsonl.verdicts.jsonl");
        fs::write(
            &verdicts_path,
            serde_json::json!({
                "question_id": "q-score",
                "question_type": "single-session-user",
                "label": true,
                "error": null
            })
            .to_string()
                + "\n",
        )
        .unwrap();

        let report = record_external_scores(
            run_root,
            &hypotheses_path,
            Some(scored_path.as_path()),
            Some(verdicts_path.as_path()),
            "test-judge",
        )
        .unwrap();

        assert_eq!(report.hypotheses, 1);
        assert_eq!(report.verdicts, 1);
        assert_eq!(report.debug_files_updated, 0);
        assert!(run_root.join("score-summary.json").is_file());
        let manifest = MemoryRunManifest::load(vault_dir.join("manifest.json"))
            .unwrap()
            .unwrap();
        assert_eq!(manifest.stages.len(), 1);
        assert!(manifest.stage_succeeded(MemoryStage::Answer));
    }

    #[test]
    fn clear_score_artifacts_removes_derived_score_files() {
        let dir = tempfile::tempdir().unwrap();
        let run_root = dir.path();
        let hypotheses_path = run_root.join("hyp.jsonl");
        fs::write(&hypotheses_path, "{}\n").unwrap();
        fs::write(run_root.join("score-summary.json"), "{}").unwrap();
        fs::create_dir_all(run_root.join("scores")).unwrap();
        fs::write(run_root.join("scores").join("old.json"), "{}").unwrap();
        fs::write(run_root.join("hyp.jsonl.scored.json"), "{}").unwrap();
        fs::write(run_root.join("hyp.jsonl.verdicts.jsonl"), "{}\n").unwrap();
        fs::write(run_root.join("hyp.jsonl.partial.verdicts.jsonl"), "{}\n").unwrap();

        let removed = clear_score_artifacts(run_root, &hypotheses_path).unwrap();

        assert_eq!(removed, 5);
        assert!(!run_root.join("score-summary.json").exists());
        assert!(!run_root.join("scores").exists());
        assert!(!run_root.join("hyp.jsonl.scored.json").exists());
        assert!(!run_root.join("hyp.jsonl.verdicts.jsonl").exists());
        assert!(!run_root.join("hyp.jsonl.partial.verdicts.jsonl").exists());
        assert!(hypotheses_path.exists());
    }

    #[cfg(feature = "symbiotic-memory-adapter")]
    #[test]
    fn record_external_scores_rejects_verdicts_outside_current_hypotheses() {
        let dir = tempfile::tempdir().unwrap();
        let hypotheses_path = dir.path().join("hyp.jsonl");
        fs::write(
            &hypotheses_path,
            serde_json::to_string(&BenchHypothesis {
                question_id: "q-owned".to_string(),
                question_type: None,
                question: "Question?".to_string(),
                hypothesis: "Answer".to_string(),
                debug_artifact: None,
                router_initial: None,
                router_final: None,
                router_reason: None,
            })
            .unwrap()
                + "\n",
        )
        .unwrap();
        let verdicts_path = dir.path().join("hyp.jsonl.verdicts.jsonl");
        fs::write(
            &verdicts_path,
            serde_json::json!({"question_id": "q-other", "label": true}).to_string() + "\n",
        )
        .unwrap();

        let err = record_external_scores(
            dir.path(),
            &hypotheses_path,
            None::<&Path>,
            Some(verdicts_path.as_path()),
            "test-judge",
        )
        .unwrap_err()
        .to_string();

        assert!(
            err.contains("not present in current-run hypotheses"),
            "{err}"
        );
    }
}
