use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};
#[cfg(feature = "symbiotic-memory-adapter")]
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fs;
use std::io::{BufRead, Write};
use std::path::Path;
#[cfg(feature = "symbiotic-memory-adapter")]
use std::sync::{Arc, Mutex};
#[cfg(feature = "symbiotic-memory-adapter")]
use std::time::{Duration, Instant};
#[cfg(feature = "symbiotic-memory-adapter")]
use symbiotic_core::{QueueId, QueueItemId};
#[cfg(feature = "symbiotic-memory-adapter")]
use symbiotic_memory::ingest::raw_unit_fingerprint;
use symbiotic_memory::ingest::{Distiller, IngestPipeline};
#[cfg(feature = "symbiotic-memory-adapter")]
use symbiotic_memory::manifest::{MemoryRunManifest, MemoryStage, stable_hash_json};
use symbiotic_memory::providers::{ChatProvider, EmbeddingProvider};
use symbiotic_memory::recall::RecallEngine;
#[cfg(feature = "symbiotic-memory-adapter")]
use symbiotic_memory::recall::{QueryPlanner, RecallAnswerDebug};
use symbiotic_memory::storage::MemoryStore;
#[cfg(feature = "symbiotic-memory-adapter")]
use symbiotic_memory::trace::{MemoryTraceEvent, MemoryTraceEventKind, MemoryTraceSink};
#[cfg(feature = "symbiotic-memory-adapter")]
use symbiotic_memory::types::{FactEvidence, RawTurnEvidence};
use symbiotic_memory::types::{SourceDocument, SourceTurn};
#[cfg(feature = "symbiotic-memory-adapter")]
use symbiotic_queue::{
    EnqueueDisposition, EnqueueRequest, QueueBackend, QueueError, QueueItem,
    QueueStatus as DurableQueueStatus, SqliteQueue,
};

#[derive(Clone, Debug, Deserialize)]
pub struct LongMemEvalRecord {
    pub question_id: String,
    pub question_type: Option<String>,
    pub question: String,
    pub question_date: Option<String>,
    pub answer: Option<Value>,
    pub haystack_dates: Vec<String>,
    pub haystack_session_ids: Vec<String>,
    pub haystack_sessions: Vec<Vec<LongMemEvalMessage>>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct LongMemEvalMessage {
    pub role: String,
    pub content: String,
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
                event_time: session_time,
                text: msg.content.clone(),
                ordinal: turns.len(),
            });
        }
    }
    SourceDocument {
        source_id: record.question_id.clone(),
        source_kind: "longmemeval".to_string(),
        captured_at: first_event_time.unwrap_or_else(Utc::now),
        turns,
        raw_payload: None,
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
    E: EmbeddingProvider + Clone,
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
        let answer = engine
            .answer_with_reference_date(&row.question, row.question_date.as_deref())
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
        None,
        policy,
        out_path,
        routed,
        answer_only,
        consolidate_briefs,
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
    planner_factory: Option<Arc<dyn Fn() -> Arc<dyn QueryPlanner> + Send + Sync>>,
    debug_metadata: Option<BenchDebugMetadata>,
    memory_trace_sink: Option<Arc<dyn MemoryTraceSink>>,
    policy: symbiotic_memory::config::RecallPolicy,
    out_path: impl AsRef<Path>,
    routed: bool,
    answer_only: bool,
    consolidate_briefs: bool,
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
        planner_factory,
        debug_metadata,
        memory_trace_sink,
        policy,
        out_path,
        routed,
        answer_only,
        consolidate_briefs,
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
    planner_factory: Option<Arc<dyn Fn() -> Arc<dyn QueryPlanner> + Send + Sync>>,
    debug_metadata: Option<BenchDebugMetadata>,
    memory_trace_sink: Option<Arc<dyn MemoryTraceSink>>,
    policy: symbiotic_memory::config::RecallPolicy,
    out_path: impl AsRef<Path>,
    routed: bool,
    answer_only: bool,
    consolidate_briefs: bool,
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
    let completed = read_existing_hypothesis_ids(out_path.as_ref())?;
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
    let planner_factory = Arc::new(planner_factory);
    let debug_metadata = Arc::new(debug_metadata);
    let memory_trace_sink = Arc::new(memory_trace_sink);
    let policy = Arc::new(policy);
    let consolidate_briefs = Arc::new(consolidate_briefs);
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
            let planner_factory = planner_factory.clone();
            let debug_metadata = debug_metadata.clone();
            let memory_trace_sink = memory_trace_sink.clone();
            let policy = policy.clone();
            let consolidate_briefs = consolidate_briefs.clone();
            let workflow_queue = workflow_queue.clone();
            let workflow_queue_id = workflow_queue_id.clone();
            let debug_run_id = debug_run_id.clone();
            async move {
                let question_id = row.question_id.clone();
                let input_hash =
                    workflow_input_hash(&row, routed, answer_only, *consolidate_briefs, &policy);
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
                    allow_terminal_reenqueue,
                )
                .await?;
                let heartbeat = spawn_workflow_heartbeat(
                    workflow_queue.clone(),
                    queue_item.item_id.clone(),
                    worker_id.clone(),
                    workflow_lease_seconds,
                );
                let row_result = process_sqlite_row(
                    row,
                    &run_root,
                    &*embedder_factory,
                    &*distiller_factory,
                    consolidator_factory.as_ref().clone(),
                    &*chat_factory,
                    planner_factory.as_ref().clone(),
                    debug_metadata.as_ref().clone(),
                    memory_trace_sink.as_ref().clone(),
                    (*policy).clone(),
                    routed,
                    answer_only,
                    *consolidate_briefs,
                    &debug_run_id,
                );
                let hypothesis = match run_workflow_row(row_result, question_timeout).await {
                    Ok(hypothesis) => hypothesis,
                    Err(err) => {
                        heartbeat.abort();
                        fail_workflow_item(
                            workflow_queue.as_ref(),
                            &queue_item.item_id,
                            &worker_id,
                            &err.to_string(),
                            queue_item.attempt,
                        )
                        .await?;
                        return Err(err);
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
        .buffer_unordered(row_buffer)
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .collect::<anyhow::Result<Vec<_>>>()?;

    Ok(completed_hyps)
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
    planner_factory: Option<Arc<dyn Fn() -> Arc<dyn QueryPlanner> + Send + Sync>>,
    debug_metadata: Option<BenchDebugMetadata>,
    memory_trace_sink: Option<Arc<dyn MemoryTraceSink>>,
    policy: symbiotic_memory::config::RecallPolicy,
    routed: bool,
    answer_only: bool,
    consolidate_briefs: bool,
    debug_run_id: &str,
) -> anyhow::Result<BenchHypothesis>
where
    E: EmbeddingProvider + Clone + Send + Sync + 'static,
    D: Distiller + 'static,
    C: ChatProvider + 'static,
{
    use symbiotic_memory::storage::sqlite::SqliteStore;

    let vault_dir = run_root.join("vaults").join(&row.question_id);
    fs::create_dir_all(&vault_dir)?;
    let source = longmemeval_to_source(&row);
    let source_hash = source_shape_hash(&source)?;
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
        manifest.index_backend = Some("sqlite".to_string());
        manifest
    });
    if manifest.source_hash != source_hash {
        anyhow::bail!(
            "vault {} manifest source hash changed; use a fresh run root",
            row.question_id
        );
    }
    manifest
        .index_backend
        .get_or_insert_with(|| "sqlite".to_string());
    manifest.save(&manifest_path)?;

    let store = SqliteStore::open(vault_dir.join("memory.sqlite"))?;
    let existing_turns = store.turns().await?;
    let existing_facts = store.active_facts().await?;
    if answer_only {
        if existing_turns.is_empty()
            || existing_facts.is_empty()
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
                .with_optional_trace_sink(memory_trace_sink.clone());
        if consolidate_briefs {
            let extractive_config = extractive_brief_config_from_env();
            if extractive_config.max_briefs > 0 {
                ingest = ingest.with_extractive_briefs(extractive_config);
            }
            if let Some(consolidator_factory) = consolidator_factory {
                ingest = ingest.with_consolidator(consolidator_factory());
            }
        }
        ingest.ingest(source.clone()).await?;
        manifest = MemoryRunManifest::load(&manifest_path)?
            .ok_or_else(|| anyhow::anyhow!("ingest did not write {}", manifest_path.display()))?;
    }
    let fact_count = store.active_facts().await?.len();
    let turn_count = store.turns().await?.len();
    if fact_count == 0 {
        anyhow::bail!(
            "vault {} produced zero active facts after ingest",
            row.question_id
        );
    }
    let mut engine = RecallEngine::new(store, embedder_factory(), chat_factory(), policy);
    if let Some(planner_factory) = planner_factory {
        engine = engine.with_query_planner(planner_factory());
    }
    manifest.begin(
        MemoryStage::Answer,
        workflow_input_hash(
            &row,
            routed,
            answer_only,
            consolidate_briefs,
            &engine.policy,
        ),
    );
    manifest.save(&manifest_path)?;
    let answer_started_at = Utc::now();
    trace_memory_stage(
        memory_trace_sink.as_ref(),
        &row.question_id,
        MemoryStage::Answer,
        MemoryTraceEventKind::OperationStarted,
        Some(answer_started_at),
        None,
        Some(workflow_input_hash(
            &row,
            routed,
            answer_only,
            consolidate_briefs,
            &engine.policy,
        )),
        None,
        None,
        serde_json::json!({
            "routed": routed,
            "answer_only": answer_only,
            "consolidate_briefs": consolidate_briefs,
        }),
        None,
    )
    .await?;
    let (answer_text, router_initial, router_final, router_reason, recall_debug) = if routed {
        let (debug, routed) = match engine
            .answer_routed_debug(&row.question, row.question_date.as_deref())
            .await
        {
            Ok(value) => value,
            Err(err) => {
                manifest.fail(MemoryStage::Answer, err.to_string());
                manifest.save(&manifest_path)?;
                trace_memory_stage(
                    memory_trace_sink.as_ref(),
                    &row.question_id,
                    MemoryStage::Answer,
                    MemoryTraceEventKind::OperationFailed,
                    Some(answer_started_at),
                    Some(Utc::now()),
                    Some(workflow_input_hash(
                        &row,
                        routed,
                        answer_only,
                        consolidate_briefs,
                        &engine.policy,
                    )),
                    None,
                    None,
                    serde_json::json!({
                        "routed": routed,
                        "answer_only": answer_only,
                        "consolidate_briefs": consolidate_briefs,
                    }),
                    Some(err.to_string()),
                )
                .await?;
                return Err(err.into());
            }
        };
        (
            debug.final_answer.text.clone(),
            Some(routed.initial_arm.as_str().to_string()),
            Some(routed.final_arm.as_str().to_string()),
            Some(routed.reason),
            Some(debug),
        )
    } else {
        let debug = match engine
            .answer_debug_with_reference_date(&row.question, row.question_date.as_deref())
            .await
        {
            Ok(value) => value,
            Err(err) => {
                manifest.fail(MemoryStage::Answer, err.to_string());
                manifest.save(&manifest_path)?;
                trace_memory_stage(
                    memory_trace_sink.as_ref(),
                    &row.question_id,
                    MemoryStage::Answer,
                    MemoryTraceEventKind::OperationFailed,
                    Some(answer_started_at),
                    Some(Utc::now()),
                    Some(workflow_input_hash(
                        &row,
                        routed,
                        answer_only,
                        consolidate_briefs,
                        &engine.policy,
                    )),
                    None,
                    None,
                    serde_json::json!({
                        "routed": routed,
                        "answer_only": answer_only,
                        "consolidate_briefs": consolidate_briefs,
                    }),
                    Some(err.to_string()),
                )
                .await?;
                return Err(err.into());
            }
        };
        (
            debug.final_answer.text.clone(),
            None,
            None,
            None,
            Some(debug),
        )
    };
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
        router_initial: router_initial.clone(),
        router_final: router_final.clone(),
        router_reason,
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
            workflow_input_hash(
                &row,
                routed,
                answer_only,
                consolidate_briefs,
                &engine.policy,
            ),
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
    trace_memory_stage(
        memory_trace_sink.as_ref(),
        &row.question_id,
        MemoryStage::Answer,
        MemoryTraceEventKind::OperationSucceeded,
        Some(answer_started_at),
        Some(Utc::now()),
        Some(workflow_input_hash(
            &row,
            routed,
            answer_only,
            consolidate_briefs,
            &engine.policy,
        )),
        Some(stable_hash_json(&hypothesis)?),
        Some(fact_count as u64),
        serde_json::json!({
            "routed": routed,
            "answer_only": answer_only,
            "consolidate_briefs": consolidate_briefs,
            "fact_count": fact_count,
            "turn_count": turn_count,
            "router_initial": router_initial,
            "router_final": router_final,
        }),
        None,
    )
    .await?;
    Ok(hypothesis)
}

#[cfg(feature = "symbiotic-memory-adapter")]
#[allow(clippy::too_many_arguments)]
async fn trace_memory_stage(
    sink: Option<&Arc<dyn MemoryTraceSink>>,
    question_id: &str,
    stage: MemoryStage,
    event: MemoryTraceEventKind,
    started_at: Option<DateTime<Utc>>,
    finished_at: Option<DateTime<Utc>>,
    input_hash: Option<String>,
    output_hash: Option<String>,
    item_count: Option<u64>,
    metrics: Value,
    error: Option<String>,
) -> anyhow::Result<()> {
    let Some(sink) = sink else {
        return Ok(());
    };
    let mut trace = MemoryTraceEvent::native_stage(
        question_id.to_string(),
        question_id.to_string(),
        stage,
        event,
    );
    trace.question_id = Some(question_id.to_string());
    trace.started_at = started_at;
    trace.finished_at = finished_at;
    trace.duration_ms = match (started_at, finished_at) {
        (Some(started), Some(finished)) => Some((finished - started).num_milliseconds()),
        _ => None,
    };
    trace.input_hash = input_hash;
    trace.output_hash = output_hash;
    trace.item_count = item_count;
    trace.metrics = metrics;
    trace.error_class = error.as_ref().map(|_| "stage_error".to_string());
    trace.error = error;
    sink.record_memory_event(trace)
        .await
        .map_err(|err| anyhow::anyhow!(err.to_string()))
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
        "gold_positions": gold_position_report(row.answer.as_ref(), recall_debug),
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
fn gold_position_report(gold: Option<&Value>, debug: &RecallAnswerDebug) -> Value {
    let candidates = gold_candidates(gold);
    let mut rows = Vec::new();
    for candidate in &candidates {
        rows.push(serde_json::json!({
            "candidate": candidate,
            "initial_fact_rank": ranked_fact_position(&debug.initial_profile.facts, candidate),
            "initial_raw_turn_rank": ranked_raw_position(&debug.initial_profile.raw_turns, candidate),
            "fallback_fact_rank": debug
                .fallback_profile
                .as_ref()
                .and_then(|profile| ranked_fact_position(&profile.facts, candidate)),
            "fallback_raw_turn_rank": debug
                .fallback_profile
                .as_ref()
                .and_then(|profile| ranked_raw_position(&profile.raw_turns, candidate)),
        }));
    }
    serde_json::json!({
        "method": "case-insensitive normalized substring over gold answer values; forensics only, not a scorer",
        "candidates": rows,
        "first_initial_fact_rank": first_candidate_rank(&candidates, |candidate| {
            ranked_fact_position(&debug.initial_profile.facts, candidate)
        }),
        "first_initial_raw_turn_rank": first_candidate_rank(&candidates, |candidate| {
            ranked_raw_position(&debug.initial_profile.raw_turns, candidate)
        }),
        "first_fallback_raw_turn_rank": debug
            .fallback_profile
            .as_ref()
            .and_then(|profile| first_candidate_rank(&candidates, |candidate| {
                ranked_raw_position(&profile.raw_turns, candidate)
            })),
    })
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn gold_candidates(gold: Option<&Value>) -> Vec<String> {
    let mut out = Vec::new();
    collect_gold_candidates(gold.unwrap_or(&Value::Null), &mut out);
    out.sort();
    out.dedup();
    out
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn collect_gold_candidates(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::String(value) => push_gold_candidate(out, value),
        Value::Number(value) => push_gold_candidate(out, &value.to_string()),
        Value::Bool(value) => push_gold_candidate(out, &value.to_string()),
        Value::Array(values) => {
            for value in values {
                collect_gold_candidates(value, out);
            }
        }
        Value::Object(values) => {
            for value in values.values() {
                collect_gold_candidates(value, out);
            }
        }
        Value::Null => {}
    }
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn push_gold_candidate(out: &mut Vec<String>, value: &str) {
    let normalized = normalize_debug_match(value);
    if !normalized.is_empty() && !out.contains(&normalized) {
        out.push(normalized);
    }
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn ranked_fact_position(facts: &[FactEvidence], candidate: &str) -> Option<usize> {
    facts
        .iter()
        .position(|evidence| normalize_debug_match(&evidence.fact.content).contains(candidate))
        .map(|idx| idx + 1)
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn ranked_raw_position(raw_turns: &[RawTurnEvidence], candidate: &str) -> Option<usize> {
    raw_turns
        .iter()
        .position(|evidence| normalize_debug_match(&evidence.text).contains(candidate))
        .map(|idx| idx + 1)
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn first_candidate_rank<F>(candidates: &[String], mut rank: F) -> Option<usize>
where
    F: FnMut(&str) -> Option<usize>,
{
    candidates
        .iter()
        .filter_map(|candidate| rank(candidate))
        .min()
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn normalize_debug_match(value: &str) -> String {
    value
        .to_ascii_lowercase()
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
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
    E: EmbeddingProvider + Clone,
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
        let (answer_text, router_initial, router_final, router_reason) = if routed {
            let (answer, routed) = engine
                .answer_routed(&row.question, row.question_date.as_deref())
                .await?;
            (
                answer.text,
                Some(routed.initial_arm.as_str().to_string()),
                Some(routed.final_arm.as_str().to_string()),
                Some(routed.reason),
            )
        } else {
            let answer = engine
                .answer_with_reference_date(&row.question, row.question_date.as_deref())
                .await?;
            (answer.text, None, None, None)
        };
        let hypothesis = BenchHypothesis {
            question_id: row.question_id.clone(),
            question_type: row.question_type.clone(),
            question: row.question.clone(),
            hypothesis: answer_text,
            debug_artifact: None,
            router_initial,
            router_final,
            router_reason,
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
fn extractive_brief_config_from_env() -> symbiotic_memory::ExtractiveBriefConfig {
    let default = symbiotic_memory::ExtractiveBriefConfig::default();
    symbiotic_memory::ExtractiveBriefConfig {
        window_turns: env_usize("SYMEM_EXTRACTIVE_BRIEF_WINDOW_TURNS")
            .unwrap_or(default.window_turns)
            .max(1),
        max_turn_chars: env_usize("SYMEM_EXTRACTIVE_BRIEF_MAX_TURN_CHARS")
            .unwrap_or(default.max_turn_chars),
        max_briefs: env_usize("SYMEM_EXTRACTIVE_BRIEF_MAX_BRIEFS").unwrap_or(default.max_briefs),
    }
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn env_usize(name: &str) -> Option<usize> {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
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
    hasher.update(policy.version.as_bytes());
    hasher.update(b"\0");
    hasher.update(
        serde_json::to_string(policy)
            .unwrap_or_else(|_| "unserializable-policy".to_string())
            .as_bytes(),
    );
    hasher.update(b"\0");
    hasher.update(raw_unit_fingerprint().as_bytes());
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
fn source_shape_hash(source: &SourceDocument) -> anyhow::Result<String> {
    stable_hash_json(&serde_json::json!({
        "source": source,
        "raw_unit_shape": raw_unit_fingerprint(),
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
            ),
            (
                "config/symbiotic-memory/longmemeval-raw-tiny.yaml",
                "memory-recall-v3-raw-tiny",
                5usize,
            ),
            (
                "config/symbiotic-memory/longmemeval-raw-wide-diagnostic.yaml",
                "memory-recall-v3-raw-wide-diagnostic",
                80usize,
            ),
        ];

        for (path, version, raw_top_k) in cases {
            let config = symbiotic_memory::MemoryConfig::load_yaml(root.join(path)).unwrap();
            let resolved = config
                .queue
                .resolve_provider_queue(&config.providers.embedding);

            assert_eq!(config.recall.version, version);
            assert_eq!(config.recall.fact_top_k, 20);
            assert_eq!(config.recall.raw_turn_top_k, raw_top_k);
            assert_eq!(config.recall.raw_fallback_top_k, raw_top_k);
            assert_eq!(config.queue.workflow_max_in_flight, 50);
            assert_eq!(resolved.queue_id, "embedding:gemini:gemini-embedding-2");
            assert_eq!(resolved.max_in_flight, 400);
            assert_eq!(resolved.timeout_seconds, 300);
        }
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
            source.turns[0].event_time.unwrap().to_rfc3339(),
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
        let mut scripted = off.clone();
        scripted.query_planner = symbiotic_memory::config::QueryPlannerMode::Scripted;

        assert_ne!(
            workflow_input_hash(&row, false, false, false, &off),
            workflow_input_hash(&row, false, false, false, &scripted)
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
            workflow_input_hash(&row, true, true, false, &raw40),
            workflow_input_hash(&row, true, true, false, &raw80)
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
            workflow_input_hash(&row, true, false, false, &policy),
            workflow_input_hash(&row, true, false, true, &policy)
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
                    .with_captured_at(turn.event_time),
                ],
            );
            fact.event_time = turn.event_time;
            fact.valid_from = turn.event_time;
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
            policy,
            &out,
            false,
            false,
            false,
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
                    .with_captured_at(turn.event_time),
                ],
            );
            fact.event_time = turn.event_time;
            fact.valid_from = turn.event_time;
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
        use symbiotic_memory::ingest::HeuristicDistiller;
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
            || HeuristicDistiller,
            || DisabledChatProvider,
            policy,
            &out,
            false,
            false,
            false,
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
        use symbiotic_memory::ingest::HeuristicDistiller;
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
            || HeuristicDistiller,
            || DisabledChatProvider,
            policy.clone(),
            &first_out,
            false,
            false,
            false,
            false,
        )
        .await
        .unwrap();

        let vault_dir = dir.path().join("vaults").join("q-answer-only");
        let before = MemoryRunManifest::load(vault_dir.join("manifest.json"))
            .unwrap()
            .unwrap();
        let distill_finished_at = before.stages[&MemoryStage::DistillWindow].finished_at;

        run_longmemeval_sqlite(
            &[row],
            dir.path(),
            HashEmbeddingProvider::default,
            || HeuristicDistiller,
            || DisabledChatProvider,
            policy,
            &answer_only_out,
            false,
            true,
            false,
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
    }

    #[cfg(feature = "symbiotic-memory-adapter")]
    #[tokio::test]
    async fn sqlite_benchmark_can_consolidate_extractive_briefs() {
        use symbiotic_memory::config::RecallPolicy;
        use symbiotic_memory::ingest::HeuristicDistiller;
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
            || HeuristicDistiller,
            || DisabledChatProvider,
            policy,
            &out,
            false,
            false,
            true,
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
