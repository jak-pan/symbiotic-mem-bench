//! Symbiotic Memory application-facade adapter.
//!
//! This module deliberately knows nothing about Memory's stores, manifests,
//! indexes, or pipeline implementation. It supplies providers and benchmark
//! records to `MemoryEngine`, then persists only facade results and diagnostics.

use chrono::{DateTime, Local, NaiveDateTime, SecondsFormat, TimeZone, Utc};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use symbiotic_core::{QueueId, QueueItemId};

use symbiotic_memory::{
    Actor, AllowAll, ArchiveMode, ChatProvider, Distiller, EmbeddingProvider,
    MemoryDetailedIngestRequest, MemoryDetailedRecallRequest, MemoryDiagnosticMode, MemoryEngine,
    MemoryEngineConfig, MemoryEngineServices, MemoryIngestExecutionStatus, MemoryIngestMode,
    MemoryIngestRequest, MemoryOperationContext, MemoryProviders, MemoryRecallDiagnostics,
    MemoryRecallRequest, MemorySessionContext, MemoryTraceSink, MemoryVault, QueryPlanner,
    RecallPolicy, Reranker, Scope, Sensitivity, SourceDocument, SourceTurn,
};
use symbiotic_queue::{
    EnqueueDisposition, EnqueueRequest, QueueBackend, QueueError, QueueItem,
    QueueStatus as DurableQueueStatus, SqliteQueue,
};

/// Reranker services installed into the stable Memory facade.
#[derive(Clone, Default)]
pub struct RerankCascade {
    pub main: Option<Arc<dyn Reranker>>,
    pub stage1: Option<Arc<dyn Reranker>>,
    pub stage1_top_x: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LongMemEvalRecord {
    pub question_id: String,
    pub question_type: Option<String>,
    pub question: String,
    pub question_date: Option<String>,
    pub answer: Option<Value>,
    #[serde(default)]
    pub answer_session_ids: Vec<String>,
    pub haystack_dates: Vec<String>,
    pub haystack_session_ids: Vec<String>,
    pub haystack_sessions: Vec<Vec<LongMemEvalMessage>>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LongMemEvalMessage {
    pub role: String,
    pub content: String,
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SharedCorpusQuestion {
    pub id: String,
    pub question: String,
    pub question_type: Option<String>,
    pub reference_date: Option<String>,
    pub corpus_key: String,
}

pub type EmbeddingFactory = Arc<dyn Fn() -> Arc<dyn EmbeddingProvider> + Send + Sync>;
pub type DistillerFactory = Arc<dyn Fn() -> Arc<dyn Distiller> + Send + Sync>;
pub type ChatFactory = Arc<dyn Fn() -> Arc<dyn ChatProvider> + Send + Sync>;
pub type PlannerFactory = Arc<dyn Fn() -> Arc<dyn QueryPlanner> + Send + Sync>;

#[derive(Clone)]
pub struct MemoryFacadeProviders {
    pub embedder: EmbeddingFactory,
    pub distiller: DistillerFactory,
    pub chat: ChatFactory,
    pub planner: Option<PlannerFactory>,
    pub reranker: RerankCascade,
    pub trace_sink: Option<Arc<dyn MemoryTraceSink>>,
}

#[derive(Clone)]
pub struct MemoryFacadeRun {
    pub run_id: String,
    pub run_root: PathBuf,
    pub source_vault_root: Option<PathBuf>,
    pub out_path: PathBuf,
    pub policy: RecallPolicy,
    pub debug_metadata: Option<BenchDebugMetadata>,
    pub answer_only: bool,
    pub ingest_mode: MemoryIngestMode,
    pub max_in_flight: usize,
    pub allow_terminal_reenqueue: bool,
}

static KIT_CONFIG: OnceLock<symbiotic_memory::profile::MemoryConfig> = OnceLock::new();
static KIT_CONFIG_HASH: OnceLock<String> = OnceLock::new();
static ACTIVE_MANIFEST_TAG: OnceLock<String> = OnceLock::new();

pub fn set_kit_config(
    config: symbiotic_memory::profile::MemoryConfig,
    config_hash: impl Into<String>,
) -> anyhow::Result<()> {
    KIT_CONFIG
        .set(config)
        .map_err(|_| anyhow::anyhow!("Memory profile was already set"))?;
    KIT_CONFIG_HASH
        .set(config_hash.into())
        .map_err(|_| anyhow::anyhow!("Memory profile hash was already set"))
}

pub fn kit_config() -> &'static symbiotic_memory::profile::MemoryConfig {
    static DEFAULT: OnceLock<symbiotic_memory::profile::MemoryConfig> = OnceLock::new();
    KIT_CONFIG
        .get()
        .unwrap_or_else(|| DEFAULT.get_or_init(symbiotic_memory::profile::MemoryConfig::default))
}

fn kit_config_hash() -> String {
    KIT_CONFIG_HASH.get().cloned().unwrap_or_else(|| {
        let value = serde_json::to_vec(kit_config()).unwrap_or_default();
        format!("sha256:{}", hex::encode(Sha256::digest(value)))
    })
}

pub fn set_active_manifest_tag(tag: impl Into<String>) -> anyhow::Result<()> {
    let tag = tag.into();
    if let Some(active) = ACTIVE_MANIFEST_TAG.get() {
        anyhow::ensure!(active == &tag, "benchmark identity is already `{active}`");
        return Ok(());
    }
    ACTIVE_MANIFEST_TAG
        .set(tag)
        .map_err(|_| anyhow::anyhow!("benchmark identity was set concurrently"))
}

fn active_manifest_tag() -> &'static str {
    ACTIVE_MANIFEST_TAG
        .get()
        .map(String::as_str)
        .unwrap_or("longmemeval-v1")
}

pub fn load_longmemeval(
    path: impl AsRef<Path>,
    limit: Option<usize>,
) -> anyhow::Result<Vec<LongMemEvalRecord>> {
    let mut rows: Vec<LongMemEvalRecord> = serde_json::from_str(&fs::read_to_string(path)?)?;
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
        first_event_time = first_event_time.or(session_time);
        for (message_idx, message) in session.iter().enumerate() {
            turns.push(SourceTurn {
                turn_id: format!("{session_id}:{message_idx}"),
                source_id: record.question_id.clone(),
                speaker: Some(message.role.clone()),
                actor: None,
                captured_at: session_time,
                event_time: None,
                ingested_at: None,
                text: message.content.clone(),
                ordinal: turns.len(),
                locator: None,
                scope: Scope::default(),
            });
        }
    }
    SourceDocument {
        source_id: record.question_id.clone(),
        source_kind: "longmemeval".to_string(),
        captured_at: first_event_time.unwrap_or_else(|| {
            Utc.timestamp_opt(0, 0)
                .single()
                .expect("Unix epoch is valid")
        }),
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
        .map(|datetime| Utc.from_utc_datetime(&datetime))
}

fn reference_datetime(row: &LongMemEvalRecord) -> String {
    std::env::var("MEMBENCH_REFERENCE_DATETIME")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            row.question_date
                .as_deref()
                .and_then(parse_longmemeval_datetime)
                .map(|value| value.to_rfc3339_opts(SecondsFormat::Secs, false))
        })
        .unwrap_or_else(|| Local::now().to_rfc3339_opts(SecondsFormat::Secs, false))
}

pub async fn run_longmemeval_facade(
    rows: &[LongMemEvalRecord],
    providers: MemoryFacadeProviders,
    run: MemoryFacadeRun,
) -> anyhow::Result<Vec<BenchHypothesis>> {
    if let Some(parent) = run.out_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let existing = read_hypotheses(&run.out_path)?;
    let completed: BTreeSet<_> = existing
        .iter()
        .map(|hypothesis| hypothesis.question_id.clone())
        .collect();
    anyhow::ensure!(
        completed.len() == existing.len(),
        "hypotheses contain duplicate question ids"
    );
    let mut output_file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&run.out_path)?;
    let workflow_dir = run.run_root.join("workflow").join("longmemeval");
    fs::create_dir_all(&workflow_dir)?;
    let workflow_queue: Arc<dyn QueueBackend> =
        Arc::new(SqliteQueue::open(workflow_dir.join("queue.sqlite"))?);
    let workflow_queue_id = QueueId::new(format!("workflow:{}", run.run_id));
    let lease_seconds = question_timeout()
        .map(|timeout| timeout.as_secs().saturating_add(60).max(60))
        .unwrap_or(660);
    let max_attempts = workflow_max_attempts();
    let pending: Vec<_> = rows
        .iter()
        .filter(|row| {
            !completed.contains(&row.question_id)
                && !(run.ingest_mode != MemoryIngestMode::Complete
                    && ingest_diagnostic_path(&run.run_root, &row.question_id).is_file())
        })
        .cloned()
        .collect();
    let mut tasks = futures::stream::iter(pending.into_iter().map(|row| {
        let providers = providers.clone();
        let run = run.clone();
        let workflow_queue = workflow_queue.clone();
        let workflow_queue_id = workflow_queue_id.clone();
        async move {
            let worker_id = workflow_worker_id(&row.question_id);
            let input_hash = workflow_input_hash(&row, &run)?;
            let queue_item = enqueue_and_claim_workflow_row(
                workflow_queue.as_ref(),
                workflow_queue_id,
                &worker_id,
                &row.question_id,
                &input_hash,
                lease_seconds,
                run.max_in_flight,
                max_attempts,
                run.allow_terminal_reenqueue,
            )
            .await?;
            let heartbeat = spawn_workflow_heartbeat(
                workflow_queue.clone(),
                queue_item.item_id.clone(),
                worker_id.clone(),
                lease_seconds,
            );
            let result =
                run_workflow_row(process_record(row, providers, run), question_timeout()).await;
            match result {
                Ok(outcome) => Ok(ClaimedRecordOutcome {
                    outcome,
                    queue_item,
                    worker_id,
                    heartbeat,
                }),
                Err(error) => {
                    heartbeat.abort();
                    fail_workflow_item(
                        workflow_queue.as_ref(),
                        &queue_item.item_id,
                        &worker_id,
                        &error.to_string(),
                        queue_item.attempt,
                    )
                    .await?;
                    Err(error)
                }
            }
        }
    }))
    .buffer_unordered(run.max_in_flight.max(1));
    while let Some(result) = tasks.next().await {
        let claimed = result?;
        let output_result = (|| -> anyhow::Result<()> {
            if let Some(hypothesis) = claimed.outcome.as_ref() {
                writeln!(output_file, "{}", serde_json::to_string(hypothesis)?)?;
                output_file.flush()?;
            }
            Ok(())
        })();
        if let Err(error) = output_result {
            claimed.heartbeat.abort();
            fail_workflow_item(
                workflow_queue.as_ref(),
                &claimed.queue_item.item_id,
                &claimed.worker_id,
                &error.to_string(),
                claimed.queue_item.attempt,
            )
            .await?;
            return Err(error);
        }
        let complete_result = workflow_queue
            .complete(&claimed.queue_item.item_id, &claimed.worker_id)
            .await;
        claimed.heartbeat.abort();
        complete_result.map_err(queue_error)?;
    }
    read_hypotheses(&run.out_path)
}

struct ClaimedRecordOutcome {
    outcome: Option<BenchHypothesis>,
    queue_item: QueueItem,
    worker_id: String,
    heartbeat: tokio::task::JoinHandle<()>,
}

async fn process_record(
    row: LongMemEvalRecord,
    providers: MemoryFacadeProviders,
    run: MemoryFacadeRun,
) -> anyhow::Result<Option<BenchHypothesis>> {
    let state_root = match &run.source_vault_root {
        Some(source_root) => source_root.join(&row.question_id),
        None => run.run_root.join("vaults").join(&row.question_id),
    };
    if run.answer_only {
        anyhow::ensure!(
            state_root.is_dir(),
            "answer-only Memory state does not exist for {} at {}",
            row.question_id,
            state_root.display()
        );
    } else {
        fs::create_dir_all(&state_root)?;
    }
    let services = MemoryEngineServices {
        reranker: providers.reranker.main.clone(),
        prefilter_reranker: providers.reranker.stage1.clone(),
        prefilter_limit: providers
            .reranker
            .stage1
            .is_some()
            .then_some(providers.reranker.stage1_top_x.max(1)),
        query_planner: providers.planner.as_ref().map(|factory| factory()),
        trace_sink: providers.trace_sink.clone(),
    };
    let engine = MemoryEngine::open_with_services(
        MemoryEngineConfig {
            vault: MemoryVault::Persistent(state_root.clone()),
            archive: ArchiveMode::VaultDefault,
            profile: kit_config().clone(),
            recall_policy: run.policy.clone(),
            access_policy: Arc::new(AllowAll),
            config_hash: format!("{}:{}", active_manifest_tag(), kit_config_hash()),
        },
        MemoryProviders {
            embedder: (providers.embedder)(),
            chat: (providers.chat)(),
            distiller: (providers.distiller)(),
        },
        services,
    )?;
    let space = format!("benchmark:{}", active_manifest_tag());
    let session = engine.session(
        MemorySessionContext::new(
            format!("{}:{}", run.run_id, row.question_id),
            Actor::Agent("membench".to_string()),
            space.clone(),
        )
        .with_write_scope(Scope::space(space))
        .with_egress(Sensitivity::Restricted),
    );
    let operation = MemoryOperationContext::new(
        run.run_id.clone(),
        row.question_id.clone(),
        row.question_id.clone(),
    );
    if !run.answer_only {
        let ingest = session
            .ingest_detailed(MemoryDetailedIngestRequest {
                ingest: MemoryIngestRequest {
                    source: longmemeval_to_source(&row),
                    fact_tags: vec![format!("benchmark:{}", active_manifest_tag())],
                },
                mode: run.ingest_mode,
                operation: Some(operation.clone()),
            })
            .await?;
        if ingest.status == MemoryIngestExecutionStatus::DiagnosticStop {
            write_ingest_diagnostic(&run.run_root, &row.question_id, &ingest)?;
            return Ok(None);
        }
    }
    let recalled = session
        .recall_detailed(MemoryDetailedRecallRequest {
            recall: MemoryRecallRequest {
                question: row.question.clone(),
                reference_date: Some(reference_datetime(&row)),
            },
            diagnostics: MemoryDiagnosticMode::Full,
            operation: Some(operation),
        })
        .await?;
    let diagnostics = recalled
        .diagnostics
        .ok_or_else(|| anyhow::anyhow!("Memory returned no requested recall diagnostics"))?;
    let debug_artifact = write_question_debug(
        &run.run_root,
        &row,
        &diagnostics,
        &recalled.recall.answer,
        run.debug_metadata.as_ref(),
    )?;
    Ok(Some(BenchHypothesis {
        question_id: row.question_id,
        question_type: row.question_type,
        question: row.question,
        hypothesis: recalled.recall.answer.text,
        debug_artifact: Some(debug_artifact),
        router_initial: None,
        router_final: None,
        router_reason: None,
    }))
}

fn write_ingest_diagnostic(
    run_root: &Path,
    question_id: &str,
    result: &symbiotic_memory::MemoryDetailedIngestResult,
) -> anyhow::Result<()> {
    let path = ingest_diagnostic_path(run_root, question_id);
    write_json_atomic(&path, result)
}

fn ingest_diagnostic_path(run_root: &Path, question_id: &str) -> PathBuf {
    run_root
        .join("artifacts")
        .join("ingest-diagnostics")
        .join(format!("{question_id}.json"))
}

fn workflow_input_hash(row: &LongMemEvalRecord, run: &MemoryFacadeRun) -> anyhow::Result<String> {
    let mut hasher = Sha256::new();
    hasher.update(serde_json::to_vec(row)?);
    hasher.update(b"\0");
    hasher.update(serde_json::to_vec(&run.policy)?);
    hasher.update(b"\0");
    hasher.update(serde_json::to_vec(&run.ingest_mode)?);
    hasher.update(b"\0");
    hasher.update(if run.answer_only {
        b"answer-only".as_slice()
    } else {
        b"ingest-and-recall".as_slice()
    });
    hasher.update(b"\0");
    hasher.update(active_manifest_tag().as_bytes());
    hasher.update(b"\0");
    hasher.update(kit_config_hash().as_bytes());
    Ok(hex::encode(hasher.finalize()))
}

async fn enqueue_and_claim_workflow_row(
    queue: &dyn QueueBackend,
    queue_id: QueueId,
    worker_id: &str,
    question_id: &str,
    input_hash: &str,
    lease_seconds: u64,
    max_in_flight: usize,
    max_attempts: u32,
    allow_terminal_reenqueue: bool,
) -> anyhow::Result<QueueItem> {
    let request = EnqueueRequest {
        queue_id,
        kind: "longmemeval.row".to_string(),
        payload: json!({
            "question_id": question_id,
            "input_hash": input_hash,
        }),
        idempotency_key: Some(format!("{question_id}:{input_hash}")),
        run_after: None,
        max_attempts: Some(max_attempts.max(1)),
        force: false,
    };
    let mut outcome = queue.enqueue(request.clone()).await.map_err(queue_error)?;
    if matches!(outcome.disposition, EnqueueDisposition::TerminalDuplicate) {
        anyhow::ensure!(
            allow_terminal_reenqueue,
            "workflow row {question_id} is terminal without an output; use --resume to re-enqueue it or use a fresh run root"
        );
        let mut forced = request;
        forced.force = true;
        outcome = queue.enqueue(forced).await.map_err(queue_error)?;
        anyhow::ensure!(
            matches!(outcome.disposition, EnqueueDisposition::Inserted),
            "workflow row {question_id} could not be force re-enqueued"
        );
    }
    for _ in 0..lease_seconds.saturating_mul(4).clamp(4, 2_400) {
        if let Some(claimed) = queue
            .claim_item(
                &outcome.item.item_id,
                worker_id,
                lease_seconds,
                Some(max_in_flight.max(1)),
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
                DurableQueueStatus::Succeeded => anyhow::bail!(
                    "workflow row {question_id} completed before this worker wrote its output"
                ),
                DurableQueueStatus::Dead => anyhow::bail!(
                    "workflow row {question_id} is dead-lettered: {}",
                    current
                        .last_error
                        .unwrap_or_else(|| "unknown error".to_string())
                ),
                DurableQueueStatus::Running
                | DurableQueueStatus::Pending
                | DurableQueueStatus::Failed => {}
            }
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    anyhow::bail!("workflow row {question_id} could not acquire a lease within {lease_seconds}s")
}

fn spawn_workflow_heartbeat(
    queue: Arc<dyn QueueBackend>,
    item_id: QueueItemId,
    worker_id: String,
    lease_seconds: u64,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let interval = Duration::from_secs((lease_seconds / 3).clamp(5, 60));
        loop {
            tokio::time::sleep(interval).await;
            if let Err(error) = queue.heartbeat(&item_id, &worker_id, lease_seconds).await {
                eprintln!(
                    "[longmemeval] workflow heartbeat stopped for {}: {error}",
                    item_id.0
                );
                break;
            }
        }
    })
}

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

fn workflow_worker_id(question_id: &str) -> String {
    let sanitized = question_id
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect::<String>();
    format!("symem-{}-{sanitized}", std::process::id())
}

fn workflow_max_attempts() -> u32 {
    std::env::var("MEMBENCH_WORKFLOW_MAX_ATTEMPTS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(3)
        .max(1)
}

fn workflow_retry_delay_seconds(attempt: u32) -> u64 {
    let base = std::env::var("MEMBENCH_WORKFLOW_RETRY_DELAY_SECS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(2_u64)
        .max(1);
    base.saturating_mul(1_u64 << attempt.saturating_sub(1).min(5))
        .min(60)
}

fn question_timeout() -> Option<Duration> {
    std::env::var("MEMBENCH_QUESTION_TIMEOUT_SECS")
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|seconds| *seconds > 0)
        .map(Duration::from_secs)
}

async fn run_workflow_row<F, T>(future: F, timeout: Option<Duration>) -> anyhow::Result<T>
where
    F: std::future::Future<Output = anyhow::Result<T>>,
{
    match timeout {
        Some(timeout) => tokio::time::timeout(timeout, future)
            .await
            .map_err(|_| anyhow::anyhow!("workflow row timed out after {}s", timeout.as_secs()))?,
        None => future.await,
    }
}

fn queue_error(error: QueueError) -> anyhow::Error {
    anyhow::anyhow!("workflow queue error: {error}")
}

fn write_question_debug(
    run_root: &Path,
    row: &LongMemEvalRecord,
    diagnostics: &MemoryRecallDiagnostics,
    answer: &symbiotic_memory::Answer,
    metadata: Option<&BenchDebugMetadata>,
) -> anyhow::Result<String> {
    let relative = PathBuf::from("vaults")
        .join(&row.question_id)
        .join("debug")
        .join("facade")
        .join("question-debug.json");
    let path = run_root.join(&relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let value = json!({
        "schema_version": 2,
        "question_id": row.question_id,
        "question_type": row.question_type,
        "question": row.question,
        "memory_contract": "application-facade",
        "recall": diagnostics,
        "final_answer": answer,
        "benchmark": metadata,
    });
    write_json_atomic(&path, &value)?;
    Ok(relative.to_string_lossy().replace('\\', "/"))
}

fn read_hypotheses(path: &Path) -> anyhow::Result<Vec<BenchHypothesis>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let file = fs::File::open(path)?;
    std::io::BufReader::new(file)
        .lines()
        .filter(|line| line.as_ref().map_or(true, |line| !line.trim().is_empty()))
        .map(|line| Ok(serde_json::from_str(&line?)?))
        .collect()
}

fn write_json_atomic(path: &Path, value: &impl Serialize) -> anyhow::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("{} has no parent", path.display()))?;
    fs::create_dir_all(parent)?;
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    serde_json::to_writer_pretty(&mut temp, value)?;
    temp.write_all(b"\n")?;
    temp.as_file_mut().sync_all()?;
    temp.persist(path)?;
    Ok(())
}

pub fn clear_score_artifacts(
    run_root: impl AsRef<Path>,
    hypotheses_path: impl AsRef<Path>,
) -> anyhow::Result<usize> {
    let run_root = run_root.as_ref();
    let hypotheses = hypotheses_path.as_ref().to_string_lossy();
    let mut paths = vec![run_root.join("score-summary.json"), run_root.join("scores")];
    for directory in ["raw", "artifacts"] {
        for file in [
            "verdicts.jsonl",
            "partial-verdicts.jsonl",
            "scored.json",
            "score-summary.json",
        ] {
            paths.push(run_root.join(directory).join(file));
        }
    }
    for suffix in [".scored.json", ".verdicts.jsonl", ".partial.verdicts.jsonl"] {
        paths.push(PathBuf::from(format!("{hypotheses}{suffix}")));
    }
    let mut removed = 0;
    for path in paths {
        if path.is_dir() {
            fs::remove_dir_all(path)?;
            removed += 1;
        } else if path.exists() {
            fs::remove_file(path)?;
            removed += 1;
        }
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_row() -> LongMemEvalRecord {
        LongMemEvalRecord {
            question_id: "q1".to_string(),
            question_type: None,
            question: "Where?".to_string(),
            question_date: None,
            answer: None,
            answer_session_ids: Vec::new(),
            haystack_dates: vec!["2024/01/02 03:04 (Tue)".to_string()],
            haystack_session_ids: vec!["s1".to_string()],
            haystack_sessions: vec![vec![LongMemEvalMessage {
                role: "user".to_string(),
                content: "At home".to_string(),
                has_answer: true,
            }]],
        }
    }

    fn sample_run(root: &Path) -> MemoryFacadeRun {
        MemoryFacadeRun {
            run_id: "test-run".to_string(),
            run_root: root.to_path_buf(),
            source_vault_root: None,
            out_path: root.join("hypotheses.jsonl"),
            policy: RecallPolicy::default(),
            debug_metadata: None,
            answer_only: false,
            ingest_mode: MemoryIngestMode::Complete,
            max_in_flight: 1,
            allow_terminal_reenqueue: false,
        }
    }

    fn local_providers() -> MemoryFacadeProviders {
        MemoryFacadeProviders {
            embedder: Arc::new(|| Arc::new(symbiotic_memory::HashEmbeddingProvider::default())),
            distiller: Arc::new(|| Arc::new(symbiotic_memory::PassthroughDistiller)),
            chat: Arc::new(|| {
                Arc::new(symbiotic_memory::providers::DisabledChatProvider) as Arc<dyn ChatProvider>
            }),
            planner: None,
            reranker: RerankCascade::default(),
            trace_sink: None,
        }
    }

    #[test]
    fn conversion_is_stable_and_preserves_capture_time() {
        let row = sample_row();
        let first = longmemeval_to_source(&row);
        let second = longmemeval_to_source(&row);
        assert_eq!(
            first.identity_digest().unwrap(),
            second.identity_digest().unwrap()
        );
        assert_eq!(first.turns[0].turn_id, "s1:0");
        assert!(first.turns[0].event_time.is_none());
    }

    #[test]
    fn conversion_without_dates_is_still_deterministic() {
        let mut row = sample_row();
        row.haystack_dates.clear();
        let first = longmemeval_to_source(&row);
        let second = longmemeval_to_source(&row);
        assert_eq!(first.captured_at, Utc.timestamp_opt(0, 0).single().unwrap());
        assert_eq!(
            first.identity_digest().unwrap(),
            second.identity_digest().unwrap()
        );
    }

    #[tokio::test]
    async fn answer_only_requires_existing_opaque_memory_state() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source-vaults");
        fs::create_dir_all(&source).unwrap();
        let mut run = sample_run(temp.path());
        run.answer_only = true;
        run.source_vault_root = Some(source.clone());

        let error = process_record(sample_row(), local_providers(), run)
            .await
            .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("answer-only Memory state does not exist")
        );
        assert!(!source.join("q1").exists());
    }

    #[tokio::test]
    async fn durable_workflow_refuses_terminal_duplicate_without_resume() {
        let queue = SqliteQueue::in_memory().unwrap();
        let queue_id = QueueId::new("workflow:test");
        let first = enqueue_and_claim_workflow_row(
            &queue,
            queue_id.clone(),
            "worker-1",
            "q1",
            "input-hash",
            60,
            1,
            3,
            false,
        )
        .await
        .unwrap();
        queue.complete(&first.item_id, "worker-1").await.unwrap();

        let error = enqueue_and_claim_workflow_row(
            &queue,
            queue_id,
            "worker-2",
            "q1",
            "input-hash",
            60,
            1,
            3,
            false,
        )
        .await
        .unwrap_err();

        assert!(error.to_string().contains("terminal without an output"));
    }

    #[test]
    fn workflow_identity_changes_when_source_changes() {
        let temp = tempfile::tempdir().unwrap();
        let run = sample_run(temp.path());
        let first = sample_row();
        let mut second = first.clone();
        second.haystack_sessions[0][0].content = "Somewhere else".to_string();

        assert_ne!(
            workflow_input_hash(&first, &run).unwrap(),
            workflow_input_hash(&second, &run).unwrap()
        );
    }

    #[test]
    fn score_cleanup_is_scoped_to_derived_outputs() {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir_all(temp.path().join("artifacts")).unwrap();
        fs::write(temp.path().join("artifacts/scored.json"), "{}").unwrap();
        fs::write(temp.path().join("source.txt"), "keep").unwrap();
        let removed =
            clear_score_artifacts(temp.path(), temp.path().join("hypotheses.jsonl")).unwrap();
        assert_eq!(removed, 1);
        assert!(temp.path().join("source.txt").exists());
    }
}
