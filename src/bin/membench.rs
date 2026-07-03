#![recursion_limit = "256"]

#[cfg(feature = "symbiotic-memory-adapter")]
use anyhow::Context;
use chrono::Utc;
use clap::{Parser, Subcommand};
#[cfg(feature = "symbiotic-memory-adapter")]
use futures::StreamExt;
#[cfg(feature = "symbiotic-memory-adapter")]
use reqwest::Client;
#[cfg(feature = "symbiotic-memory-adapter")]
use serde::Serialize;
#[cfg(feature = "symbiotic-memory-adapter")]
use serde_json::Value;
use serde_json::json;
use sha2::{Digest, Sha256};
#[cfg(feature = "symbiotic-memory-adapter")]
use std::collections::VecDeque;
use std::collections::{BTreeMap, BTreeSet};
#[cfg(feature = "symbiotic-memory-adapter")]
use std::io::Write;
use std::path::{Path, PathBuf};
#[cfg(feature = "symbiotic-memory-adapter")]
use std::sync::Arc;
#[cfg(feature = "symbiotic-memory-adapter")]
use std::time::{Duration, Instant};
use symbiotic_mem_bench::{
    BenchQueueEvent, cost, registry, runner, step_analytics, summarize_queue_timing, trials,
};

const LONGMEMEVAL_CLEANED_S_URL: &str = "https://huggingface.co/datasets/xiaowu0162/longmemeval-cleaned/resolve/main/longmemeval_s_cleaned.json";
const LONGMEMEVAL_CLEANED_S_BYTES: u64 = 277_383_467;

const COMMON_ARTIFACT_KINDS: &[&str] = &[
    "hypotheses",
    "scored",
    "verdicts",
    "partial_verdicts",
    "provenance",
    "memory_traces",
    "model_traces",
    "step_analytics",
    "score_summary",
];

#[derive(Parser)]
#[command(name = "membench")]
#[command(version)]
#[command(about = "Memory-system benchmark orchestrator")]
struct Cli {
    #[arg(long, alias = "model")]
    system: Option<String>,
    #[arg(long)]
    benchmark: Option<String>,
    #[arg(long)]
    symbiotic_memory: bool,
    #[arg(long)]
    long_mem_eval: bool,
    /// Run the explicit local no-network smoke mode instead of the paid provider-backed benchmark.
    #[arg(long)]
    smoke: bool,
    #[arg(long)]
    dataset: Option<PathBuf>,
    #[arg(long)]
    run_root: Option<PathBuf>,
    #[arg(long, default_value = "runs")]
    registry_root: PathBuf,
    #[arg(long, default_value_t = 10)]
    limit: usize,
    #[arg(long, default_value = "stratified")]
    sample: String,
    #[arg(long, default_value = "../symbiotic-memory/Cargo.toml")]
    memory_manifest: PathBuf,
    #[arg(
        long,
        default_value = "config/symbiotic-memory/longmemeval-raw-light.yaml"
    )]
    memory_config: Option<PathBuf>,
    #[arg(long)]
    symem_bin: Option<PathBuf>,
    #[arg(long, default_value = "llm")]
    distiller: String,
    /// Owner-default stack: qwen3-embedding-8b via OpenRouter (+ the nemotron
    /// free reranker). Gemini embeddings are NOT the default.
    #[arg(long, default_value = "openrouter")]
    embedder: String,
    #[arg(long, default_value = "zvec-hybrid")]
    store: String,
    #[arg(long)]
    prompt_dir: Option<PathBuf>,
    #[arg(long, default_value = "distill")]
    distill_prompt: String,
    /// Enable the memory engine's generative answerer policy. LongMemEval still writes hypotheses when this is off.
    #[arg(long)]
    answerer: bool,
    #[arg(long)]
    no_answerer: bool,
    #[arg(long)]
    routed: bool,
    #[arg(long)]
    no_routed: bool,
    #[arg(long)]
    answer_only: bool,
    /// Re-judge an existing run's stored hypotheses with the current judge (default: the official
    /// per-question-type LongMemEval grader) WITHOUT re-answering. Reuses the run root named by
    /// --run-name, reads its hypotheses.jsonl, rewrites verdicts/score-summary/report. Cheap (judge
    /// calls only); swap graders via SYMEM_JUDGE_PROMPT_MODE.
    #[arg(long)]
    rejudge: bool,
    /// Gold-oracle answer mode: feed the answerer ONLY the gold-session raw turns (zero retrieval,
    /// zero noise) instead of the recall→rerank output, isolating the reader from retrieval. Recall
    /// still runs (the question-debug profile stays populated) but its evidence never reaches the
    /// answerer. Same answerer/judge/scoring path as a normal run — the only variable is clean-gold
    /// vs retrieved-noisy context. Also honored via `SYMEM_ORACLE_GOLD=1`.
    #[arg(long)]
    oracle_gold: bool,
    /// Re-embed an existing vault's facts+turns (from --source-vault-root) with the current
    /// embedder/enrichment and rebuild the index — reuses distill, no LLM re-distill.
    #[arg(long)]
    re_embed: bool,
    #[arg(long)]
    consolidate_briefs: bool,
    #[arg(long)]
    no_consolidate_briefs: bool,
    /// Stop ingest after raw-turn embedding for provider/transport diagnostics.
    #[arg(long)]
    stop_after_raw_embed: bool,
    /// Stage-isolated ingest diagnostic: raw-embed, distill, or raw-embed-distill.
    #[arg(long, value_parser = ["raw-embed", "distill", "raw-embed-distill"])]
    ingest_diagnostic: Option<String>,
    #[arg(long)]
    resume: bool,
    #[arg(long)]
    fresh: bool,
    #[arg(long, default_value = "flash")]
    query_planner: Option<String>,
    /// Enable Symbiotic Memory's generic evidence-ledger answer support stage for this named trial.
    #[arg(long)]
    evidence_ledger: bool,
    /// Enable Symbiotic Memory's generic answer verifier pass for this named trial.
    #[arg(long)]
    answer_verifier: bool,
    /// Enable Symbiotic Memory's generic answer gap retry pass for this named trial.
    #[arg(long)]
    answer_gap_retry: bool,
    /// Enable Symbiotic Memory's answer gap retry only when the primary answer abstains.
    #[arg(long)]
    answer_unavailable_retry: bool,
    #[arg(long)]
    score: bool,
    #[arg(long)]
    no_score: bool,
    #[arg(long)]
    oracle: Option<PathBuf>,
    #[arg(long, default_value_t = 400)]
    judge_workers: usize,
    /// Score this many hypotheses first to warm DeepSeek's shared-prefix cache before the real score.
    #[arg(long, default_value_t = 0)]
    prewarm_judge_cache: usize,
    /// Seconds to pause after judge cache prewarm before the real score.
    #[arg(long, default_value_t = 10)]
    prewarm_pause_secs: u64,
    #[arg(long, default_value = "queued-longmemeval-deepseek-v4-flash")]
    scorer: String,
    /// Override the default repo-local `.env.test.local` file.
    #[arg(long)]
    env_file: Option<PathBuf>,
    #[arg(long)]
    provider_queue_dir: Option<PathBuf>,
    /// For answer-only reruns, link immutable vault data from this vault root instead of copying a full run.
    #[arg(long)]
    source_vault_root: Option<PathBuf>,
    /// Keep local smoke-test artifacts instead of deleting them after success.
    #[arg(long, hide = true)]
    keep_smoke_run: bool,
    #[arg(long)]
    hypotheses: Option<PathBuf>,
    #[arg(long)]
    provenance: Option<PathBuf>,
    #[arg(long)]
    verdicts: Option<PathBuf>,
    #[arg(long)]
    partial_verdicts: Option<PathBuf>,
    #[arg(long)]
    memory_traces: Option<PathBuf>,
    #[arg(long)]
    model_traces: Option<PathBuf>,
    #[arg(long)]
    scored: Option<PathBuf>,
    #[arg(long)]
    import_report: bool,
    #[arg(long)]
    run_name: Option<String>,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
#[allow(clippy::large_enum_variant)]
enum Command {
    Explore {
        #[arg(long)]
        run_root: Option<PathBuf>,
        #[arg(long, default_value = "runs")]
        registry_root: PathBuf,
        /// Emit the normalized run index as JSON (machine-readable). Without a
        /// `--run-root`, scans both `runs/` and `records/`.
        #[arg(long)]
        json: bool,
    },
    /// Manage the canonical vault store ($SYMEM_VAULT_STORE): back up per-run
    /// vaults outside the disposable `runs/` tree and rediscover them by name.
    Vault {
        #[command(subcommand)]
        command: VaultCommand,
    },
    SaveRecord {
        #[arg(long)]
        run_root: PathBuf,
        #[arg(long, default_value = "records")]
        records_root: PathBuf,
        #[arg(long)]
        record_name: Option<String>,
        #[arg(long)]
        force: bool,
    },
    SummarizeQueueEvents {
        #[arg(long)]
        jsonl: PathBuf,
    },
    SummarizeModelTraces {
        #[arg(long)]
        jsonl: PathBuf,
    },
    Analytics {
        #[arg(long)]
        run_root: PathBuf,
    },
    ProviderEmbedProbe {
        #[arg(long)]
        dataset: Option<PathBuf>,
        #[arg(long, default_value_t = 10)]
        limit: usize,
        #[arg(long, default_value = "stratified")]
        sample: String,
        #[arg(long)]
        run_root: Option<PathBuf>,
        #[arg(long, default_value = "qwen/qwen3-embedding-8b")]
        model: String,
        #[arg(long, default_value_t = 1024)]
        dimensions: usize,
        #[arg(long, default_value_t = 250)]
        batch_size: usize,
        #[arg(long, default_value_t = 32_000)]
        batch_max_chars: usize,
        /// Batch packing scope: source matches ingest; global is useful for theoretical transport probes.
        #[arg(long, default_value = "source")]
        pack_scope: String,
        #[arg(long)]
        concurrency: Option<usize>,
        #[arg(long, default_value_t = 4)]
        client_pool_size: usize,
        /// Transport mode: default, h1, h2, h1-fresh, or h2-fresh.
        #[arg(long, default_value = "h2")]
        http_mode: String,
        #[arg(long, default_value_t = 120)]
        timeout_secs: u64,
        #[arg(long, default_value_t = 15)]
        connect_timeout_secs: u64,
        #[arg(long, default_value_t = 64)]
        pool_max_idle_per_host: usize,
        #[arg(long)]
        env_file: Option<PathBuf>,
        #[arg(long, default_value = "https://openrouter.ai/api/v1")]
        base_url: String,
        #[arg(long)]
        print_order: bool,
    },
    Trials {
        #[command(subcommand)]
        command: TrialsCommand,
    },
    /// Join a run's kept facts + verdicts with the dataset's gold annotation and
    /// classify each question: correct / reader_fail (gold present, reader missed)
    /// / retrieval_gap (a gold piece missing). Writes `artifacts/gold-eval.json`.
    GoldEval {
        /// Run dir name under `runs/.../<limit>/<name>`, or an explicit path.
        #[arg(long)]
        run: String,
    },
}

#[derive(Subcommand)]
enum VaultCommand {
    /// Copy (or move) a run's `vaults/` into the canonical store keyed by name.
    Save {
        /// Run to back up: a run dir name under
        /// `runs/symbiotic-memory/long-mem-eval/<limit>/<name>`, or an explicit path.
        #[arg(long)]
        run: String,
        /// Store key (subdir under $SYMEM_VAULT_STORE). Defaults to the run dir name.
        #[arg(long = "as")]
        key: Option<String>,
        /// Move instead of copy, then leave a symlink at the run's `vaults/`
        /// pointing back into the store (keeps `--source-vault-root` working).
        #[arg(long)]
        r#move: bool,
    },
    /// List stored vaults: key, saved_at, vault_count, size, accuracy.
    List,
    /// Print `$SYMEM_VAULT_STORE/<key>/vaults` (for `--source-vault-root`).
    Path { key: String },
}

#[derive(Subcommand)]
enum TrialsCommand {
    /// Derive typed trial stack/delta artifacts from existing benchmark run artifacts.
    Derive {
        /// Stable trial-stack id. Generated from the change title and run roots when omitted.
        #[arg(long)]
        stack_id: Option<String>,
        #[arg(long)]
        trial_run_root: PathBuf,
        #[arg(long)]
        comparison_run_root: PathBuf,
        #[arg(long)]
        original_baseline_run_root: Option<PathBuf>,
        #[arg(long)]
        output_dir: Option<PathBuf>,
        /// Stable change id. Generated from the change title and run roots when omitted.
        #[arg(long)]
        change_id: Option<String>,
        #[arg(long)]
        change_title: String,
        #[arg(long)]
        reasoning: String,
        /// Repeatable: `path[:line]|area|summary`.
        #[arg(long)]
        changed_file: Vec<String>,
        /// Repeatable command or check used to validate this change.
        #[arg(long)]
        verification: Vec<String>,
        /// Repeatable known risk or possible overgeneralization.
        #[arg(long)]
        risk: Vec<String>,
        #[arg(long, default_value = "diagnostic_only")]
        decision: String,
        /// Replace existing rows for this trial run id.
        #[arg(long)]
        force: bool,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    if let Some(command) = cli.command {
        return match command {
            Command::Explore {
                run_root,
                registry_root,
                json,
            } => {
                let run_root = run_root.map(|path| resolve_repo_path(&path));
                if json {
                    explore_runs_json(run_root)
                } else {
                    explore_runs(run_root, resolve_repo_path(&registry_root))
                }
            }
            Command::Vault { command } => match command {
                VaultCommand::Save { run, key, r#move } => vault_save(&run, key.as_deref(), r#move),
                VaultCommand::List => vault_list(),
                VaultCommand::Path { key } => vault_path(&key),
            },
            Command::SaveRecord {
                run_root,
                records_root,
                record_name,
                force,
            } => save_record(
                resolve_repo_path(&run_root),
                resolve_repo_path(&records_root),
                record_name,
                force,
            ),
            Command::SummarizeQueueEvents { jsonl } => summarize_queue_events(jsonl),
            Command::SummarizeModelTraces { jsonl } => summarize_model_traces(jsonl),
            Command::Analytics { run_root } => {
                write_analytics_for_run(resolve_repo_path(&run_root))
            }
            Command::ProviderEmbedProbe {
                dataset,
                limit,
                sample,
                run_root,
                model,
                dimensions,
                batch_size,
                batch_max_chars,
                pack_scope,
                concurrency,
                client_pool_size,
                http_mode,
                timeout_secs,
                connect_timeout_secs,
                pool_max_idle_per_host,
                env_file,
                base_url,
                print_order,
            } => provider_embed_probe(ProviderEmbedProbeCli {
                dataset,
                limit,
                sample,
                run_root,
                model,
                dimensions,
                batch_size,
                batch_max_chars,
                pack_scope,
                concurrency,
                client_pool_size,
                http_mode,
                timeout_secs,
                connect_timeout_secs,
                pool_max_idle_per_host,
                env_file,
                base_url,
                print_order,
            }),
            Command::Trials { command } => match command {
                TrialsCommand::Derive {
                    stack_id,
                    trial_run_root,
                    comparison_run_root,
                    original_baseline_run_root,
                    output_dir,
                    change_id,
                    change_title,
                    reasoning,
                    changed_file,
                    verification,
                    risk,
                    decision,
                    force,
                } => derive_trials(TrialDeriveCli {
                    stack_id,
                    trial_run_root,
                    comparison_run_root,
                    original_baseline_run_root,
                    output_dir,
                    change_id,
                    change_title,
                    reasoning,
                    changed_file,
                    verification,
                    risk,
                    decision,
                    force,
                }),
            },
            Command::GoldEval { run } => gold_eval(&run),
        };
    }

    run_selected_benchmark(cli)
}

#[cfg_attr(not(feature = "symbiotic-memory-adapter"), allow(dead_code))]
struct ProviderEmbedProbeCli {
    dataset: Option<PathBuf>,
    limit: usize,
    sample: String,
    run_root: Option<PathBuf>,
    model: String,
    dimensions: usize,
    batch_size: usize,
    batch_max_chars: usize,
    pack_scope: String,
    concurrency: Option<usize>,
    client_pool_size: usize,
    http_mode: String,
    timeout_secs: u64,
    connect_timeout_secs: u64,
    pool_max_idle_per_host: usize,
    env_file: Option<PathBuf>,
    base_url: String,
    print_order: bool,
}

#[cfg(not(feature = "symbiotic-memory-adapter"))]
fn provider_embed_probe(_cli: ProviderEmbedProbeCli) -> anyhow::Result<()> {
    anyhow::bail!("provider-embed-probe requires --features symbiotic-memory-adapter")
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn provider_embed_probe(cli: ProviderEmbedProbeCli) -> anyhow::Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(provider_embed_probe_async(cli))
}

#[cfg(feature = "symbiotic-memory-adapter")]
#[derive(Clone, Copy, Debug)]
enum ProbeHttpMode {
    Default,
    H1,
    H2,
    H1Fresh,
    H2Fresh,
}

#[cfg(feature = "symbiotic-memory-adapter")]
impl ProbeHttpMode {
    fn parse(raw: &str) -> anyhow::Result<Self> {
        match raw {
            "default" => Ok(Self::Default),
            "h1" | "http1" => Ok(Self::H1),
            "h2" | "http2" => Ok(Self::H2),
            "h1-fresh" | "http1-fresh" => Ok(Self::H1Fresh),
            "h2-fresh" | "http2-fresh" => Ok(Self::H2Fresh),
            other => anyhow::bail!(
                "unknown --http-mode {other}; use default, h1, h2, h1-fresh, or h2-fresh"
            ),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::H1 => "h1",
            Self::H2 => "h2",
            Self::H1Fresh => "h1-fresh",
            Self::H2Fresh => "h2-fresh",
        }
    }

    fn fresh_client_per_request(self) -> bool {
        matches!(self, Self::H1Fresh | Self::H2Fresh)
    }
}

#[cfg(feature = "symbiotic-memory-adapter")]
#[derive(Clone)]
struct ProbeBatch {
    batch_index: usize,
    labels: Vec<String>,
    texts: Vec<String>,
    chars: usize,
}

#[cfg(feature = "symbiotic-memory-adapter")]
#[derive(Clone)]
struct ProbeRequest {
    batch_index: usize,
    batch_items: usize,
    batch_chars: usize,
    request_bytes: usize,
    labels: Vec<String>,
    body: Vec<u8>,
}

#[cfg(feature = "symbiotic-memory-adapter")]
#[derive(Serialize)]
struct ProbeRequestSample {
    request_index: usize,
    batch_index: usize,
    batch_items: usize,
    batch_chars: usize,
    request_bytes: usize,
    response_bytes: usize,
    start_offset_ms: u128,
    headers_ms: Option<u128>,
    body_ms: Option<u128>,
    decode_ms: Option<u128>,
    total_ms: u128,
    completion_offset_ms: u128,
    http_version: Option<String>,
    status: Option<u16>,
    ok: bool,
    error_class: Option<String>,
    response_items: Option<usize>,
    first_labels: Vec<String>,
}

#[cfg(feature = "symbiotic-memory-adapter")]
#[derive(Serialize)]
struct ProbeSummary {
    run_root: String,
    dataset: String,
    limit: usize,
    sample: String,
    model: String,
    dimensions: usize,
    http_mode: String,
    client_pool_size: usize,
    concurrency: usize,
    batch_size: usize,
    batch_max_chars: usize,
    pack_scope: String,
    text_count: usize,
    request_count: usize,
    ok: usize,
    failed: usize,
    wall_ms: u128,
    response_bytes_total: usize,
    request_bytes_total: usize,
    statuses: BTreeMap<String, usize>,
    versions: BTreeMap<String, usize>,
    errors: BTreeMap<String, usize>,
    request_ms: PercentileSummary,
    headers_ms: PercentileSummary,
    body_ms: PercentileSummary,
    decode_ms: PercentileSummary,
    completion_gap_ms: PercentileSummary,
    batch_items: PercentileSummary,
    batch_chars: PercentileSummary,
    request_bytes: PercentileSummary,
    response_bytes: PercentileSummary,
}

#[cfg(feature = "symbiotic-memory-adapter")]
#[derive(Serialize)]
struct PercentileSummary {
    min: u128,
    p50: u128,
    p80: u128,
    p95: u128,
    p98: u128,
    max: u128,
}

#[cfg(feature = "symbiotic-memory-adapter")]
async fn provider_embed_probe_async(cli: ProviderEmbedProbeCli) -> anyhow::Result<()> {
    let mode = ProbeHttpMode::parse(&cli.http_mode)?;
    let dataset = resolve_longmemeval_dataset(cli.dataset.clone())?;
    let rows = symbiotic_mem_bench::symbiotic_memory_adapter::load_longmemeval(&dataset, None)?;
    let rows = select_longmemeval_rows(rows, cli.limit, &cli.sample)?;
    let groups = raw_embedding_text_groups(&rows);
    let pack_scope = parse_probe_pack_scope(&cli.pack_scope)?;
    let text_count = groups.iter().map(|(_, texts)| texts.len()).sum();
    let batches = match pack_scope {
        ProbePackScope::Source => {
            let mut batches = Vec::new();
            for (_source_id, texts) in groups {
                batches.extend(probe_embedding_batches(
                    texts,
                    cli.batch_size.clamp(1, 250),
                    cli.batch_max_chars,
                ));
            }
            for (batch_index, batch) in batches.iter_mut().enumerate() {
                batch.batch_index = batch_index;
            }
            batches
        }
        ProbePackScope::Global => {
            let texts = groups
                .into_iter()
                .flat_map(|(_, texts)| texts)
                .collect::<Vec<_>>();
            probe_embedding_batches(texts, cli.batch_size.clamp(1, 250), cli.batch_max_chars)
        }
    };
    let requests = probe_requests(&cli.model, cli.dimensions, batches)?;
    let request_count = requests.len();
    let concurrency = cli.concurrency.unwrap_or(request_count).max(1);
    let run_root = cli
        .run_root
        .as_ref()
        .map(|path| resolve_repo_path(path))
        .unwrap_or_else(|| {
            repo_root()
                .join("runs")
                .join(".tmp")
                .join("provider-embed-probe")
                .join(format!(
                    "{}-{}-{}",
                    mode.label(),
                    cli.limit,
                    Utc::now().format("%Y%m%d-%H%M%S")
                ))
        });
    std::fs::create_dir_all(&run_root)?;

    let api_key = openrouter_api_key(cli.env_file.as_ref())?;
    let base_url = cli.base_url.trim_end_matches('/').to_string();
    let clients = Arc::new(
        (0..cli.client_pool_size.max(1))
            .map(|_| {
                build_probe_client(
                    mode,
                    cli.timeout_secs,
                    cli.connect_timeout_secs,
                    cli.pool_max_idle_per_host,
                )
            })
            .collect::<anyhow::Result<Vec<_>>>()?,
    );
    let started = Instant::now();
    let samples = futures::stream::iter(requests.into_iter().enumerate())
        .map(|(request_index, request)| {
            let clients = clients.clone();
            let api_key = api_key.clone();
            let base_url = base_url.clone();
            async move {
                let fresh_client;
                let client = if mode.fresh_client_per_request() {
                    fresh_client = build_probe_client(
                        mode,
                        cli.timeout_secs,
                        cli.connect_timeout_secs,
                        cli.pool_max_idle_per_host,
                    )?;
                    &fresh_client
                } else {
                    &clients[request_index % clients.len()]
                };
                run_probe_request(client, &base_url, &api_key, request_index, request, started)
                    .await
            }
        })
        .buffer_unordered(concurrency)
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .collect::<anyhow::Result<Vec<_>>>()?;
    let wall_ms = started.elapsed().as_millis();

    let requests_jsonl = run_root.join("requests.jsonl");
    let mut out = std::fs::File::create(&requests_jsonl)?;
    let mut sorted_samples = samples;
    sorted_samples.sort_by_key(|sample| sample.request_index);
    for sample in &sorted_samples {
        writeln!(out, "{}", serde_json::to_string(sample)?)?;
    }
    let summary = probe_summary(
        &run_root,
        &dataset,
        &cli,
        mode,
        concurrency,
        wall_ms,
        text_count,
        &sorted_samples,
    );
    let summary_json = run_root.join("summary.json");
    std::fs::write(&summary_json, serde_json::to_string_pretty(&summary)?)?;

    println!("{}", serde_json::to_string_pretty(&summary)?);
    if cli.print_order {
        let mut order = sorted_samples.iter().collect::<Vec<_>>();
        order.sort_by_key(|sample| sample.completion_offset_ms);
        for sample in order {
            println!(
                "done request={} start={}ms total={}ms complete={}ms items={} chars={} status={:?} err={:?}",
                sample.request_index,
                sample.start_offset_ms,
                sample.total_ms,
                sample.completion_offset_ms,
                sample.batch_items,
                sample.batch_chars,
                sample.status,
                sample.error_class
            );
        }
    }
    println!("wrote {}", portable_path(&requests_jsonl));
    println!("wrote {}", portable_path(&summary_json));
    Ok(())
}

#[cfg(feature = "symbiotic-memory-adapter")]
#[derive(Clone, Copy)]
enum ProbePackScope {
    Source,
    Global,
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn parse_probe_pack_scope(raw: &str) -> anyhow::Result<ProbePackScope> {
    match raw {
        "source" => Ok(ProbePackScope::Source),
        "global" => Ok(ProbePackScope::Global),
        other => anyhow::bail!("unknown --pack-scope {other}; use source or global"),
    }
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn raw_embedding_text_groups(
    rows: &[symbiotic_mem_bench::symbiotic_memory_adapter::LongMemEvalRecord],
) -> Vec<(String, Vec<(String, String)>)> {
    // Effective shape now comes from the kit's config crate defaults (the
    // SYMEM_* env layer is gone); when the bench grows config-file plumbing
    // these become the resolved config snapshot.
    let distill = symbiotic_memory_config::DistillSection::default();
    let max_input_tokens = symbiotic_memory_config::EmbedSection::default().max_input_tokens;
    let raw_window = symbiotic_memory::ingest::RawWindowConfig::from_values(
        distill.raw_window_size,
        distill.raw_window_stride,
    );
    rows.iter()
        .map(|row| {
            let source = symbiotic_mem_bench::symbiotic_memory_adapter::longmemeval_to_source(row);
            let source_id = source.source_id.clone();
            let texts = symbiotic_memory::ingest::source_turns_with_derived_units(
                &source,
                raw_window,
                distill.raw_unit_max_input_tokens,
            )
                .into_iter()
                .flat_map(move |turn| {
                    let formatted = symbiotic_memory::ingest::format_turn_for_embedding(&turn);
                    if symbiotic_memory::ingest::approx_tokens(&formatted) > max_input_tokens {
                        symbiotic_memory::ingest::split_turn_by_text_budget(&turn, max_input_tokens)
                    } else {
                        vec![turn]
                    }
                })
                .map(|turn| {
                    let label = format!("turn={}", turn.turn_id);
                    let text = symbiotic_memory::ingest::format_turn_for_embedding(&turn);
                    (label, text)
                })
                .collect::<Vec<_>>();
            (source_id, texts)
        })
        .collect()
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn probe_embedding_batches(
    mut inputs: Vec<(String, String)>,
    max_items: usize,
    max_chars: usize,
) -> Vec<ProbeBatch> {
    let max_items = max_items.max(1);
    let max_chars = max_chars.max(1);
    inputs.sort_by(|left, right| {
        right
            .1
            .len()
            .cmp(&left.1.len())
            .then_with(|| left.0.cmp(&right.0))
    });

    let mut bins: Vec<(usize, Vec<(String, String)>)> = Vec::new();
    for item in inputs {
        let item_chars = item.1.len();
        let mut item = Some(item);
        for (batch_chars, batch) in &mut bins {
            if batch.len() < max_items && batch_chars.saturating_add(item_chars) <= max_chars {
                *batch_chars = batch_chars.saturating_add(item_chars);
                batch.push(item.take().expect("probe item already packed"));
                break;
            }
        }
        if let Some(item) = item {
            bins.push((item_chars, vec![item]));
        }
    }

    bins.sort_by(|(left_chars, left_batch), (right_chars, right_batch)| {
        left_chars
            .cmp(right_chars)
            .then_with(|| left_batch.len().cmp(&right_batch.len()))
    });
    bins.into_iter()
        .enumerate()
        .map(|(batch_index, (chars, items))| {
            let (labels, texts): (Vec<_>, Vec<_>) = items.into_iter().unzip();
            ProbeBatch {
                batch_index,
                labels,
                texts,
                chars,
            }
        })
        .collect()
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn probe_requests(
    model: &str,
    dimensions: usize,
    batches: Vec<ProbeBatch>,
) -> anyhow::Result<Vec<ProbeRequest>> {
    batches
        .into_iter()
        .map(|batch| {
            let body = serde_json::to_vec(&json!({
                "model": model,
                "input": batch.texts,
                "dimensions": dimensions,
            }))?;
            Ok(ProbeRequest {
                batch_index: batch.batch_index,
                batch_items: batch.labels.len(),
                batch_chars: batch.chars,
                request_bytes: body.len(),
                labels: batch.labels,
                body,
            })
        })
        .collect()
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn openrouter_api_key(env_file: Option<&PathBuf>) -> anyhow::Result<String> {
    if let Some(value) = std::env::var("OPENROUTER_API_KEY")
        .ok()
        .filter(|value| !value.is_empty())
    {
        return Ok(value);
    }
    let env_file = env_file
        .map(|path| resolve_repo_path(path))
        .or_else(|| {
            let path = repo_root().join(".env.test.local");
            path.exists().then_some(path)
        })
        .context("OPENROUTER_API_KEY missing and no .env.test.local found")?;
    let env = load_env_file(&env_file)?;
    env.get("OPENROUTER_API_KEY")
        .filter(|value| !value.is_empty())
        .cloned()
        .with_context(|| format!("OPENROUTER_API_KEY missing in {}", env_file.display()))
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn build_probe_client(
    mode: ProbeHttpMode,
    timeout_secs: u64,
    connect_timeout_secs: u64,
    pool_max_idle_per_host: usize,
) -> anyhow::Result<Client> {
    let mut builder = Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .connect_timeout(Duration::from_secs(connect_timeout_secs))
        .pool_idle_timeout(Duration::from_secs(90))
        .pool_max_idle_per_host(pool_max_idle_per_host)
        .tcp_keepalive(Duration::from_secs(60));
    match mode {
        ProbeHttpMode::H1 | ProbeHttpMode::H1Fresh => {
            builder = builder.http1_only();
        }
        ProbeHttpMode::H2 | ProbeHttpMode::H2Fresh => {}
        ProbeHttpMode::Default => {}
    }
    Ok(builder.build()?)
}

#[cfg(feature = "symbiotic-memory-adapter")]
async fn run_probe_request(
    client: &Client,
    base_url: &str,
    api_key: &str,
    request_index: usize,
    request: ProbeRequest,
    run_started: Instant,
) -> anyhow::Result<ProbeRequestSample> {
    let started = Instant::now();
    let start_offset_ms = run_started.elapsed().as_millis();
    let response = client
        .post(format!("{base_url}/embeddings"))
        .bearer_auth(api_key)
        .header("content-type", "application/json")
        .body(request.body)
        .send()
        .await;
    let response = match response {
        Ok(response) => response,
        Err(err) => {
            let total_ms = started.elapsed().as_millis();
            return Ok(ProbeRequestSample {
                request_index,
                batch_index: request.batch_index,
                batch_items: request.batch_items,
                batch_chars: request.batch_chars,
                request_bytes: request.request_bytes,
                response_bytes: 0,
                start_offset_ms,
                headers_ms: None,
                body_ms: None,
                decode_ms: None,
                total_ms,
                completion_offset_ms: start_offset_ms + total_ms,
                http_version: None,
                status: None,
                ok: false,
                error_class: Some(classify_reqwest_error(&err)),
                response_items: None,
                first_labels: request.labels.into_iter().take(3).collect(),
            });
        }
    };
    let headers_ms = started.elapsed().as_millis();
    let version = response.version();
    let status = response.status().as_u16();
    let bytes = match response.bytes().await {
        Ok(bytes) => bytes,
        Err(err) => {
            let total_ms = started.elapsed().as_millis();
            return Ok(ProbeRequestSample {
                request_index,
                batch_index: request.batch_index,
                batch_items: request.batch_items,
                batch_chars: request.batch_chars,
                request_bytes: request.request_bytes,
                response_bytes: 0,
                start_offset_ms,
                headers_ms: Some(headers_ms),
                body_ms: None,
                decode_ms: None,
                total_ms,
                completion_offset_ms: start_offset_ms + total_ms,
                http_version: Some(format!("{version:?}")),
                status: Some(status),
                ok: false,
                error_class: Some(classify_reqwest_error(&err)),
                response_items: None,
                first_labels: request.labels.into_iter().take(3).collect(),
            });
        }
    };
    let body_ms = started.elapsed().as_millis();
    let response_bytes = bytes.len();
    let decoded = serde_json::from_slice::<Value>(&bytes);
    let decode_ms = started.elapsed().as_millis();
    let response_items = decoded
        .as_ref()
        .ok()
        .and_then(|value| value.get("data"))
        .and_then(Value::as_array)
        .map(Vec::len);
    let ok = (200..300).contains(&status) && decoded.is_ok();
    Ok(ProbeRequestSample {
        request_index,
        batch_index: request.batch_index,
        batch_items: request.batch_items,
        batch_chars: request.batch_chars,
        request_bytes: request.request_bytes,
        response_bytes,
        start_offset_ms,
        headers_ms: Some(headers_ms),
        body_ms: Some(body_ms),
        decode_ms: Some(decode_ms),
        total_ms: decode_ms,
        completion_offset_ms: start_offset_ms + decode_ms,
        http_version: Some(format!("{version:?}")),
        status: Some(status),
        ok,
        error_class: if !(200..300).contains(&status) {
            Some("http_status".to_string())
        } else if decoded.is_err() {
            Some("json_decode".to_string())
        } else {
            None
        },
        response_items,
        first_labels: request.labels.into_iter().take(3).collect(),
    })
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn classify_reqwest_error(err: &reqwest::Error) -> String {
    if err.is_timeout() {
        "timeout".to_string()
    } else if err.is_connect() {
        "connect".to_string()
    } else if err.is_decode() {
        "decode".to_string()
    } else if err.is_body() {
        "body".to_string()
    } else if err.is_request() {
        "request".to_string()
    } else {
        "other".to_string()
    }
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn probe_summary(
    run_root: &Path,
    dataset: &Path,
    cli: &ProviderEmbedProbeCli,
    mode: ProbeHttpMode,
    concurrency: usize,
    wall_ms: u128,
    text_count: usize,
    samples: &[ProbeRequestSample],
) -> ProbeSummary {
    let mut statuses = BTreeMap::<String, usize>::new();
    let mut versions = BTreeMap::<String, usize>::new();
    let mut errors = BTreeMap::<String, usize>::new();
    let mut completion_offsets = samples
        .iter()
        .map(|sample| sample.completion_offset_ms)
        .collect::<Vec<_>>();
    completion_offsets.sort_unstable();
    let completion_gaps = completion_offsets
        .windows(2)
        .map(|window| window[1].saturating_sub(window[0]))
        .collect::<Vec<_>>();
    for sample in samples {
        *statuses
            .entry(
                sample
                    .status
                    .map_or_else(|| "none".to_string(), |s| s.to_string()),
            )
            .or_default() += 1;
        *versions
            .entry(
                sample
                    .http_version
                    .clone()
                    .unwrap_or_else(|| "none".to_string()),
            )
            .or_default() += 1;
        if let Some(error) = &sample.error_class {
            *errors.entry(error.clone()).or_default() += 1;
        }
    }
    ProbeSummary {
        run_root: portable_path(run_root),
        dataset: portable_path(dataset),
        limit: cli.limit,
        sample: cli.sample.clone(),
        model: cli.model.clone(),
        dimensions: cli.dimensions,
        http_mode: mode.label().to_string(),
        client_pool_size: cli.client_pool_size.max(1),
        concurrency,
        batch_size: cli.batch_size.clamp(1, 250),
        batch_max_chars: cli.batch_max_chars,
        pack_scope: cli.pack_scope.clone(),
        text_count,
        request_count: samples.len(),
        ok: samples.iter().filter(|sample| sample.ok).count(),
        failed: samples.iter().filter(|sample| !sample.ok).count(),
        wall_ms,
        response_bytes_total: samples.iter().map(|sample| sample.response_bytes).sum(),
        request_bytes_total: samples.iter().map(|sample| sample.request_bytes).sum(),
        statuses,
        versions,
        errors,
        request_ms: summarize_u128(
            samples
                .iter()
                .filter(|sample| sample.ok)
                .map(|s| s.total_ms),
        ),
        headers_ms: summarize_u128(samples.iter().filter_map(|sample| sample.headers_ms)),
        body_ms: summarize_u128(samples.iter().filter_map(|sample| sample.body_ms)),
        decode_ms: summarize_u128(samples.iter().filter_map(|sample| sample.decode_ms)),
        completion_gap_ms: summarize_u128(completion_gaps),
        batch_items: summarize_u128(samples.iter().map(|sample| sample.batch_items as u128)),
        batch_chars: summarize_u128(samples.iter().map(|sample| sample.batch_chars as u128)),
        request_bytes: summarize_u128(samples.iter().map(|sample| sample.request_bytes as u128)),
        response_bytes: summarize_u128(samples.iter().map(|sample| sample.response_bytes as u128)),
    }
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn summarize_u128(values: impl IntoIterator<Item = u128>) -> PercentileSummary {
    let mut values = values.into_iter().collect::<Vec<_>>();
    values.sort_unstable();
    PercentileSummary {
        min: values.first().copied().unwrap_or(0),
        p50: percentile_u128(&values, 0.50),
        p80: percentile_u128(&values, 0.80),
        p95: percentile_u128(&values, 0.95),
        p98: percentile_u128(&values, 0.98),
        max: values.last().copied().unwrap_or(0),
    }
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn percentile_u128(sorted: &[u128], p: f64) -> u128 {
    if sorted.is_empty() {
        return 0;
    }
    let idx = ((sorted.len() - 1) as f64 * p).ceil() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

struct TrialDeriveCli {
    stack_id: Option<String>,
    trial_run_root: PathBuf,
    comparison_run_root: PathBuf,
    original_baseline_run_root: Option<PathBuf>,
    output_dir: Option<PathBuf>,
    change_id: Option<String>,
    change_title: String,
    reasoning: String,
    changed_file: Vec<String>,
    verification: Vec<String>,
    risk: Vec<String>,
    decision: String,
    force: bool,
}

fn derive_trials(cli: TrialDeriveCli) -> anyhow::Result<()> {
    let trial_run_root = resolve_repo_path(&cli.trial_run_root);
    let comparison_run_root = resolve_repo_path(&cli.comparison_run_root);
    let original_baseline_run_root = cli
        .original_baseline_run_root
        .as_ref()
        .map(|path| resolve_repo_path(path));
    let change_id = cli.change_id.unwrap_or_else(|| {
        generated_trial_id(
            &cli.change_title,
            &trial_run_root,
            &comparison_run_root,
            original_baseline_run_root.as_deref(),
        )
    });
    let stack_id = cli.stack_id.unwrap_or_else(|| format!("trial-{change_id}"));
    let output_dir = cli
        .output_dir
        .map(|path| resolve_repo_path(&path))
        .unwrap_or_else(|| {
            repo_root()
                .join("runs")
                .join("analysis")
                .join(stack_id.clone())
        });
    trials::derive_trial(trials::TrialDeriveOptions {
        stack_id,
        output_dir: output_dir.clone(),
        trial_run_root,
        comparison_run_root,
        original_baseline_run_root,
        change_id,
        change_title: cli.change_title,
        reasoning: cli.reasoning,
        changed_files: cli
            .changed_file
            .iter()
            .map(|raw| parse_changed_file(raw))
            .collect::<anyhow::Result<Vec<_>>>()?,
        verification: cli.verification,
        risks: cli.risk,
        decision: cli.decision,
        force: cli.force,
    })?;
    println!("wrote trial artifacts to {}", portable_path(&output_dir));
    Ok(())
}

fn generated_trial_id(
    title: &str,
    trial_run_root: &Path,
    comparison_run_root: &Path,
    original_baseline_run_root: Option<&Path>,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(title.as_bytes());
    hasher.update(b"\n");
    hasher.update(portable_display_path(trial_run_root).as_bytes());
    hasher.update(b"\n");
    hasher.update(portable_display_path(comparison_run_root).as_bytes());
    if let Some(root) = original_baseline_run_root {
        hasher.update(b"\n");
        hasher.update(portable_display_path(root).as_bytes());
    }
    let hash = short_hex(&hasher.finalize());
    let slug = slugify_id(title);
    format!("{slug}-{}", &hash[..10])
}

fn short_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

fn slugify_id(value: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in value.chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            last_dash = false;
        } else if !last_dash && !out.is_empty() {
            out.push('-');
            last_dash = true;
        }
        if out.len() >= 56 {
            break;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        "trial".to_string()
    } else {
        out
    }
}

fn portable_display_path(path: &Path) -> String {
    path.strip_prefix(repo_root())
        .unwrap_or(path)
        .to_string_lossy()
        .to_string()
}

fn parse_changed_file(raw: &str) -> anyhow::Result<trials::ChangedFileInput> {
    let mut parts = raw.splitn(3, '|');
    let path_and_line = parts.next().unwrap_or_default();
    if path_and_line.trim().is_empty() {
        anyhow::bail!("--changed-file requires a path");
    }
    let area = parts
        .next()
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned);
    let summary = parts
        .next()
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned);
    let (path, line) = parse_path_line(path_and_line)?;
    Ok(trials::ChangedFileInput {
        path,
        line,
        area,
        summary,
    })
}

fn parse_path_line(raw: &str) -> anyhow::Result<(String, Option<u64>)> {
    let Some((path, line)) = raw.rsplit_once(':') else {
        return Ok((raw.to_string(), None));
    };
    if path.is_empty() {
        return Ok((raw.to_string(), None));
    }
    match line.parse::<u64>() {
        Ok(line) => Ok((path.to_string(), Some(line))),
        Err(_) => Ok((raw.to_string(), None)),
    }
}

fn run_selected_benchmark(cli: Cli) -> anyhow::Result<()> {
    let system = selected_value(
        cli.system.as_deref(),
        cli.symbiotic_memory,
        "symbiotic-memory",
        "--system or --symbiotic-memory",
    )?;
    let benchmark = selected_value(
        cli.benchmark.as_deref(),
        cli.long_mem_eval,
        "long-mem-eval",
        "--benchmark or --long-mem-eval",
    )?;

    // Gold-oracle mode is consumed per-question inside the in-process adapter via SYMEM_ORACLE_GOLD;
    // the `--oracle-gold` flag just exports it here. Safe to set_var now: this runs single-threaded,
    // before any tokio runtime/worker threads spawn (the runtime is built inside the run fn below).
    if cli.oracle_gold {
        unsafe {
            std::env::set_var("SYMEM_ORACLE_GOLD", "1");
        }
    }

    match (system.as_str(), benchmark.as_str()) {
        ("symbiotic-memory", "long-mem-eval") => {
            if cli.import_report {
                let hypotheses = cli
                    .hypotheses
                    .ok_or_else(|| anyhow::anyhow!("--hypotheses is required"))?;
                let scored = cli
                    .scored
                    .ok_or_else(|| anyhow::anyhow!("--scored is required for --import-report"))?;
                let run_name = cli.run_name.unwrap_or_else(|| infer_run_name(&hypotheses));
                let registry_root = resolve_repo_path(&cli.registry_root);
                let run_root = cli
                    .run_root
                    .map(|path| resolve_repo_path(&path))
                    .unwrap_or_else(|| {
                        default_import_run_root(
                            &registry_root,
                            &system,
                            &benchmark,
                            &scored,
                            &run_name,
                        )
                        .unwrap_or_else(|_| {
                            default_run_root(&registry_root, &system, &benchmark, &run_name)
                        })
                    });
                return import_benchmark_report(ImportedBenchmarkReport {
                    system,
                    benchmark,
                    run_root,
                    run_name,
                    hypotheses,
                    provenance: cli.provenance,
                    verdicts: cli.verdicts,
                    partial_verdicts: cli.partial_verdicts,
                    memory_traces: cli.memory_traces,
                    model_traces: cli.model_traces,
                    scored,
                });
            }
            let registry_root = resolve_repo_path(&cli.registry_root);
            let explicit_run_root = cli.run_root.is_some();
            let distiller = if cli.smoke {
                "heuristic".to_string()
            } else {
                cli.distiller.clone()
            };
            let embedder = if cli.smoke {
                "hash".to_string()
            } else {
                cli.embedder.clone()
            };
            let score = if cli.smoke {
                if cli.score {
                    anyhow::bail!("choose either --smoke or --score, not both");
                }
                false
            } else {
                enabled_by_default("score", cli.score, cli.no_score)?
            };
            let answerer = if cli.smoke {
                false
            } else {
                enabled_by_default("answerer", cli.answerer, cli.no_answerer)?
            };
            let routed = false;
            let consolidate_briefs = if cli.smoke {
                false
            } else {
                enabled_by_default(
                    "consolidate-briefs",
                    cli.consolidate_briefs,
                    cli.no_consolidate_briefs,
                )?
            };
            let query_planner = if cli.smoke {
                Some("off".to_string())
            } else {
                cli.query_planner.clone()
            };
            let ephemeral_smoke_run = is_ephemeral_native_smoke_run(
                &cli,
                explicit_run_root,
                &distiller,
                &embedder,
                score,
            );
            let run_name = cli.run_name.unwrap_or_else(default_native_run_name);
            if cli.prewarm_judge_cache > 0 && !score {
                anyhow::bail!("--prewarm-judge-cache requires --score");
            }
            let run_root = cli
                .run_root
                .map(|path| resolve_repo_path(&path))
                .unwrap_or_else(|| {
                    if ephemeral_smoke_run {
                        default_smoke_run_root(
                            &registry_root,
                            &system,
                            &benchmark,
                            cli.limit,
                            &run_name,
                        )
                    } else {
                        default_native_run_root(
                            &registry_root,
                            &system,
                            &benchmark,
                            cli.limit,
                            &run_name,
                        )
                    }
                });
            let fresh = effective_fresh(cli.resume, cli.answer_only, cli.rejudge, cli.fresh)?;
            let dataset = resolve_longmemeval_dataset(cli.dataset)?;
            run_symbiotic_memory_longmemeval(SymbioticMemoryCliRun {
                dataset,
                run_root,
                run_name,
                limit: cli.limit,
                sample: cli.sample,
                memory_manifest: cli.memory_manifest,
                memory_config: cli.memory_config,
                symem_bin: cli.symem_bin,
                distiller,
                embedder,
                store: cli.store,
                prompt_dir: cli.prompt_dir,
                distill_prompt: cli.distill_prompt,
                answerer,
                routed,
                answer_only: cli.answer_only,
                rejudge: cli.rejudge,
                re_embed: cli.re_embed,
                consolidate_briefs,
                stop_after_raw_embed: cli.stop_after_raw_embed,
                ingest_diagnostic: cli.ingest_diagnostic,
                resume: cli.resume,
                fresh,
                query_planner,
                evidence_ledger: cli.evidence_ledger,
                answer_verifier: cli.answer_verifier,
                answer_gap_retry: cli.answer_gap_retry,
                answer_unavailable_retry: cli.answer_unavailable_retry,
                score,
                oracle: cli.oracle,
                judge_workers: cli.judge_workers,
                prewarm_judge_cache: cli.prewarm_judge_cache,
                prewarm_pause_secs: cli.prewarm_pause_secs,
                scorer: cli.scorer,
                env_file: cli.env_file,
                provider_queue_dir: cli.provider_queue_dir,
                source_vault_root: cli.source_vault_root,
                ephemeral_smoke_run,
            })
        }
        _ => {
            anyhow::bail!("unsupported benchmark selection: system={system}, benchmark={benchmark}")
        }
    }
}

fn is_ephemeral_native_smoke_run(
    cli: &Cli,
    explicit_run_root: bool,
    distiller: &str,
    embedder: &str,
    score: bool,
) -> bool {
    !explicit_run_root
        && !cli.keep_smoke_run
        && !cli.import_report
        && !score
        && distiller == "heuristic"
        && embedder == "hash"
}

fn enabled_by_default(label: &str, positive: bool, negative: bool) -> anyhow::Result<bool> {
    if positive && negative {
        anyhow::bail!("choose either --{label} or --no-{label}, not both");
    }
    Ok(!negative)
}

fn selected_value(
    explicit: Option<&str>,
    flag: bool,
    flag_value: &str,
    label: &str,
) -> anyhow::Result<String> {
    match (explicit, flag) {
        (Some(value), true) if value != flag_value => {
            anyhow::bail!("{label} specified conflicting values: {value} and {flag_value}")
        }
        (Some(value), _) => Ok(value.to_string()),
        (None, true) => Ok(flag_value.to_string()),
        (None, false) => anyhow::bail!("{label} is required"),
    }
}

fn effective_fresh(
    resume: bool,
    answer_only: bool,
    rejudge: bool,
    explicit_fresh: bool,
) -> anyhow::Result<bool> {
    if resume && explicit_fresh {
        anyhow::bail!("choose either --resume or --fresh, not both");
    }
    if (answer_only || rejudge) && explicit_fresh {
        anyhow::bail!(
            "--answer-only/--rejudge reuse an existing run root and cannot be combined with --fresh"
        );
    }
    Ok(!resume && !answer_only && !rejudge)
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn resolve_repo_path(path: &std::path::Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        repo_root().join(path)
    }
}

fn default_longmemeval_dataset_path() -> PathBuf {
    repo_root()
        .join("runs")
        .join("inputs")
        .join("longmemeval-cleaned")
        .join("longmemeval_s_cleaned.json")
}

fn resolve_longmemeval_dataset(dataset: Option<PathBuf>) -> anyhow::Result<PathBuf> {
    if let Some(dataset) = dataset {
        let dataset = resolve_repo_path(&dataset);
        if !dataset.exists() {
            anyhow::bail!(
                "dataset does not exist: {}. Omit --dataset to auto-download the default cleaned LongMemEval S dataset.",
                dataset.display()
            );
        }
        return Ok(dataset);
    }

    let dataset = default_longmemeval_dataset_path();
    ensure_default_longmemeval_dataset(&dataset)?;
    Ok(dataset)
}

fn ensure_default_longmemeval_dataset(path: &Path) -> anyhow::Result<()> {
    if dataset_has_expected_size(path)? {
        return Ok(());
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let partial = path.with_extension("json.download");
    if partial.exists() {
        std::fs::remove_file(&partial)?;
    }

    eprintln!(
        "LongMemEval cleaned S dataset missing; downloading {} to {}",
        LONGMEMEVAL_CLEANED_S_URL,
        path.display()
    );
    let curl = if Path::new("/usr/bin/curl").exists() {
        "/usr/bin/curl"
    } else {
        "curl"
    };
    let status = std::process::Command::new(curl)
        .arg("-L")
        .arg("--fail")
        .arg("--progress-bar")
        .arg("-o")
        .arg(&partial)
        .arg(LONGMEMEVAL_CLEANED_S_URL)
        .status()?;
    if !status.success() {
        let _ = std::fs::remove_file(&partial);
        anyhow::bail!("failed to download LongMemEval cleaned S dataset: {status}");
    }
    if !dataset_has_expected_size(&partial)? {
        let size = std::fs::metadata(&partial)
            .map(|meta| meta.len())
            .unwrap_or(0);
        let _ = std::fs::remove_file(&partial);
        anyhow::bail!(
            "downloaded LongMemEval dataset has unexpected size: got {size}, expected {LONGMEMEVAL_CLEANED_S_BYTES}"
        );
    }
    std::fs::rename(partial, path)?;
    Ok(())
}

fn dataset_has_expected_size(path: &Path) -> anyhow::Result<bool> {
    match std::fs::metadata(path) {
        Ok(meta) => Ok(meta.is_file() && meta.len() == LONGMEMEVAL_CLEANED_S_BYTES),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(err) => Err(err.into()),
    }
}

struct ImportedBenchmarkReport {
    system: String,
    benchmark: String,
    run_root: PathBuf,
    run_name: String,
    hypotheses: PathBuf,
    provenance: Option<PathBuf>,
    verdicts: Option<PathBuf>,
    partial_verdicts: Option<PathBuf>,
    memory_traces: Option<PathBuf>,
    model_traces: Option<PathBuf>,
    scored: PathBuf,
}

fn import_benchmark_report(import: ImportedBenchmarkReport) -> anyhow::Result<()> {
    std::fs::create_dir_all(&import.run_root)?;
    let scored_json = read_json(&import.scored)?;
    let correct = nested_u64(&scored_json, &["counts", "total_correct"]);
    let total = nested_u64(&scored_json, &["counts", "scored"]);
    let accuracy = scored_json
        .get("overall_accuracy")
        .and_then(|value| value.as_f64())
        .or_else(|| ratio(correct, total));
    let task_averaged_accuracy = scored_json
        .get("task_averaged_accuracy")
        .and_then(|value| value.as_f64());
    let abstention_correct = nested_u64(&scored_json, &["counts", "abstention_correct"]);
    let abstention_total = nested_u64(&scored_json, &["counts", "abstention_total"]);
    let abstention_accuracy = ratio(abstention_correct, abstention_total).or_else(|| {
        scored_json
            .get("abstention_accuracy")
            .and_then(|value| value.as_f64())
    });
    let hypotheses_artifact = copy_artifact(
        &import.run_root,
        &import.hypotheses,
        "hypotheses",
        "hypotheses.jsonl",
    )?;
    let scored_artifact = copy_artifact(&import.run_root, &import.scored, "scored", "scored.json")?;
    let provenance_artifact = import
        .provenance
        .as_ref()
        .map(|path| copy_artifact(&import.run_root, path, "provenance", "provenance.jsonl"))
        .transpose()?;
    let verdicts_artifact = import
        .verdicts
        .as_ref()
        .map(|path| copy_artifact(&import.run_root, path, "verdicts", "verdicts.jsonl"))
        .transpose()?;
    let partial_verdicts_artifact = import
        .partial_verdicts
        .as_ref()
        .map(|path| {
            copy_artifact(
                &import.run_root,
                path,
                "partial_verdicts",
                "partial-verdicts.jsonl",
            )
        })
        .transpose()?;
    let memory_traces_artifact = import
        .memory_traces
        .as_ref()
        .map(|path| {
            copy_artifact(
                &import.run_root,
                path,
                "memory_traces",
                "memory-traces.jsonl",
            )
        })
        .transpose()?;
    let model_traces_artifact = import
        .model_traces
        .as_ref()
        .map(|path| copy_artifact(&import.run_root, path, "model_traces", "model-traces.jsonl"))
        .transpose()?;
    let imported_manifest = imported_artifact_manifest(&import);
    let run_params = imported_run_params(&import, total);
    write_run_params(&import.run_root, &run_params)?;

    let mut report = json!({
        "schema": "membench.report.v1",
        "system": import.system,
        "benchmark": import.benchmark,
        "run_kind": "imported-artifact",
        "run_name": import.run_name,
        "run_params": run_params,
        "metrics": {
            "accuracy": {
                "correct": correct,
                "total": total,
                "value": accuracy,
            },
            "task_averaged_accuracy": task_averaged_accuracy,
            "abstention_accuracy": {
                "correct": abstention_correct,
                "total": abstention_total,
                "value": abstention_accuracy,
            }
        },
        "artifact_manifest": imported_manifest,
        "artifacts": {
            "hypotheses": hypotheses_artifact,
            "scored": scored_artifact,
        }
    });
    if let Some(provenance_artifact) = provenance_artifact {
        report["artifacts"]["provenance"] = provenance_artifact;
    }
    if let Some(verdicts_artifact) = verdicts_artifact {
        report["artifacts"]["verdicts"] = verdicts_artifact;
    }
    if let Some(partial_verdicts_artifact) = partial_verdicts_artifact {
        report["artifacts"]["partial_verdicts"] = partial_verdicts_artifact;
    }
    if let Some(memory_traces_artifact) = memory_traces_artifact {
        report["artifacts"]["memory_traces"] = memory_traces_artifact;
    }
    if let Some(model_traces_artifact) = model_traces_artifact {
        report["artifacts"]["model_traces"] = model_traces_artifact;
    }
    if let Some(step_analytics_artifact) = write_step_analytics_artifact(&import.run_root)? {
        report["artifacts"]["step_analytics"] = step_analytics_artifact;
        let available = report["artifacts"]
            .as_object()
            .into_iter()
            .flat_map(|object| object.keys().cloned())
            .collect::<Vec<_>>();
        report["artifact_manifest"] = artifact_manifest(
            available.iter().map(String::as_str),
            false,
            "Imported artifact runs preserve copied benchmark artifacts only; native state folders such as raw, vaults, workflow, and provider-queue may be absent.",
        );
    }
    enrich_report_with_cohort(&mut report, &import.run_root, &run_params);
    let out = import.run_root.join("benchmark-report.json");
    std::fs::write(&out, serde_json::to_string_pretty(&report)? + "\n")?;
    println!("{}", render_benchmark_report(&report));
    Ok(())
}

/// Add cohort identity, role models, config signature, cost/latency, and a
/// timestamp to a freshly built report. These derived fields let the dashboard
/// group comparable runs and rank by cost/speed; they degrade to nulls when the
/// underlying artifacts are absent (e.g. imported runs without traces).
fn enrich_report_with_cohort(
    report: &mut serde_json::Value,
    run_root: &Path,
    params: &serde_json::Value,
) {
    let fields = registry::compute_cohort_fields(run_root, params);
    report["created_at"] = json!(Utc::now().to_rfc3339());
    report["cohort"] = json!({
        "dataset_fingerprint": fields.dataset_fingerprint,
        "judge_model": fields.judge_model,
        "judge_prompt_mode": fields.judge_prompt_mode,
    });
    if !fields.models.is_empty()
        && let Ok(models) = serde_json::to_value(&fields.models)
    {
        report["models"] = models;
    }
    report["config_signature"] = json!(fields.config_signature);
    if let Some(cost) = fields.cost_micro_usd {
        report["metrics"]["cost_micro_usd"] = json!(cost);
    }
    if let Some(p50) = fields.latency_ms_p50 {
        report["metrics"]["latency_ms_p50"] = json!(p50);
    }
    if let Some(p95) = fields.latency_ms_p95 {
        report["metrics"]["latency_ms_p95"] = json!(p95);
    }
    report["metrics"]["cache"] = json!({
        "cached_input_tokens": fields.cached_input_tokens,
        "uncached_input_tokens": fields.uncached_input_tokens,
        "output_tokens": fields.output_tokens,
        "response_cache_hits": fields.response_cache_hits,
        "prompt_cache_hits": fields.prompt_cache_hits,
        "prompt_cache_partial_hits": fields.prompt_cache_partial_hits,
        "prompt_cache_misses": fields.prompt_cache_misses,
        "roles": fields.role_stats,
    });
}

fn read_json(path: &Path) -> anyhow::Result<serde_json::Value> {
    let raw = std::fs::read_to_string(path)?;
    serde_json::from_str(&raw).map_err(Into::into)
}

#[allow(dead_code)]
fn read_jsonl_values(path: &Path, limit: Option<usize>) -> anyhow::Result<Vec<serde_json::Value>> {
    let raw = std::fs::read_to_string(path)?;
    let mut out = Vec::new();
    for (idx, line) in raw.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        out.push(serde_json::from_str(line).map_err(|err| {
            anyhow::anyhow!(
                "invalid JSONL line {} in {}: {err}",
                idx + 1,
                path.display()
            )
        })?);
        if limit.is_some_and(|limit| out.len() >= limit) {
            break;
        }
    }
    Ok(out)
}

fn nested_u64(value: &serde_json::Value, path: &[&str]) -> Option<u64> {
    let mut cursor = value;
    for key in path {
        cursor = cursor.get(*key)?;
    }
    cursor.as_u64()
}

fn ratio(numerator: Option<u64>, denominator: Option<u64>) -> Option<f64> {
    let denominator = denominator?;
    if denominator == 0 {
        return None;
    }
    Some(numerator? as f64 / denominator as f64)
}

fn artifact_summary(path: &PathBuf) -> anyhow::Result<serde_json::Value> {
    let bytes = std::fs::read(path)?;
    let sha256 = Sha256::digest(&bytes);
    let text = String::from_utf8_lossy(&bytes);
    let non_empty_lines = text.lines().filter(|line| !line.trim().is_empty()).count();
    Ok(json!({
        "path": portable_path(path),
        "bytes": bytes.len(),
        "non_empty_lines": non_empty_lines,
        "sha256": format!("{sha256:x}"),
    }))
}

#[cfg_attr(not(feature = "symbiotic-memory-adapter"), allow(dead_code))]
fn optional_existing(path: PathBuf) -> Option<PathBuf> {
    path.exists().then_some(path)
}

fn native_raw_dir(run_root: &Path) -> PathBuf {
    run_root.join("raw")
}

fn native_hypotheses_path(run_root: &Path) -> PathBuf {
    native_raw_dir(run_root).join("hypotheses.jsonl")
}

#[cfg_attr(not(feature = "symbiotic-memory-adapter"), allow(dead_code))]
fn native_provenance_path(run_root: &Path) -> PathBuf {
    native_raw_dir(run_root).join("provenance.jsonl")
}

#[cfg_attr(not(feature = "symbiotic-memory-adapter"), allow(dead_code))]
fn native_scored_path(run_root: &Path) -> Option<PathBuf> {
    let hypotheses = native_hypotheses_path(run_root);
    optional_existing(PathBuf::from(format!(
        "{}.scored.json",
        hypotheses.to_string_lossy()
    )))
    .or_else(|| {
        optional_existing(native_raw_dir(run_root).join("scores/hypotheses.jsonl.scored.json"))
    })
    .or_else(|| optional_existing(native_raw_dir(run_root).join("scored.json")))
}

#[cfg_attr(not(feature = "symbiotic-memory-adapter"), allow(dead_code))]
fn native_verdicts_path(run_root: &Path) -> Option<PathBuf> {
    let hypotheses = native_hypotheses_path(run_root);
    optional_existing(PathBuf::from(format!(
        "{}.verdicts.jsonl",
        hypotheses.to_string_lossy()
    )))
    .or_else(|| optional_existing(native_raw_dir(run_root).join("verdicts.jsonl")))
}

#[cfg_attr(not(feature = "symbiotic-memory-adapter"), allow(dead_code))]
fn native_partial_verdicts_path(run_root: &Path) -> Option<PathBuf> {
    let hypotheses = native_hypotheses_path(run_root);
    optional_existing(PathBuf::from(format!(
        "{}.partial.verdicts.jsonl",
        hypotheses.to_string_lossy()
    )))
    .or_else(|| optional_existing(native_raw_dir(run_root).join("partial-verdicts.jsonl")))
}

#[cfg_attr(not(feature = "symbiotic-memory-adapter"), allow(dead_code))]
fn native_memory_traces_path(run_root: &Path) -> Option<PathBuf> {
    optional_existing(native_raw_dir(run_root).join("memory-traces.jsonl"))
        .or_else(|| optional_existing(run_root.join("traces").join("memory-events.jsonl")))
}

#[cfg_attr(not(feature = "symbiotic-memory-adapter"), allow(dead_code))]
fn native_model_traces_path(run_root: &Path) -> Option<PathBuf> {
    optional_existing(native_raw_dir(run_root).join("model-traces.jsonl"))
        .or_else(|| optional_existing(run_root.join("model-traces.jsonl")))
        .or_else(|| {
            optional_existing(
                run_root
                    .join("provider-queue")
                    .join("model-queue-traces.jsonl"),
            )
        })
}

#[cfg_attr(not(feature = "symbiotic-memory-adapter"), allow(dead_code))]
fn native_score_summary_path(run_root: &Path) -> Option<PathBuf> {
    optional_existing(native_raw_dir(run_root).join("score-summary.json"))
        .or_else(|| optional_existing(run_root.join("score-summary.json")))
}

fn copy_artifact(
    run_root: &Path,
    source: &Path,
    kind: &str,
    filename: &str,
) -> anyhow::Result<serde_json::Value> {
    let artifact_dir = run_root.join("artifacts");
    std::fs::create_dir_all(&artifact_dir)?;
    let dest = artifact_dir.join(filename);
    if source != dest {
        std::fs::copy(source, &dest)?;
    }
    let mut summary = artifact_summary(&dest)?;
    summary["kind"] = json!(kind);
    Ok(summary)
}

fn write_step_analytics_artifact(run_root: &Path) -> anyhow::Result<Option<serde_json::Value>> {
    let Some(analytics) = step_analytics::derive_run_step_analytics(run_root)? else {
        return Ok(None);
    };
    let artifact_dir = run_root.join("artifacts");
    std::fs::create_dir_all(&artifact_dir)?;
    let path = artifact_dir.join("step-analytics.json");
    std::fs::write(&path, serde_json::to_string_pretty(&analytics)? + "\n")?;
    let mut summary = artifact_summary(&path)?;
    summary["kind"] = json!("step_analytics");
    Ok(Some(summary))
}

#[cfg_attr(not(feature = "symbiotic-memory-adapter"), allow(dead_code))]
fn write_native_provenance(
    run: &SymbioticMemoryCliRun,
    hypotheses: &Path,
    memory_traces: Option<&Path>,
) -> anyhow::Result<PathBuf> {
    let provenance_path = native_provenance_path(&run.run_root);
    if let Some(parent) = provenance_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let judge = resolved_judge_params(run);
    let memory_trace_index = memory_traces
        .map(index_memory_traces_by_question)
        .transpose()?
        .unwrap_or_default();
    let raw = std::fs::read_to_string(hypotheses)?;
    let mut lines = Vec::new();
    for line in raw.lines().filter(|line| !line.trim().is_empty()) {
        let hypothesis: serde_json::Value = serde_json::from_str(line)?;
        let question_id = hypothesis
            .get("question_id")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_string();
        if question_id.is_empty() {
            continue;
        }
        let memory_trace_ids = memory_trace_index
            .get(&question_id)
            .cloned()
            .unwrap_or_default();
        let record = json!({
            "schema": "membench.provenance.v1",
            "question_id": question_id,
            "initial_pick": hypothesis.get("router_initial").cloned().unwrap_or(serde_json::Value::Null),
            "final_pick": hypothesis.get("router_final").cloned().unwrap_or(serde_json::Value::Null),
            "router_reason": hypothesis.get("router_reason").cloned().unwrap_or(serde_json::Value::Null),
            "debug_artifact": hypothesis.get("debug_artifact").cloned().unwrap_or(serde_json::Value::Null),
            "system": "symbiotic-memory",
            "benchmark": "long-mem-eval",
            "run_name": run.run_name,
            "dataset": portable_path(&run.dataset),
            "memory_config": run.memory_config.as_deref().map(portable_path),
            "distiller": run.distiller,
            "embedder": run.embedder,
            "store": run.store,
            "answerer": run.answerer,
            "routed": run.routed,
            "consolidate_briefs": run.consolidate_briefs,
            "query_planner": run.query_planner,
            "scorer": run.scorer,
            "judge_operator": judge.operator.clone(),
            "judge_model": judge.model.clone(),
            "judge_workers": run.judge_workers,
            "memory_trace_ids": memory_trace_ids,
        });
        lines.push(serde_json::to_string(&record)?);
    }
    std::fs::write(&provenance_path, lines.join("\n") + "\n")?;
    Ok(provenance_path)
}

#[cfg_attr(not(feature = "symbiotic-memory-adapter"), allow(dead_code))]
fn index_memory_traces_by_question(path: &Path) -> anyhow::Result<BTreeMap<String, Vec<String>>> {
    let raw = std::fs::read_to_string(path)?;
    let mut index: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for line in raw.lines().filter(|line| !line.trim().is_empty()) {
        let trace: serde_json::Value = serde_json::from_str(line)?;
        let question_id = trace
            .get("question_id")
            .or_else(|| trace.get("source_id"))
            .and_then(|value| value.as_str())
            .unwrap_or_default();
        let trace_id = trace
            .get("trace_id")
            .and_then(|value| value.as_str())
            .unwrap_or_default();
        if !question_id.is_empty() && !trace_id.is_empty() {
            index
                .entry(question_id.to_string())
                .or_default()
                .push(trace_id.to_string());
        }
    }
    Ok(index)
}

fn portable_path(path: &Path) -> String {
    path.strip_prefix(repo_root())
        .unwrap_or(path)
        .display()
        .to_string()
}

#[cfg_attr(not(feature = "symbiotic-memory-adapter"), allow(dead_code))]
fn write_native_benchmark_report(run: &SymbioticMemoryCliRun) -> anyhow::Result<()> {
    let hypotheses = native_hypotheses_path(&run.run_root);
    let scored = native_scored_path(&run.run_root);
    let scored_json = scored.as_ref().map(|path| read_json(path)).transpose()?;
    let correct = scored_json
        .as_ref()
        .and_then(|value| nested_u64(value, &["counts", "total_correct"]));
    let total = scored_json
        .as_ref()
        .and_then(|value| nested_u64(value, &["counts", "scored"]));
    let accuracy = scored_json
        .as_ref()
        .and_then(|value| value.get("overall_accuracy"))
        .and_then(|value| value.as_f64())
        .or_else(|| ratio(correct, total));
    let task_averaged_accuracy = scored_json
        .as_ref()
        .and_then(|value| value.get("task_averaged_accuracy"))
        .and_then(|value| value.as_f64());
    let abstention_correct = scored_json
        .as_ref()
        .and_then(|value| nested_u64(value, &["counts", "abstention_correct"]));
    let abstention_total = scored_json
        .as_ref()
        .and_then(|value| nested_u64(value, &["counts", "abstention_total"]));
    let abstention_accuracy = ratio(abstention_correct, abstention_total).or_else(|| {
        scored_json
            .as_ref()
            .and_then(|value| value.get("abstention_accuracy"))
            .and_then(|value| value.as_f64())
    });

    let hypotheses_artifact =
        copy_artifact(&run.run_root, &hypotheses, "hypotheses", "hypotheses.jsonl")?;
    let scored_artifact = scored
        .as_ref()
        .map(|path| copy_artifact(&run.run_root, path, "scored", "scored.json"))
        .transpose()?;
    let memory_traces_source = native_memory_traces_path(&run.run_root);
    let provenance = write_native_provenance(run, &hypotheses, memory_traces_source.as_deref())?;

    let mut artifacts = serde_json::Map::new();
    artifacts.insert("hypotheses".to_string(), hypotheses_artifact);
    if let Some(scored_artifact) = scored_artifact {
        artifacts.insert("scored".to_string(), scored_artifact);
    }
    artifacts.insert(
        "provenance".to_string(),
        copy_artifact(&run.run_root, &provenance, "provenance", "provenance.jsonl")?,
    );
    for (name, source, file_name) in [
        (
            "verdicts",
            native_verdicts_path(&run.run_root),
            "verdicts.jsonl",
        ),
        (
            "partial_verdicts",
            native_partial_verdicts_path(&run.run_root),
            "partial-verdicts.jsonl",
        ),
        ("memory_traces", memory_traces_source, "memory-traces.jsonl"),
        (
            "model_traces",
            native_model_traces_path(&run.run_root),
            "model-traces.jsonl",
        ),
        (
            "score_summary",
            native_score_summary_path(&run.run_root),
            "score-summary.json",
        ),
    ] {
        if let Some(source) = source {
            artifacts.insert(
                name.to_string(),
                copy_artifact(&run.run_root, &source, name, file_name)?,
            );
        }
    }
    if let Some(step_analytics_artifact) = write_step_analytics_artifact(&run.run_root)? {
        artifacts.insert("step_analytics".to_string(), step_analytics_artifact);
    }
    let artifact_manifest = artifact_manifest(
        artifacts.keys().map(String::as_str),
        true,
        "Native run state folders may include raw outputs, vaults, workflow manifests, and provider queue state.",
    );

    let run_params = symbiotic_memory_run_params(run);
    write_run_params(&run.run_root, &run_params)?;
    let mut report = json!({
        "schema": "membench.report.v1",
        "system": "symbiotic-memory",
        "benchmark": "long-mem-eval",
        "run_kind": "native",
        "run_name": run.run_name,
        "run_params": run_params,
        "metrics": {
            "accuracy": {
                "correct": correct,
                "total": total,
                "value": accuracy,
            },
            "task_averaged_accuracy": task_averaged_accuracy,
            "abstention_accuracy": {
                "correct": abstention_correct,
                "total": abstention_total,
                "value": abstention_accuracy,
            }
        },
        "artifact_manifest": artifact_manifest,
        "artifacts": serde_json::Value::Object(artifacts),
    });
    enrich_report_with_cohort(
        &mut report,
        &run.run_root,
        &symbiotic_memory_run_params(run),
    );
    std::fs::write(
        run.run_root.join("benchmark-report.json"),
        serde_json::to_string_pretty(&report)? + "\n",
    )?;
    println!("{}", render_benchmark_report(&report));
    Ok(())
}

fn infer_run_name(path: &std::path::Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| {
            name.strip_suffix(".jsonl")
                .or_else(|| name.strip_suffix(".json"))
                .unwrap_or(name)
                .trim_start_matches("hyp-")
                .to_string()
        })
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "imported".to_string())
}

fn default_run_root(
    registry_root: &std::path::Path,
    system: &str,
    benchmark: &str,
    run_name: &str,
) -> PathBuf {
    registry_root
        .join(sanitize_path_component(system))
        .join(sanitize_path_component(benchmark))
        .join(sanitize_path_component(run_name))
}

fn default_native_run_root(
    registry_root: &std::path::Path,
    system: &str,
    benchmark: &str,
    limit: usize,
    run_name: &str,
) -> PathBuf {
    default_grouped_run_root(registry_root, system, benchmark, limit as u64, run_name)
}

fn default_smoke_run_root(
    registry_root: &std::path::Path,
    system: &str,
    benchmark: &str,
    limit: usize,
    run_name: &str,
) -> PathBuf {
    registry_root
        .join(".tmp")
        .join(sanitize_path_component(system))
        .join(sanitize_path_component(benchmark))
        .join(limit.to_string())
        .join(sanitize_path_component(run_name))
}

fn default_import_run_root(
    registry_root: &std::path::Path,
    system: &str,
    benchmark: &str,
    scored: &Path,
    run_name: &str,
) -> anyhow::Result<PathBuf> {
    let scored_json = read_json(scored)?;
    Ok(nested_u64(&scored_json, &["counts", "scored"])
        .map(|limit| default_grouped_run_root(registry_root, system, benchmark, limit, run_name))
        .unwrap_or_else(|| default_run_root(registry_root, system, benchmark, run_name)))
}

fn default_grouped_run_root(
    root: &std::path::Path,
    system: &str,
    benchmark: &str,
    limit: u64,
    run_name: &str,
) -> PathBuf {
    root.join(sanitize_path_component(system))
        .join(sanitize_path_component(benchmark))
        .join(limit.to_string())
        .join(sanitize_path_component(run_name))
}

fn sanitize_path_component(value: &str) -> String {
    let sanitized: String = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
                ch
            } else {
                '-'
            }
        })
        .collect();
    let sanitized = sanitized.trim_matches('-');
    if sanitized.is_empty() {
        "unnamed".to_string()
    } else {
        sanitized.to_string()
    }
}

/// Emit the normalized run index (or a single run) as JSON, the same shape the
/// dashboard backend serves.
fn explore_runs_json(run_root: Option<PathBuf>) -> anyhow::Result<()> {
    let repo = repo_root();
    let roots = match &run_root {
        Some(root) => vec![root.clone()],
        None => vec![repo.join("runs"), repo.join("records")],
    };
    let records = registry::scan_registry(&roots, &repo);
    let trial_index = registry::scan_trial_markers(&repo);
    let summaries: Vec<registry::RunSummary> = records
        .iter()
        .map(|record| registry::summarize_with_trials(record, &trial_index))
        .collect();
    println!("{}", serde_json::to_string_pretty(&summaries)?);
    Ok(())
}

fn explore_runs(run_root: Option<PathBuf>, registry_root: PathBuf) -> anyhow::Result<()> {
    if let Some(run_root) = run_root {
        return explore_run(run_root);
    }
    list_registry_runs(registry_root)
}

fn explore_run(run_root: PathBuf) -> anyhow::Result<()> {
    let report_path = run_root.join("benchmark-report.json");
    if report_path.exists() {
        let raw = std::fs::read_to_string(&report_path)
            .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", report_path.display()))?;
        let report: serde_json::Value = serde_json::from_str(&raw)?;
        println!("{}", render_benchmark_report(&report));
        return Ok(());
    }
    let params_path = run_root.join("run-params.json");
    let raw = std::fs::read_to_string(&params_path)
        .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", params_path.display()))?;
    let params: serde_json::Value = serde_json::from_str(&raw)?;
    println!("{}", render_run_params(&params));
    Ok(())
}

fn write_analytics_for_run(run_root: PathBuf) -> anyhow::Result<()> {
    let Some(artifact) = write_step_analytics_artifact(&run_root)? else {
        anyhow::bail!(
            "cannot derive step analytics: no memory/model traces found under {}",
            portable_path(&run_root)
        );
    };
    update_report_with_step_analytics(&run_root, artifact.clone())?;
    println!(
        "wrote {}",
        artifact
            .get("path")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("artifacts/step-analytics.json")
    );
    Ok(())
}

fn update_report_with_step_analytics(
    run_root: &Path,
    artifact: serde_json::Value,
) -> anyhow::Result<()> {
    let report_path = run_root.join("benchmark-report.json");
    if !report_path.is_file() {
        return Ok(());
    }
    let mut report = read_json(&report_path)?;
    report["artifacts"]["step_analytics"] = artifact;
    let available = report["artifacts"]
        .as_object()
        .into_iter()
        .flat_map(|object| object.keys().cloned())
        .collect::<Vec<_>>();
    let native_state_available = report
        .get("artifact_manifest")
        .and_then(|manifest| manifest.get("native_state_available"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let native_state_note = report
        .get("artifact_manifest")
        .and_then(|manifest| manifest.get("native_state_note"))
        .and_then(serde_json::Value::as_str)
        .map(ToOwned::to_owned)
        .unwrap_or_default();
    report["artifact_manifest"] = artifact_manifest(
        available.iter().map(String::as_str),
        native_state_available,
        &native_state_note,
    );
    std::fs::write(report_path, serde_json::to_string_pretty(&report)? + "\n")?;
    Ok(())
}

fn list_registry_runs(registry_root: PathBuf) -> anyhow::Result<()> {
    let mut reports = Vec::new();
    registry::collect_report_paths(&registry_root, &mut reports);
    reports.sort();
    if reports.is_empty() {
        println!(
            "No benchmark reports found under {}",
            registry_root.display()
        );
        return Ok(());
    }
    println!("Benchmark Runs");
    println!("==============");
    for report_path in reports {
        let raw = std::fs::read_to_string(&report_path)?;
        let report: serde_json::Value = serde_json::from_str(&raw)?;
        let system = text_at(&report, &["system"]).unwrap_or("unknown");
        let benchmark = text_at(&report, &["benchmark"]).unwrap_or("unknown");
        let run_name = text_at(&report, &["run_name"]).unwrap_or("unnamed");
        let run_kind = text_at(&report, &["run_kind"]).unwrap_or("unknown");
        let accuracy = nested_f64(&report, &["metrics", "accuracy", "value"])
            .map(|value| format!("{value:.3}"))
            .unwrap_or_else(|| "n/a".to_string());
        let completeness = artifact_manifest_list_summary(report.get("artifact_manifest"));
        let run_root = report_path
            .parent()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "n/a".to_string());
        println!(
            "{system} / {benchmark} / {run_name}  kind={run_kind}  accuracy={accuracy}  {completeness}  root={run_root}"
        );
    }
    Ok(())
}

fn save_record(
    run_root: PathBuf,
    records_root: PathBuf,
    record_name: Option<String>,
    force: bool,
) -> anyhow::Result<()> {
    let report_path = run_root.join("benchmark-report.json");
    let raw = std::fs::read_to_string(&report_path)
        .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", report_path.display()))?;
    let report: serde_json::Value = serde_json::from_str(&raw)?;
    let system = text_at(&report, &["system"]).unwrap_or("unknown");
    let benchmark = text_at(&report, &["benchmark"]).unwrap_or("unknown");
    let run_name = record_name
        .or_else(|| text_at(&report, &["run_name"]).map(ToOwned::to_owned))
        .unwrap_or_else(|| "unnamed".to_string());
    let dest = record_run_root(&records_root, system, benchmark, &report, &run_name);
    if dest.exists() {
        if !force {
            anyhow::bail!(
                "record already exists at {}; pass --force to overwrite",
                dest.display()
            );
        }
        std::fs::remove_dir_all(&dest)?;
    }
    copy_dir_recursive(&run_root, &dest)?;
    println!("saved record: {}", dest.display());
    Ok(())
}

fn record_run_root(
    records_root: &std::path::Path,
    system: &str,
    benchmark: &str,
    report: &serde_json::Value,
    run_name: &str,
) -> PathBuf {
    let limit = nested_u64(report, &["run_params", "limit"])
        .or_else(|| nested_u64(report, &["metrics", "accuracy", "total"]));
    if let Some(limit) = limit {
        return records_root
            .join(sanitize_path_component(system))
            .join(sanitize_path_component(benchmark))
            .join(limit.to_string())
            .join(sanitize_path_component(run_name));
    }
    default_run_root(records_root, system, benchmark, run_name)
}

fn copy_dir_recursive(source: &std::path::Path, dest: &std::path::Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(dest)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let source_entry_path = entry.path();
        let dest_path = dest.join(entry.file_name());
        if source_entry_path.is_dir() {
            copy_dir_recursive(&source_entry_path, &dest_path)?;
        } else {
            std::fs::copy(&source_entry_path, &dest_path)?;
        }
    }
    Ok(())
}

/// Environment variable naming the canonical vault store: an absolute path to a
/// directory holding `<key>/vaults/` backups outside the disposable `runs/` tree.
/// Unset ⇒ the store feature is inert and all current behavior is unchanged.
const SYMEM_VAULT_STORE_ENV: &str = "SYMEM_VAULT_STORE";

/// Read `$SYMEM_VAULT_STORE` if set to a non-empty value. Inert when unset; used
/// by transparent by-name discovery, which must never fail just because the
/// store is not configured.
fn vault_store_dir_opt() -> Option<PathBuf> {
    std::env::var_os(SYMEM_VAULT_STORE_ENV)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

/// Like [`vault_store_dir_opt`], but errors clearly for the `vault` subcommands,
/// which cannot operate without a configured store.
fn require_vault_store_dir() -> anyhow::Result<PathBuf> {
    vault_store_dir_opt().ok_or_else(|| {
        anyhow::anyhow!(
            "{SYMEM_VAULT_STORE_ENV} is not set; export it to an absolute store directory, e.g. {SYMEM_VAULT_STORE_ENV}=/Users/me/membench-vaults"
        )
    })
}

/// Resolve a `--source-vault-root` value with transparent by-name discovery: an
/// existing path always wins, but a bare key that names `$SYMEM_VAULT_STORE/<key>/vaults`
/// falls back to the store (logging one line). Never overrides an existing path.
#[cfg_attr(not(feature = "symbiotic-memory-adapter"), allow(dead_code))]
fn resolve_source_vault_root(value: &Path) -> PathBuf {
    resolve_source_vault_root_with_store(value, vault_store_dir_opt().as_deref())
}

/// The gold-evidence piece a turn belongs to. Turn ids look like
/// `answer_<hash>[_<N>]:<turn_index>`; the piece is everything before the first
/// `:`. NB: split on `:` only — never strip the trailing `_N`, which would
/// collapse `answer_X_1..answer_X_4` into one piece (a verified prior bug).
fn gold_piece_of_turn(turn_id: &str) -> &str {
    turn_id.split(':').next().unwrap_or(turn_id)
}

/// True when an answerer evidence id is a raw conversation turn (a turn id like
/// `<hash>_<N>:<M>`) rather than a distilled `mem-` fact or a `brief-` summary.
fn evidence_id_is_raw_turn(id: &str) -> bool {
    id.contains(':') && !id.starts_with("mem-") && !id.starts_with("brief-")
}

/// The turn-level gold set for one record: every `<session_id>:<turn_index>`
/// whose message carries `has_answer == true`. This is finer-grained than the
/// session-level `answer_session_ids`, and is what the rerank trace ranks (its
/// raw candidates are turn ids). `haystack_session_ids[sk]` names session `sk`
/// and `haystack_sessions[sk][ti]` is its `ti`-th turn.
fn gold_turn_ids(
    record: &symbiotic_mem_bench::symbiotic_memory_adapter::LongMemEvalRecord,
) -> BTreeSet<String> {
    let mut gold = BTreeSet::new();
    for (sk, session) in record.haystack_sessions.iter().enumerate() {
        let Some(session_id) = record.haystack_session_ids.get(sk) else {
            continue;
        };
        for (ti, message) in session.iter().enumerate() {
            if message.has_answer {
                gold.insert(format!("{session_id}:{ti}"));
            }
        }
    }
    gold
}

/// One raw-turn rerank candidate, normalized away from the merged-vs-separate
/// trace artifact: an id stripped of any `raw:` prefix plus its two scores.
struct RawCand {
    id: String,
    embedding_score: f64,
    rerank_score: f64,
}

/// Pull the raw-turn candidates out of a `rerank_trace`, regardless of trace
/// shape, so embed/rerank ranks compare fairly across runs:
///   - **merged** trace (`candidate_type:"merged"`): keep only ids beginning
///     `raw:`, strip that prefix. (Their global `embedding_rank` is polluted by
///     interleaved facts, so we ignore it and re-rank by score below.)
///   - **separate** trace (`candidate_type:"raw_turn"`): the candidate list is
///     already raw-only with bare ids.
/// Missing scores fall back to `-inf` (embedding) / `final_rank` is folded in by
/// the caller; a candidate with neither a rerank score nor a final rank sorts
/// last. Returns the de-duplicated raw candidates (first occurrence wins).
fn raw_turn_candidates(traces: &[Value]) -> Vec<RawCand> {
    let mut out: Vec<RawCand> = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for trace in traces {
        let ctype = trace
            .get("candidate_type")
            .and_then(|t| t.as_str())
            .unwrap_or("");
        let merged = ctype == "merged";
        if !merged && ctype != "raw_turn" {
            continue;
        }
        let Some(candidates) = trace.get("candidates").and_then(|list| list.as_array()) else {
            continue;
        };
        for candidate in candidates {
            let Some(raw_id) = candidate.get("candidate_id").and_then(|c| c.as_str()) else {
                continue;
            };
            // In a merged trace, only `raw:`-prefixed candidates are raw turns;
            // facts/briefs are excluded. In a separate raw_turn trace, ids are
            // already bare turn ids (tolerate a stray `raw:` prefix anyway).
            if merged && !raw_id.starts_with("raw:") {
                continue;
            }
            let id = raw_id.strip_prefix("raw:").unwrap_or(raw_id).to_string();
            if !seen.insert(id.clone()) {
                continue;
            }
            let embedding_score = candidate
                .get("embedding_score")
                .and_then(|s| s.as_f64())
                .unwrap_or(f64::NEG_INFINITY);
            // Prefer the rerank score; fall back to a synthetic score derived
            // from `final_rank` (lower rank => higher score) so the rerank order
            // is preserved even when only ranks were recorded.
            let rerank_score = candidate
                .get("rerank_score")
                .and_then(|s| s.as_f64())
                .or_else(|| {
                    candidate
                        .get("final_rank")
                        .and_then(|r| r.as_f64())
                        .map(|rank| -rank)
                })
                .unwrap_or(f64::NEG_INFINITY);
            out.push(RawCand {
                id,
                embedding_score,
                rerank_score,
            });
        }
    }
    out
}

/// 1-based rank of the **deepest** (worst-ranked) gold turn among the raw-turn
/// candidates, after re-ranking them *among themselves* by the given key (embed
/// score or rerank score, both descending). `None` when no gold turn appears in
/// the candidate set. Ties keep input order, which is stable across the two
/// passes so a gold turn never floats above an equal-scored neighbor in one pass
/// and below it in the other.
fn deepest_gold_rank(
    cands: &[RawCand],
    gold: &BTreeSet<String>,
    key: impl Fn(&RawCand) -> f64,
) -> Option<usize> {
    let mut order: Vec<usize> = (0..cands.len()).collect();
    order.sort_by(|&a, &b| {
        key(&cands[b])
            .partial_cmp(&key(&cands[a]))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut deepest = None;
    for (rank0, &idx) in order.iter().enumerate() {
        if gold.contains(&cands[idx].id) {
            deepest = Some(rank0 + 1);
        }
    }
    deepest
}

/// `gold-eval`: ground every question in LongMemEval's own `answer_session_ids`
/// annotation. For each question we join the dataset gold pieces with the run's
/// kept distilled facts and verdict, then classify:
///   - `correct`        : the judge marked the answer correct.
///   - `retrieval_gap`  : wrong/abstained AND some gold piece is missing from the
///                        kept distilled facts (the distillery never captured it).
///   - `reader_fail`    : wrong/abstained AND every gold piece is present (the
///                        evidence was there; the reader still failed).
/// A second, descriptive axis reports per gold piece HOW it is covered —
/// distilled `fact`, `raw` turn, `both`, or `none` — without feeding the class.
///
/// GOLD IDENTITY — the one valid method. Gold is the question's
/// `answer_session_ids` annotation, refined to the `has_answer` turns within
/// those sessions (`gold_turn_ids`), matched to candidates by TURN ID, and
/// ranked with `deepest_gold_rank` (raw-only: sort raw candidates by embed
/// score, then by rerank score). The per-question record lands in
/// `artifacts/gold-eval.json` (`gold_embed_rank` / `gold_rerank_rank` /
/// `gold_top_rank` / `gold_deepest_rank` / `gold_turns_in_set`, and coverage by
/// fact/raw/both/none). NEVER identify gold by substring-matching the answer
/// text against candidate content: that over-matches (a candidate that merely
/// contains the answer string by coincidence) and under-matches (paraphrased
/// gold), and it is not the dataset's ground truth. (A substring "forensics"
/// helper once shipped in the adapter and misled analysis; it was removed.)
fn gold_eval(run: &str) -> anyhow::Result<()> {
    let run_root = resolve_run_for_vault_save(run)?;
    let run_name = run_root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(run)
        .to_string();

    // Dataset path from run-params.json (repo-relative or absolute).
    let params = read_json(&run_root.join("run-params.json"))
        .with_context(|| format!("reading run-params.json under {}", portable_path(&run_root)))?;
    let dataset_field = params
        .get("dataset")
        .and_then(|value| value.as_str())
        .ok_or_else(|| anyhow::anyhow!("run-params.json has no string `dataset` field"))?;
    let dataset_path = resolve_repo_path(Path::new(dataset_field));
    let records =
        symbiotic_mem_bench::symbiotic_memory_adapter::load_longmemeval(&dataset_path, None)
            .with_context(|| format!("loading dataset {}", portable_path(&dataset_path)))?;

    // Verdicts keyed by question id; correctness from the autoeval boolean (the
    // `label` string "incorrect" contains "correct" — never substring it).
    let mut correct_by_qid: BTreeMap<String, bool> = BTreeMap::new();
    let mut abstain_by_qid: BTreeMap<String, bool> = BTreeMap::new();
    for verdict in symbiotic_mem_bench::artifacts::read_verdicts(&run_root) {
        let correct = verdict
            .autoeval_label
            .as_ref()
            .and_then(|auto| auto.label)
            .or(verdict.label)
            .unwrap_or(false);
        correct_by_qid.insert(verdict.question_id.clone(), correct);
        abstain_by_qid.insert(verdict.question_id, verdict.is_abstention.unwrap_or(false));
    }

    let mut questions = Vec::new();
    // ALL-question tallies.
    let (mut total, mut correct, mut wrong, mut abstained) = (0usize, 0usize, 0usize, 0usize);
    let (mut single_piece, mut multi_piece) = (0usize, 0usize);
    let (mut cls_correct, mut cls_reader, mut cls_gap) = (0usize, 0usize, 0usize);
    let (mut src_fact, mut src_raw, mut src_both, mut src_none) = (0usize, 0usize, 0usize, 0usize);
    // Multi-piece piece-coverage (single-piece questions trivially cover; the
    // validated reference reports the multi-piece scope).
    let (mut multi_gold_needed, mut multi_gold_covered) = (0usize, 0usize);
    // Gold-turn retrieval-rank distributions (deepest gold turn, raw-only,
    // ranked among raw candidates by embed score then by rerank score). The
    // denominator is questions whose deepest gold turn appears in the raw
    // candidate set at all (`*_in_set`); ranks for absent gold are `None`.
    let mut embed_ranks: Vec<usize> = Vec::new();
    let mut rerank_ranks: Vec<usize> = Vec::new();
    let (mut gold_turns_in_set_total, mut gold_turns_total_total) = (0usize, 0usize);

    for record in &records {
        let qid = &record.question_id;
        let gold: Vec<String> = record.answer_session_ids.clone();
        let n_gold = gold.len();
        let is_correct = correct_by_qid.get(qid).copied().unwrap_or(false);
        let is_abstained = abstain_by_qid.get(qid).copied().unwrap_or(false);

        // Per-question kept facts + raw evidence from the debug artifact.
        let debug_path = run_root
            .join("vaults")
            .join(qid)
            .join("debug")
            .join("question-debug.json");
        let debug = read_json(&debug_path).ok();

        // Distilled-fact coverage: a gold piece is fact-covered when a kept fact's
        // source_refs include a turn from it. Facts carry a `.score` for ranking.
        let mut fact_pieces: BTreeSet<String> = BTreeSet::new();
        let mut scored_pieces: Vec<(f64, BTreeSet<String>)> = Vec::new();
        // Raw-turn coverage (descriptive only): kept raw evidence + raw_turn
        // rerank candidates. candidate ids here are bare turn ids; tolerate a
        // legacy `raw:` prefix too.
        let mut raw_pieces: BTreeSet<String> = BTreeSet::new();
        if let Some(debug) = &debug {
            let recall = debug.get("recall");
            if let Some(facts) = recall
                .and_then(|recall| recall.get("initial_profile"))
                .and_then(|profile| profile.get("facts"))
                .and_then(|facts| facts.as_array())
            {
                for entry in facts {
                    let score = entry.get("score").and_then(|score| score.as_f64());
                    let mut pieces = BTreeSet::new();
                    if let Some(refs) = entry
                        .get("fact")
                        .and_then(|fact| fact.get("source_refs"))
                        .and_then(|refs| refs.as_array())
                    {
                        for source in refs {
                            if let Some(turn) = source.get("turn_id").and_then(|t| t.as_str()) {
                                let piece = gold_piece_of_turn(turn).to_string();
                                fact_pieces.insert(piece.clone());
                                pieces.insert(piece);
                            }
                        }
                    }
                    scored_pieces.push((score.unwrap_or(f64::NEG_INFINITY), pieces));
                }
            }
            if let Some(evidence) = recall
                .and_then(|recall| recall.get("final_answer"))
                .and_then(|answer| answer.get("evidence_ids"))
                .and_then(|ids| ids.as_array())
            {
                for id in evidence {
                    if let Some(id) = id.as_str() {
                        if evidence_id_is_raw_turn(id) {
                            raw_pieces.insert(gold_piece_of_turn(id).to_string());
                        }
                    }
                }
            }
            if let Some(traces) = recall
                .and_then(|recall| recall.get("rerank_trace"))
                .and_then(|trace| trace.as_array())
            {
                for trace in traces {
                    if trace.get("candidate_type").and_then(|t| t.as_str()) != Some("raw_turn") {
                        continue;
                    }
                    if let Some(candidates) =
                        trace.get("candidates").and_then(|list| list.as_array())
                    {
                        for candidate in candidates {
                            if let Some(id) = candidate.get("candidate_id").and_then(|c| c.as_str())
                            {
                                let id = id.strip_prefix("raw:").unwrap_or(id);
                                raw_pieces.insert(gold_piece_of_turn(id).to_string());
                            }
                        }
                    }
                }
            }
        }

        // Gold-turn retrieval rank after embedding vs after rerank. Turn-level
        // gold (`has_answer` turns) is matched against the RAW-TURN candidates
        // re-ranked among themselves — embed score for the embed rank, rerank
        // score (final_rank fallback) for the rerank rank — so merged and
        // separate traces yield comparable ranks. We report the deepest (worst)
        // gold turn's rank, which is the bar the reader must clear to see *all*
        // gold for the question.
        let gold_turns = gold_turn_ids(record);
        let gold_turns_total = gold_turns.len();
        let raw_cands = debug
            .as_ref()
            .and_then(|debug| debug.get("recall"))
            .and_then(|recall| recall.get("rerank_trace"))
            .and_then(|trace| trace.as_array())
            .map(|traces| raw_turn_candidates(traces))
            .unwrap_or_default();
        let gold_embed_rank = deepest_gold_rank(&raw_cands, &gold_turns, |c| c.embedding_score);
        let gold_rerank_rank = deepest_gold_rank(&raw_cands, &gold_turns, |c| c.rerank_score);
        // A gold turn is "in set" when it appears among the raw candidates; the
        // embed pass and rerank pass rank the same set, so either rank's
        // presence is the same signal — use the embed pass.
        let gold_turns_in_set = gold_turns
            .iter()
            .filter(|turn| raw_cands.iter().any(|c| &&c.id == turn))
            .count();
        gold_turns_in_set_total += gold_turns_in_set;
        gold_turns_total_total += gold_turns_total;
        if let Some(rank) = gold_embed_rank {
            embed_ranks.push(rank);
        }
        if let Some(rank) = gold_rerank_rank {
            rerank_ranks.push(rank);
        }

        // 1-based ranks of the first / deepest gold-piece fact (facts by score desc).
        scored_pieces.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        let gold_set: BTreeSet<&String> = gold.iter().collect();
        let (mut gold_top_rank, mut gold_deepest_rank): (Option<usize>, Option<usize>) =
            (None, None);
        for (idx, (_, pieces)) in scored_pieces.iter().enumerate() {
            if pieces.iter().any(|piece| gold_set.contains(piece)) {
                let rank = idx + 1;
                gold_top_rank.get_or_insert(rank);
                gold_deepest_rank = Some(rank);
            }
        }

        // Per gold piece: classification coverage is distill-only; the
        // fact/raw/both/none axis is reported alongside.
        let mut covered_pieces = 0usize;
        let mut missing_pieces: Vec<String> = Vec::new();
        let (mut covered_by_fact, mut covered_by_raw) = (0usize, 0usize);
        for piece in &gold {
            let by_fact = fact_pieces.contains(piece);
            let by_raw = raw_pieces.contains(piece);
            if by_fact {
                covered_by_fact += 1;
            }
            if by_raw {
                covered_by_raw += 1;
            }
            match (by_fact, by_raw) {
                (true, true) => src_both += 1,
                (true, false) => src_fact += 1,
                (false, true) => src_raw += 1,
                (false, false) => src_none += 1,
            }
            // Coverage that drives the class is distilled-fact coverage.
            if by_fact {
                covered_pieces += 1;
            } else {
                missing_pieces.push(piece.clone());
            }
        }

        let class = if is_correct {
            cls_correct += 1;
            "correct"
        } else if covered_pieces < n_gold {
            cls_gap += 1;
            "retrieval_gap"
        } else {
            cls_reader += 1;
            "reader_fail"
        };

        total += 1;
        if is_correct {
            correct += 1;
        } else {
            wrong += 1;
        }
        if is_abstained {
            abstained += 1;
        }
        if n_gold > 1 {
            multi_piece += 1;
            multi_gold_needed += n_gold;
            multi_gold_covered += covered_pieces;
        } else {
            single_piece += 1;
        }

        questions.push(json!({
            "qid": qid,
            "type": record.question_type,
            "answer": record.answer,
            "n_gold_pieces": n_gold,
            "covered_pieces": covered_pieces,
            "missing_pieces": missing_pieces,
            "covered_by_fact": covered_by_fact,
            "covered_by_raw": covered_by_raw,
            "gold_top_rank": gold_top_rank,
            "gold_deepest_rank": gold_deepest_rank,
            "gold_embed_rank": gold_embed_rank,
            "gold_rerank_rank": gold_rerank_rank,
            "gold_turns_in_set": gold_turns_in_set,
            "gold_turns_total": gold_turns_total,
            "correct": is_correct,
            "abstained": is_abstained,
            "class": class,
        }));
    }

    let piece_coverage = if multi_gold_needed == 0 {
        0.0
    } else {
        (multi_gold_covered as f64 / multi_gold_needed as f64 * 1000.0).round() / 1000.0
    };

    // Gold-turn rank distribution over the questions whose deepest gold turn
    // landed in the raw candidate set. `within(N)` is the fraction at rank <= N
    // (top-N recall of the deepest gold turn); `mean` is the average rank. Both
    // rounded to 3 decimals; `n` is the in-set question count (the denominator).
    let rank_distribution = |ranks: &[usize]| {
        let n = ranks.len();
        let round3 = |value: f64| (value * 1000.0).round() / 1000.0;
        let within = |limit: usize| {
            if n == 0 {
                0.0
            } else {
                round3(ranks.iter().filter(|&&r| r <= limit).count() as f64 / n as f64)
            }
        };
        let mean = if n == 0 {
            0.0
        } else {
            round3(ranks.iter().sum::<usize>() as f64 / n as f64)
        };
        json!({
            "n": n,
            "within_10": within(10),
            "within_20": within(20),
            "within_50": within(50),
            "within_100": within(100),
            "mean": mean,
        })
    };
    let gold_turn_in_set_pct = if gold_turns_total_total == 0 {
        0.0
    } else {
        (gold_turns_in_set_total as f64 / gold_turns_total_total as f64 * 1000.0).round() / 1000.0
    };
    let gold_rank_summary = json!({
        "embed": rank_distribution(&embed_ranks),
        "rerank": rank_distribution(&rerank_ranks),
        "gold_turns_in_set": gold_turns_in_set_total,
        "gold_turns_total": gold_turns_total_total,
        "gold_turn_in_set_pct": gold_turn_in_set_pct,
    });

    let report = json!({
        "schema_version": 1,
        "run_name": run_name,
        "dataset_path": portable_path(&dataset_path),
        "summary": {
            "total": total,
            "correct": correct,
            "wrong": wrong,
            "abstained": abstained,
            "single_piece": single_piece,
            "multi_piece": multi_piece,
            "gold_pieces_needed": multi_gold_needed,
            "gold_pieces_covered": multi_gold_covered,
            "piece_coverage": piece_coverage,
            "class_counts": {
                "correct": cls_correct,
                "reader_fail": cls_reader,
                "retrieval_gap": cls_gap,
            },
            "coverage_by_source": {
                "fact": src_fact,
                "raw": src_raw,
                "both": src_both,
                "none": src_none,
            },
            "gold_rank": gold_rank_summary,
        },
        "questions": questions,
    });

    let out_dir = run_root.join("artifacts");
    std::fs::create_dir_all(&out_dir)?;
    let out_path = out_dir.join("gold-eval.json");
    let partial = out_path.with_extension("json.partial");
    std::fs::write(&partial, serde_json::to_vec_pretty(&report)?)?;
    std::fs::rename(&partial, &out_path)?;

    let fmt_within = |dist: &Value| {
        let g = |key: &str| dist.get(key).and_then(|v| v.as_f64()).unwrap_or(0.0);
        format!(
            "top10={:.3} top20={:.3} top50={:.3} top100={:.3} mean={:.1} n={}",
            g("within_10"),
            g("within_20"),
            g("within_50"),
            g("within_100"),
            g("mean"),
            dist.get("n").and_then(|v| v.as_u64()).unwrap_or(0),
        )
    };
    println!(
        "gold-eval: {} -> {}\n  total={total} correct={correct} wrong={wrong} abstained={abstained} (single={single_piece} multi={multi_piece})\n  classes: correct={cls_correct} reader_fail={cls_reader} retrieval_gap={cls_gap}\n  multi-piece gold coverage: {multi_gold_covered}/{multi_gold_needed} ({piece_coverage:.3})\n  coverage_by_source: fact={src_fact} raw={src_raw} both={src_both} none={src_none}\n  gold-turn in set: {gold_turns_in_set_total}/{gold_turns_total_total} ({gold_turn_in_set_pct:.3})\n  deepest gold rank | embed:  {}\n                    | rerank: {}",
        run_name,
        portable_path(&out_path),
        fmt_within(&gold_rank_summary["embed"]),
        fmt_within(&gold_rank_summary["rerank"]),
    );
    Ok(())
}

/// Resolve a `vault save --run` value to a run root: an existing path (absolute
/// or repo-relative) wins; otherwise treat it as a run dir name and search under
/// `runs/symbiotic-memory/long-mem-eval/<limit>/<name>`.
fn resolve_run_for_vault_save(run: &str) -> anyhow::Result<PathBuf> {
    let direct = resolve_repo_path(Path::new(run));
    if direct.is_dir() {
        return Ok(direct);
    }
    let grouped_root = repo_root()
        .join("runs")
        .join("symbiotic-memory")
        .join("long-mem-eval");
    if let Ok(entries) = std::fs::read_dir(&grouped_root) {
        let mut matches: Vec<PathBuf> = Vec::new();
        for limit_entry in entries.flatten() {
            let candidate = limit_entry.path().join(run);
            if candidate.is_dir() {
                matches.push(candidate);
            }
        }
        matches.sort();
        if let Some(found) = matches.first() {
            return Ok(found.clone());
        }
    }
    anyhow::bail!(
        "could not resolve run '{run}': not an existing path and not found under {}/<limit>/{run}",
        portable_path(&grouped_root)
    )
}

/// Best-effort accuracy for a run, from `benchmark-report.json` then `score-summary.json`.
fn read_run_accuracy(run_root: &Path) -> Option<f64> {
    let report = run_root.join("benchmark-report.json");
    if let Ok(value) = read_json(&report) {
        if let Some(accuracy) = nested_f64(&value, &["metrics", "accuracy", "value"]) {
            return Some(accuracy);
        }
    }
    let summary = native_score_summary_path(run_root)?;
    let value = read_json(&summary).ok()?;
    nested_f64(&value, &["metrics", "overall_accuracy"])
        .or_else(|| nested_f64(&value, &["overall_accuracy"]))
}

/// Number of per-question vault subdirectories directly under a `vaults/` dir.
fn count_vault_subdirs(vaults_dir: &Path) -> usize {
    std::fs::read_dir(vaults_dir)
        .map(|entries| {
            entries
                .flatten()
                .filter(|entry| entry.path().is_dir())
                .count()
        })
        .unwrap_or(0)
}

/// Human-readable directory size via `du -sh` (best effort; "n/a" on failure).
fn dir_size_human(path: &Path) -> String {
    std::process::Command::new("du")
        .arg("-sh")
        .arg(path)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| {
            String::from_utf8_lossy(&output.stdout)
                .split_whitespace()
                .next()
                .map(ToOwned::to_owned)
        })
        .unwrap_or_else(|| "n/a".to_string())
}

/// `vault save`: copy (default) or move a run's `vaults/` into the canonical
/// store at `$SYMEM_VAULT_STORE/<key>/vaults/`, then write `store-meta.json`.
fn vault_save(run: &str, key: Option<&str>, move_vaults: bool) -> anyhow::Result<()> {
    let store = require_vault_store_dir()?;
    let run_root = resolve_run_for_vault_save(run)?;
    vault_save_in(&store, &run_root, key, move_vaults)
}

fn vault_save_in(
    store: &Path,
    run_root: &Path,
    key: Option<&str>,
    move_vaults: bool,
) -> anyhow::Result<()> {
    let source_vaults = run_root.join("vaults");
    if !source_vaults.is_dir() {
        anyhow::bail!(
            "run {} has no vaults/ directory to save",
            portable_path(run_root)
        );
    }
    let run_dir_name = run_root
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "could not derive a key from run path {}",
                run_root.display()
            )
        })?;
    let key = key.unwrap_or(run_dir_name);
    let key_dir = store.join(sanitize_path_component(key));
    let dest_vaults = key_dir.join("vaults");
    if dest_vaults.exists() {
        anyhow::bail!(
            "store key already populated at {}; pick a different --as key or remove it first",
            portable_path(&dest_vaults)
        );
    }
    std::fs::create_dir_all(&key_dir)?;

    if move_vaults {
        // Move the tree, then leave a symlink behind so `--source-vault-root <run>`
        // and answer-only reruns keep resolving the (now relocated) vault data.
        std::fs::rename(&source_vaults, &dest_vaults).or_else(|err| {
            // Fall back to copy+remove across filesystem boundaries (EXDEV).
            if err.raw_os_error() == Some(libc_exdev()) {
                copy_dir_recursive(&source_vaults, &dest_vaults)?;
                std::fs::remove_dir_all(&source_vaults)?;
                Ok(())
            } else {
                Err(anyhow::Error::from(err))
            }
        })?;
        symlink_dir(&dest_vaults, &source_vaults)?;
    } else {
        copy_dir_recursive(&source_vaults, &dest_vaults)?;
    }

    let vault_count = count_vault_subdirs(&dest_vaults);
    let saved_at = chrono::DateTime::<Utc>::from(std::time::SystemTime::now())
        .to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let meta = json!({
        "schema": "membench.vault_store_meta.v1",
        "key": key,
        "source_run": portable_path(run_root),
        "saved_at": saved_at,
        "vault_count": vault_count,
        "accuracy": read_run_accuracy(run_root),
        "moved": move_vaults,
    });
    std::fs::write(
        key_dir.join("store-meta.json"),
        serde_json::to_string_pretty(&meta)? + "\n",
    )?;

    println!(
        "saved {} vault(s) -> {} ({})",
        vault_count,
        portable_path(&dest_vaults),
        if move_vaults {
            "moved, symlink left behind"
        } else {
            "copied"
        }
    );
    Ok(())
}

/// `vault list`: table of stored vaults by scanning `$SYMEM_VAULT_STORE/*/store-meta.json`.
fn vault_list() -> anyhow::Result<()> {
    let store = require_vault_store_dir()?;
    vault_list_in(&store)
}

fn vault_list_in(store: &Path) -> anyhow::Result<()> {
    let mut keys: Vec<PathBuf> = std::fs::read_dir(store)
        .map(|entries| {
            entries
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| path.join("vaults").is_dir())
                .collect()
        })
        .unwrap_or_default();
    keys.sort();
    if keys.is_empty() {
        println!("No stored vaults found under {}", portable_path(store));
        return Ok(());
    }
    println!(
        "{:<32}  {:<25}  {:>6}  {:>7}  {:>8}",
        "KEY", "SAVED_AT", "VAULTS", "SIZE", "ACCURACY"
    );
    for key_dir in keys {
        let key = key_dir
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("?")
            .to_string();
        let vaults_dir = key_dir.join("vaults");
        let meta = read_json(&key_dir.join("store-meta.json")).ok();
        let saved_at = meta
            .as_ref()
            .and_then(|value| text_at(value, &["saved_at"]).map(ToOwned::to_owned))
            .unwrap_or_else(|| "-".to_string());
        let vault_count = meta
            .as_ref()
            .and_then(|value| nested_u64(value, &["vault_count"]))
            .unwrap_or_else(|| count_vault_subdirs(&vaults_dir) as u64);
        let accuracy = meta
            .as_ref()
            .and_then(|value| nested_f64(value, &["accuracy"]))
            .map(|value| format!("{value:.3}"))
            .unwrap_or_else(|| "n/a".to_string());
        let size = dir_size_human(&vaults_dir);
        println!("{key:<32}  {saved_at:<25}  {vault_count:>6}  {size:>7}  {accuracy:>8}");
    }
    Ok(())
}

/// `vault path <key>`: print `$SYMEM_VAULT_STORE/<key>/vaults` for scripting.
fn vault_path(key: &str) -> anyhow::Result<()> {
    let store = require_vault_store_dir()?;
    let vaults = store.join(sanitize_path_component(key)).join("vaults");
    if !vaults.is_dir() {
        anyhow::bail!("no stored vaults for key '{key}' at {}", vaults.display());
    }
    println!("{}", vaults.display());
    Ok(())
}

/// Core of [`resolve_source_vault_root`] with an explicit store dir, so by-name
/// discovery can be unit-tested without mutating process environment.
#[cfg_attr(not(feature = "symbiotic-memory-adapter"), allow(dead_code))]
fn resolve_source_vault_root_with_store(value: &Path, store: Option<&Path>) -> PathBuf {
    let direct = resolve_repo_path(value);
    if direct.is_dir() {
        return direct;
    }
    if let Some(store) = store {
        let candidate = store.join(value).join("vaults");
        if candidate.is_dir() {
            eprintln!(
                "[vault] resolved '{}' from {SYMEM_VAULT_STORE_ENV}",
                value.display()
            );
            return candidate;
        }
    }
    direct
}

/// The `EXDEV` errno ("cross-device link"), used to detect when `rename` cannot
/// move across filesystems and a copy+remove fallback is required.
fn libc_exdev() -> i32 {
    18
}

#[cfg(unix)]
fn symlink_dir(source: &Path, target: &Path) -> anyhow::Result<()> {
    std::os::unix::fs::symlink(source, target)?;
    Ok(())
}

#[cfg(not(unix))]
fn symlink_dir(_source: &Path, _target: &Path) -> anyhow::Result<()> {
    anyhow::bail!("vault save --move requires a Unix-like filesystem for the leave-behind symlink");
}

fn render_benchmark_report(report: &serde_json::Value) -> String {
    let system = text_at(report, &["system"]).unwrap_or("unknown");
    let benchmark = text_at(report, &["benchmark"]).unwrap_or("unknown");
    let run_name = text_at(report, &["run_name"]).unwrap_or("unnamed");
    let run_kind = text_at(report, &["run_kind"]).unwrap_or("unknown");
    let mut out = String::new();
    out.push_str("Benchmark Report\n");
    out.push_str("================\n");
    out.push_str(&format!("Run       : {run_name}\n"));
    out.push_str(&format!("System    : {system}\n"));
    out.push_str(&format!("Benchmark : {benchmark}\n"));
    out.push_str(&format!("Kind      : {run_kind}\n"));
    out.push('\n');
    out.push_str("Metrics\n");
    out.push_str("-------\n");
    out.push_str(&format_metric_line(
        "accuracy",
        nested_u64(report, &["metrics", "accuracy", "correct"]),
        nested_u64(report, &["metrics", "accuracy", "total"]),
        nested_f64(report, &["metrics", "accuracy", "value"]),
    ));
    if let Some(value) = nested_f64(report, &["metrics", "task_averaged_accuracy"]) {
        out.push_str(&format!("task_averaged_accuracy: {:.3}\n", value));
    }
    let abstention = format_metric_line(
        "abstention_accuracy",
        nested_u64(report, &["metrics", "abstention_accuracy", "correct"]),
        nested_u64(report, &["metrics", "abstention_accuracy", "total"]),
        nested_f64(report, &["metrics", "abstention_accuracy", "value"]),
    );
    if !abstention.ends_with("n/a\n") {
        out.push_str(&abstention);
    }
    if let Some(params) = report.get("run_params") {
        out.push('\n');
        out.push_str(&render_params_body(params));
    }
    if let Some(manifest) = report.get("artifact_manifest") {
        out.push('\n');
        out.push_str(&render_artifact_manifest(manifest));
    }
    out.push('\n');
    out.push_str("Artifacts\n");
    out.push_str("---------\n");
    if let Some(artifacts) = report.get("artifacts").and_then(|value| value.as_object()) {
        for (name, artifact) in artifacts {
            let rows = artifact
                .get("non_empty_lines")
                .and_then(|value| value.as_u64())
                .map(|value| value.to_string())
                .unwrap_or_else(|| "n/a".to_string());
            let sha = artifact
                .get("sha256")
                .and_then(|value| value.as_str())
                .map(|value| value.chars().take(12).collect::<String>())
                .unwrap_or_else(|| "n/a".to_string());
            let path = artifact
                .get("path")
                .and_then(|value| value.as_str())
                .unwrap_or("n/a");
            out.push_str(&format!("{name}: rows={rows} sha256={sha} path={path}\n"));
        }
    }
    out
}

fn render_run_params(params: &serde_json::Value) -> String {
    let mut out = String::new();
    out.push_str("Benchmark Run Params\n");
    out.push_str("====================\n");
    out.push_str(&render_params_body(params));
    out
}

fn render_params_body(params: &serde_json::Value) -> String {
    let mut out = String::new();
    out.push_str("Run Params\n");
    out.push_str("----------\n");
    for key in [
        "system",
        "benchmark",
        "run_kind",
        "run_name",
        "dataset",
        "limit",
        "distiller",
        "embedder",
        "store",
        "answer_output",
        "generative_answerer_enabled",
        "answerer",
        "configured_models",
        "runtime_models",
        "workflow",
        "transport",
        "thinking",
        "runtime_provider_note",
        "provider_queue_available",
        "workflow_queue_available",
        "ephemeral_smoke_run",
        "routed",
        "answer_only",
        "consolidate_briefs",
        "query_planner",
        "evidence_ledger",
        "answer_verifier",
        "answer_gap_retry",
        "answer_unavailable_retry",
        "score_output",
        "score",
        "scorer",
        "judge_operator",
        "judge_model",
        "judge_workers",
    ] {
        if let Some(value) = params.get(key)
            && !value.is_null()
        {
            out.push_str(&format!("{key}: {}\n", display_json_scalar(value)));
        }
    }
    if let Some(manifest) = params.get("artifact_manifest") {
        out.push('\n');
        out.push_str(&render_artifact_manifest(manifest));
    }
    out
}

fn render_artifact_manifest(manifest: &serde_json::Value) -> String {
    let mut out = String::new();
    out.push_str("Artifact Manifest\n");
    out.push_str("-----------------\n");
    out.push_str(&format!(
        "native_state_available: {}\n",
        manifest
            .get("native_state_available")
            .map(display_json_scalar)
            .unwrap_or_else(|| "unknown".to_string())
    ));
    if let Some(missing) = manifest.get("missing").and_then(|value| value.as_array()) {
        let missing = missing
            .iter()
            .filter_map(|value| value.as_str())
            .collect::<Vec<_>>();
        out.push_str(&format!(
            "missing: {}\n",
            if missing.is_empty() {
                "none".to_string()
            } else {
                missing.join(", ")
            }
        ));
    }
    if let Some(note) = manifest
        .get("native_state_note")
        .and_then(|value| value.as_str())
    {
        out.push_str(&format!("note: {note}\n"));
    }
    out
}

fn artifact_manifest_list_summary(manifest: Option<&serde_json::Value>) -> String {
    let Some(manifest) = manifest else {
        return "artifacts=unknown".to_string();
    };
    let native_state = manifest
        .get("native_state_available")
        .and_then(|value| value.as_bool())
        .map(|value| {
            if value {
                "native-state"
            } else {
                "artifact-only"
            }
        })
        .unwrap_or("state-unknown");
    let missing_count = manifest
        .get("missing")
        .and_then(|value| value.as_array())
        .map(Vec::len)
        .unwrap_or(0);
    if missing_count == 0 {
        format!("{native_state} missing=none")
    } else {
        format!("{native_state} missing={missing_count}")
    }
}

fn display_json_scalar(value: &serde_json::Value) -> String {
    value
        .as_str()
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| value.to_string())
}

fn format_metric_line(
    label: &str,
    correct: Option<u64>,
    total: Option<u64>,
    value: Option<f64>,
) -> String {
    match (correct, total, value) {
        (Some(correct), Some(total), Some(value)) => {
            format!("{label}: {correct}/{total} = {:.3}\n", value)
        }
        (_, _, Some(value)) => format!("{label}: {:.3}\n", value),
        _ => format!("{label}: n/a\n"),
    }
}

fn text_at<'a>(value: &'a serde_json::Value, path: &[&str]) -> Option<&'a str> {
    let mut cursor = value;
    for key in path {
        cursor = cursor.get(*key)?;
    }
    cursor.as_str()
}

fn nested_f64(value: &serde_json::Value, path: &[&str]) -> Option<f64> {
    let mut cursor = value;
    for key in path {
        cursor = cursor.get(*key)?;
    }
    cursor.as_f64()
}

#[cfg_attr(not(feature = "symbiotic-memory-adapter"), allow(dead_code))]
struct SymbioticMemoryCliRun {
    dataset: PathBuf,
    run_root: PathBuf,
    run_name: String,
    limit: usize,
    sample: String,
    memory_manifest: PathBuf,
    memory_config: Option<PathBuf>,
    symem_bin: Option<PathBuf>,
    distiller: String,
    embedder: String,
    store: String,
    prompt_dir: Option<PathBuf>,
    distill_prompt: String,
    answerer: bool,
    routed: bool,
    answer_only: bool,
    rejudge: bool,
    re_embed: bool,
    consolidate_briefs: bool,
    stop_after_raw_embed: bool,
    ingest_diagnostic: Option<String>,
    resume: bool,
    fresh: bool,
    query_planner: Option<String>,
    evidence_ledger: bool,
    answer_verifier: bool,
    answer_gap_retry: bool,
    answer_unavailable_retry: bool,
    score: bool,
    oracle: Option<PathBuf>,
    judge_workers: usize,
    prewarm_judge_cache: usize,
    prewarm_pause_secs: u64,
    scorer: String,
    env_file: Option<PathBuf>,
    provider_queue_dir: Option<PathBuf>,
    source_vault_root: Option<PathBuf>,
    ephemeral_smoke_run: bool,
}

fn run_symbiotic_memory_longmemeval(run: SymbioticMemoryCliRun) -> anyhow::Result<()> {
    validate_provider_role_selection(&run)?;
    #[cfg(not(feature = "symbiotic-memory-adapter"))]
    {
        let _ = run;
        anyhow::bail!(
            "native symbiotic-memory runs require the `symbiotic-memory-adapter` feature"
        );
    }
    #[cfg(feature = "symbiotic-memory-adapter")]
    {
        run_symbiotic_memory_longmemeval_native(run)
    }
}

fn validate_provider_role_selection(run: &SymbioticMemoryCliRun) -> anyhow::Result<()> {
    if run.embedder == "gemini" {
        let operator = run_env_value(run, "SYMEM_EMBED_OPERATOR");
        let model = run_env_value(run, "SYMEM_EMBED_MODEL");
        if operator.as_deref() == Some("openrouter")
            || model.as_deref().is_some_and(|model| model.contains('/'))
        {
            anyhow::bail!(
                "invalid embedding provider selection: --embedder gemini cannot use OpenRouter embedding settings (SYMEM_EMBED_OPERATOR={operator:?}, SYMEM_EMBED_MODEL={model:?}); pass --embedder openrouter for qwen embeddings"
            );
        }
    }
    Ok(())
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn run_symbiotic_memory_longmemeval_native(run: SymbioticMemoryCliRun) -> anyhow::Result<()> {
    let _paid_run_lock = if requires_paid_provider_lock(&run) {
        Some(PaidProviderRunLock::acquire(&run)?)
    } else {
        None
    };
    if run.fresh && run.run_root.exists() {
        std::fs::remove_dir_all(&run.run_root)?;
    }
    std::fs::create_dir_all(&run.run_root)?;
    if run.answer_only && !run.resume {
        clear_answer_only_run_outputs(&run.run_root)?;
    }
    let config = run
        .memory_config
        .as_ref()
        .map(symbiotic_memory::MemoryConfig::load_yaml)
        .transpose()?
        .unwrap_or_default();
    let workflow_max_in_flight =
        effective_workflow_max_in_flight_for_run(&run, Some(config.queue.workflow_max_in_flight));
    write_run_params(&run.run_root, &symbiotic_memory_run_params(&run))?;
    eprintln!(
        "[longmemeval] launch settings workflow_max_in_flight={} embed_transport={} chat_transport={} thinking={}",
        workflow_max_in_flight,
        transport_label(
            run_env_bool(
                &run,
                "SYMEM_OPENROUTER_HTTP_HTTP1_ONLY",
                run.embedder == "openrouter"
            ),
            run_env_value(&run, "SYMEM_OPENROUTER_HTTP_CLIENT_POOL_SIZE").as_deref(),
            run_env_value(&run, "SYMEM_OPENROUTER_HTTP_POOL_MAX_IDLE_PER_HOST").as_deref(),
        ),
        transport_label(
            run_env_bool(&run, "SYMEM_CHAT_HTTP_HTTP1_ONLY", false),
            run_env_value(&run, "SYMEM_CHAT_HTTP_CLIENT_POOL_SIZE").as_deref(),
            run_env_value(&run, "SYMEM_CHAT_HTTP_POOL_MAX_IDLE_PER_HOST").as_deref(),
        ),
        thinking_summary_label(&run),
    );

    let provider_runtime = ProviderRuntime::new(&run, &config)?;
    let rows = symbiotic_mem_bench::symbiotic_memory_adapter::load_longmemeval(&run.dataset, None)?;
    let rows = select_longmemeval_rows(rows, run.limit, &run.sample)?;
    // --rejudge: re-grade an existing run's stored hypotheses with the current judge, NO re-answer.
    // Reuses this run root's hypotheses.jsonl (fresh=false keeps it intact); score_prepared rewrites
    // verdicts/scored/summary, then we rewrite the report. Skips all ingest/recall/answer machinery.
    if run.rejudge {
        let hypotheses_path = native_hypotheses_path(&run.run_root);
        if !hypotheses_path.exists() {
            anyhow::bail!(
                "--rejudge: no hypotheses at {} — point --run-name at an existing answered run",
                hypotheses_path.display()
            );
        }
        eprintln!(
            "[longmemeval] --rejudge: re-grading {} stored answers (no re-answer)",
            rows.len()
        );
        let runtime = tokio::runtime::Runtime::new()?;
        let judge_factory = provider_runtime.judge_factory(&run)?;
        runtime.block_on(score_longmemeval_native(
            &run,
            &rows,
            &hypotheses_path,
            judge_factory,
        ))?;
        write_native_benchmark_report(&run)?;
        // --rejudge rewrote verdicts/score-summary, so refresh gold-eval too.
        // Defensive: a failure only logs and never fails the run.
        if let Err(e) = gold_eval(&run.run_name) {
            eprintln!("[gold-eval] auto-run skipped: {e}");
        }
        return Ok(());
    }
    // Redo stage: --re-embed is redo=embed; SYMEM_REDO=reweave|distill|index selects the others.
    let redo_stage = if run.re_embed {
        Some("embed".to_string())
    } else {
        run_env_value(&run, "SYMEM_REDO").map(|value| value.trim().to_ascii_lowercase())
    };
    if let Some(stage) = &redo_stage {
        if !matches!(stage.as_str(), "embed" | "reweave" | "distill" | "index") {
            anyhow::bail!(
                "--redo/SYMEM_REDO must be one of: embed, reweave, distill, index (got {stage})"
            );
        }
    }
    let redo_active = redo_stage.is_some();
    // Guardrail: warn loudly when a tuning knob is set while its enabling gate is OFF, so a knob
    // never silently no-ops (which previously invalidated weeks of experiments).
    warn_inert_tuning_knobs(&run, redo_stage.as_deref());
    symbiotic_mem_bench::symbiotic_memory_adapter::set_redo_stage(redo_stage);
    // Resolve the kit's typed config (SYMBIOTIC_MEMORY__* from env-file/process) and install it —
    // the adapter stamps its sections on every engine it constructs. Overridden keys are echoed so
    // an arm's config surface is visible in the run log.
    let kit_config = resolve_kit_config(&run)?;
    if !kit_config.provenance.is_empty() {
        let overridden: Vec<&str> = kit_config.provenance.keys().map(String::as_str).collect();
        eprintln!(
            "[longmemeval] kit config overrides: {} (hash {})",
            overridden.join(", "),
            &kit_config.hash[..12]
        );
    }
    symbiotic_mem_bench::symbiotic_memory_adapter::set_kit_config(kit_config.config.clone());
    if redo_active && run.answer_only {
        anyhow::bail!("a redo stage and --answer-only are mutually exclusive");
    }
    if redo_active && run.source_vault_root.is_none() {
        anyhow::bail!("a redo stage requires --source-vault-root");
    }
    if let Some(source_vault_root) = &run.source_vault_root {
        if !run.answer_only && !redo_active {
            anyhow::bail!("--source-vault-root is only valid with --answer-only or a redo stage");
        }
        if run.resume {
            anyhow::bail!(
                "--source-vault-root creates a fresh linked vault view and cannot be combined with --resume"
            );
        }
        if redo_active {
            // A redo stage COPIES memory.sqlite (re-running mutates it) and omits zvec-hybrid so the
            // index rebuilds fresh at the current embedding dimensions.
            prepare_re_embed_linked_vaults(&run.run_root, source_vault_root, &rows)?;
        } else {
            prepare_answer_only_linked_vaults(&run.run_root, source_vault_root, &rows)?;
        }
    }
    let mut policy = config.recall.clone();
    policy.answerer_enabled = run.answerer;
    // Wire the answerer system prompt: when an explicit --prompt-dir supplies an `answer` template,
    // its `system` text overrides the engine's built-in default. Absent that, leave it None so the
    // hardcoded default is used (no behavior change for runs without an answer.yaml).
    if let Some(answer_system_prompt) = load_answer_system_prompt_override(&run) {
        eprintln!(
            "[longmemeval] answerer system prompt overridden from --prompt-dir ({} chars)",
            answer_system_prompt.len()
        );
        policy.answer_system_prompt = Some(answer_system_prompt);
    }
    if let Some(query_planner) = &run.query_planner {
        policy.query_planner = match query_planner.as_str() {
            "off" => symbiotic_memory::QueryPlannerMode::Off,
            "compact" | "local" | "scripted" => symbiotic_memory::QueryPlannerMode::Compact,
            "flash" => symbiotic_memory::QueryPlannerMode::Flash,
            other => anyhow::bail!("unknown --query-planner value: {other}"),
        };
    }
    if run.evidence_ledger {
        policy.evidence_ledger = true;
    }
    if run.answer_verifier {
        policy.answer_verifier = true;
    }
    if run.answer_gap_retry {
        policy.answer_gap_retry = true;
    }
    if run.answer_unavailable_retry {
        policy.answer_unavailable_retry = true;
    }
    symbiotic_mem_bench::symbiotic_memory_adapter::clear_score_artifacts(
        &run.run_root,
        native_hypotheses_path(&run.run_root),
    )?;
    let memory_trace_writer =
        std::sync::Arc::new(symbiotic_memory::AsyncJsonlMemoryTraceSink::open(
            run.run_root.join("traces").join("memory-events.jsonl"),
        )?);
    let memory_trace_sink: Option<std::sync::Arc<dyn symbiotic_memory::MemoryTraceSink>> =
        Some(memory_trace_writer.clone());
    let runtime = tokio::runtime::Runtime::new()?;
    let hypotheses_path = native_hypotheses_path(&run.run_root);
    if let Some(parent) = hypotheses_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if matches!(run.store.as_str(), "sqlite" | "zvec" | "zvec-hybrid") {
        let zvec_marker = run.run_root.join(".store-zvec");
        let selected_backend = match run.store.as_str() {
            "sqlite" => "sqlite",
            "zvec" | "zvec-hybrid" => "zvec-hybrid",
            _ => "zvec-hybrid",
        };
        std::fs::write(&zvec_marker, format!("{selected_backend}\n"))?;
        let embedder_factory = provider_runtime.embedding_factory(&run)?;
        let distiller_factory = provider_runtime.distiller_factory(&run)?;
        let consolidator_factory = provider_runtime.consolidator_factory(&run)?;
        let answer_factory = provider_runtime.answer_factory(&run)?;
        let answer_retry_factory = provider_runtime.answer_retry_factory(&run)?;
        let planner_factory = provider_runtime.query_planner_factory(&run)?;
        let reranker = provider_runtime.reranker(&run)?;
        runtime.block_on(
            symbiotic_mem_bench::symbiotic_memory_adapter::run_longmemeval_sqlite_with_planner(
                &rows,
                &run.run_root,
                move || embedder_factory(),
                move || distiller_factory(),
                consolidator_factory,
                move || answer_factory(),
                answer_retry_factory,
                planner_factory,
                reranker,
                Some(provider_runtime.debug_metadata(&run)),
                memory_trace_sink,
                policy,
                hypotheses_path.clone(),
                run.routed,
                run.answer_only,
                run.consolidate_briefs,
                effective_stop_after_raw_embed(&run),
                adapter_ingest_diagnostic_mode(&run),
                Some(workflow_max_in_flight),
                run.resume,
            ),
        )?;
    } else if run.store == "memory" {
        if run.answer_only || run.consolidate_briefs || run.routed || run.score {
            anyhow::bail!("--store memory only supports simple unscored slice runs");
        }
        let embedder_factory = provider_runtime.embedding_factory(&run)?;
        let distiller_factory = provider_runtime.distiller_factory(&run)?;
        let answer_factory = provider_runtime.answer_factory(&run)?;
        runtime.block_on(
            symbiotic_mem_bench::symbiotic_memory_adapter::run_longmemeval_slice(
                &rows,
                symbiotic_memory::storage::InMemoryStore::default,
                move || embedder_factory(),
                move || distiller_factory(),
                move || answer_factory(),
                policy,
                hypotheses_path.clone(),
            ),
        )?;
    } else {
        anyhow::bail!("unknown --store value: {}", run.store);
    }
    if run.score {
        let judge_factory = provider_runtime.judge_factory(&run)?;
        runtime.block_on(score_longmemeval_native(
            &run,
            &rows,
            &hypotheses_path,
            judge_factory,
        ))?;
    }
    provider_runtime.flush_trace_writers(Duration::from_secs(30));
    if !memory_trace_writer.flush_blocking(Duration::from_secs(30)) {
        eprintln!("[longmemeval] timed out waiting for memory trace writer to drain");
    }
    let dropped_memory_traces = memory_trace_writer.dropped();
    if dropped_memory_traces > 0 {
        eprintln!("[longmemeval] dropped {dropped_memory_traces} memory trace events");
    }
    write_native_benchmark_report(&run)?;
    // Keep artifacts/gold-eval.json fresh after every scored run, so nobody has to
    // run `membench gold-eval --run <name>` by hand. Defensive: a failure (e.g. a
    // run with no answer_session_ids, which gold_eval bails on) only logs.
    if run.score {
        if let Err(e) = gold_eval(&run.run_name) {
            eprintln!("[gold-eval] auto-run skipped: {e}");
        }
    }
    if run.ephemeral_smoke_run {
        let root = run.run_root.clone();
        std::fs::remove_dir_all(&root)?;
        eprintln!(
            "ephemeral local smoke run succeeded; removed {}",
            root.display()
        );
    }
    Ok(())
}

#[cfg(feature = "symbiotic-memory-adapter")]
#[derive(Clone)]
struct DynDistiller(Arc<dyn symbiotic_memory::Distiller>);

#[cfg(feature = "symbiotic-memory-adapter")]
#[async_trait::async_trait]
impl symbiotic_memory::Distiller for DynDistiller {
    async fn distill(
        &self,
        source: &symbiotic_memory::SourceDocument,
        receipt: &symbiotic_memory::RawArchiveReceipt,
    ) -> anyhow::Result<Vec<symbiotic_memory::MemoryFact>> {
        self.0.distill(source, receipt).await
    }

    async fn distill_into(
        &self,
        source: &symbiotic_memory::SourceDocument,
        receipt: &symbiotic_memory::RawArchiveReceipt,
        sink: &mut dyn symbiotic_memory::ingest::DistillSink,
    ) -> anyhow::Result<()> {
        self.0.distill_into(source, receipt, sink).await
    }
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn requires_paid_provider_lock(run: &SymbioticMemoryCliRun) -> bool {
    if run.ephemeral_smoke_run {
        return false;
    }
    run.distiller == "llm"
        || matches!(run.embedder.as_str(), "gemini" | "openrouter")
        || run.score
        || run.scorer.starts_with("queued-")
}

#[cfg(feature = "symbiotic-memory-adapter")]
#[derive(Debug)]
struct PaidProviderRunLock {
    path: PathBuf,
}

#[cfg(feature = "symbiotic-memory-adapter")]
impl PaidProviderRunLock {
    fn acquire(run: &SymbioticMemoryCliRun) -> anyhow::Result<Self> {
        let lock_root = repo_root().join("runs").join(".locks");
        Self::acquire_in(lock_root, run)
    }

    fn acquire_in(lock_root: PathBuf, run: &SymbioticMemoryCliRun) -> anyhow::Result<Self> {
        std::fs::create_dir_all(&lock_root)?;
        let path = lock_root.join("paid-provider-run.lock");
        match std::fs::create_dir(&path) {
            Ok(()) => {
                let metadata = json!({
                    "schema": "membench.paid_provider_run_lock.v1",
                    "pid": std::process::id(),
                    "created_at": Utc::now().to_rfc3339(),
                    "run_name": run.run_name,
                    "run_root": portable_path(&run.run_root),
                    "system": "symbiotic-memory",
                    "benchmark": "long-mem-eval",
                    "limit": run.limit,
                    "score": run.score,
                    "answer_only": run.answer_only,
                    "distiller": run.distiller,
                    "embedder": run.embedder,
                    "scorer": run.scorer,
                });
                std::fs::write(
                    path.join("owner.json"),
                    serde_json::to_vec_pretty(&metadata)?,
                )?;
                Ok(Self { path })
            }
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                let owner = path.join("owner.json");
                if let Some(pid) = paid_provider_lock_owner_pid(&owner)?
                    && !process_is_running(pid)
                {
                    std::fs::remove_dir_all(&path).with_context(|| {
                        format!(
                            "failed to remove stale paid provider run lock at {}",
                            path.display()
                        )
                    })?;
                    return Self::acquire_in(lock_root, run);
                }
                anyhow::bail!(
                    "another paid provider-backed membench run appears to be active; refusing to start a second one. Inspect {} and remove {} only after confirming the recorded process is dead.",
                    portable_path(&owner),
                    portable_path(&path)
                )
            }
            Err(err) => Err(err).with_context(|| {
                format!(
                    "failed to acquire paid provider run lock at {}",
                    path.display()
                )
            }),
        }
    }
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn paid_provider_lock_owner_pid(owner: &Path) -> anyhow::Result<Option<u32>> {
    if !owner.exists() {
        return Ok(None);
    }
    let bytes = std::fs::read(owner).with_context(|| {
        format!(
            "failed to read paid provider run lock owner {}",
            owner.display()
        )
    })?;
    let value: Value = serde_json::from_slice(&bytes).with_context(|| {
        format!(
            "failed to parse paid provider run lock owner {}",
            owner.display()
        )
    })?;
    let schema_ok =
        value.get("schema").and_then(Value::as_str) == Some("membench.paid_provider_run_lock.v1");
    if !schema_ok {
        return Ok(None);
    }
    Ok(value
        .get("pid")
        .and_then(Value::as_u64)
        .and_then(|pid| u32::try_from(pid).ok()))
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn process_is_running(pid: u32) -> bool {
    if pid == std::process::id() {
        return true;
    }
    std::process::Command::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[cfg(feature = "symbiotic-memory-adapter")]
impl Drop for PaidProviderRunLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn clear_answer_only_run_outputs(run_root: &Path) -> anyhow::Result<()> {
    for path in [
        run_root.join("artifacts"),
        run_root.join("benchmark-report.json"),
        run_root.join("traces").join("memory-events.jsonl"),
        run_root.join("raw").join("memory-traces.jsonl"),
        run_root.join("raw").join("model-traces.jsonl"),
        run_root.join("model-traces.jsonl"),
        run_root
            .join("provider-queue")
            .join("model-queue-traces.jsonl"),
    ] {
        remove_path_if_exists(&path)?;
    }
    Ok(())
}

#[cfg(feature = "symbiotic-memory-adapter")]
/// Like `prepare_answer_only_linked_vaults`, but COPIES `memory.sqlite` (re-embedding mutates it via
/// `upsert_facts`, so symlinking would corrupt the shared source baseline) and intentionally omits
/// `zvec-hybrid` so the recall index is rebuilt fresh at the current embedding dimensions.
fn prepare_re_embed_linked_vaults(
    run_root: &Path,
    source_vault_root: &Path,
    rows: &[symbiotic_mem_bench::symbiotic_memory_adapter::LongMemEvalRecord],
) -> anyhow::Result<()> {
    let source_vault_root = resolve_source_vault_root(source_vault_root);
    if !source_vault_root.is_dir() {
        anyhow::bail!(
            "source vault root does not exist: {}",
            source_vault_root.display()
        );
    }
    let target_vault_root = run_root.join("vaults");
    std::fs::create_dir_all(&target_vault_root)?;
    for row in rows {
        let source_vault = source_vault_root.join(&row.question_id);
        let target_vault = target_vault_root.join(&row.question_id);
        let source_manifest = source_vault.join("manifest.json");
        let source_memory = source_vault.join("memory.sqlite");
        if !source_manifest.is_file() || !source_memory.is_file() {
            anyhow::bail!(
                "source vault {} is missing manifest.json or memory.sqlite",
                source_vault.display()
            );
        }
        std::fs::create_dir_all(&target_vault)?;
        remove_path_if_exists(&target_vault.join("manifest.json"))?;
        remove_path_if_exists(&target_vault.join("memory.sqlite"))?;
        remove_path_if_exists(&target_vault.join("archive"))?;
        remove_path_if_exists(&target_vault.join("zvec-hybrid"))?;
        std::fs::copy(&source_manifest, target_vault.join("manifest.json"))?;
        std::fs::copy(&source_memory, target_vault.join("memory.sqlite"))?;
        let source_archive = source_vault.join("archive");
        if source_archive.exists() {
            link_path(&source_archive, &target_vault.join("archive"))?;
        }
        // Facts-only re-embed (the default): carry the source vector index forward so the existing
        // raw-turn vectors are preserved — only fact vectors get overwritten by upsert_facts, no turn
        // re-embed. A full turn re-embed (SYMEM_REEMBED_TURNS=1) instead leaves zvec-hybrid absent so
        // the index rebuilds from scratch at the (possibly new) embedding dimensions.
        if !symbiotic_mem_bench::symbiotic_memory_adapter::reembed_turns() {
            let source_zvec = source_vault.join("zvec-hybrid");
            if source_zvec.is_dir() {
                copy_dir_recursive(&source_zvec, &target_vault.join("zvec-hybrid"))?;
            }
        }
    }
    Ok(())
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn prepare_answer_only_linked_vaults(
    run_root: &Path,
    source_vault_root: &Path,
    rows: &[symbiotic_mem_bench::symbiotic_memory_adapter::LongMemEvalRecord],
) -> anyhow::Result<()> {
    let source_vault_root = resolve_source_vault_root(source_vault_root);
    if !source_vault_root.is_dir() {
        anyhow::bail!(
            "source vault root does not exist: {}",
            source_vault_root.display()
        );
    }
    let target_vault_root = run_root.join("vaults");
    std::fs::create_dir_all(&target_vault_root)?;
    for row in rows {
        let source_vault = source_vault_root.join(&row.question_id);
        let target_vault = target_vault_root.join(&row.question_id);
        let source_manifest = source_vault.join("manifest.json");
        let source_memory = source_vault.join("memory.sqlite");
        if !source_manifest.is_file() || !source_memory.is_file() {
            anyhow::bail!(
                "source vault {} is missing manifest.json or memory.sqlite",
                source_vault.display()
            );
        }

        std::fs::create_dir_all(&target_vault)?;
        remove_path_if_exists(&target_vault.join("manifest.json"))?;
        remove_path_if_exists(&target_vault.join("memory.sqlite"))?;
        remove_path_if_exists(&target_vault.join("archive"))?;
        remove_path_if_exists(&target_vault.join("zvec-hybrid"))?;
        std::fs::copy(source_manifest, target_vault.join("manifest.json"))?;
        link_path(&source_memory, &target_vault.join("memory.sqlite"))?;
        let source_archive = source_vault.join("archive");
        if source_archive.exists() {
            link_path(&source_archive, &target_vault.join("archive"))?;
        }
        let source_zvec = source_vault.join("zvec-hybrid");
        if source_zvec.exists() {
            link_path(&source_zvec, &target_vault.join("zvec-hybrid"))?;
        }
    }
    Ok(())
}

#[cfg(feature = "symbiotic-memory-adapter")]
#[cfg(unix)]
fn link_path(source: &Path, target: &Path) -> anyhow::Result<()> {
    std::os::unix::fs::symlink(source, target)?;
    Ok(())
}

#[cfg(feature = "symbiotic-memory-adapter")]
#[cfg(not(unix))]
fn link_path(source: &Path, target: &Path) -> anyhow::Result<()> {
    if source.is_dir() {
        anyhow::bail!("directory vault links require a Unix-like filesystem");
    }
    std::fs::hard_link(source, target)?;
    Ok(())
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn remove_path_if_exists(path: &Path) -> anyhow::Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_dir() && !metadata.file_type().is_symlink() => {
            std::fs::remove_dir_all(path)?;
        }
        Ok(_) => {
            std::fs::remove_file(path)?;
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => return Err(err.into()),
    }
    Ok(())
}

#[cfg(feature = "symbiotic-memory-adapter")]
struct ProviderRuntime {
    config: symbiotic_memory::MemoryConfig,
    queue_registry: symbiotic_memory::QueueRegistry,
    queue_store: Arc<dyn symbiotic_memory::QueueEventStore>,
    queue_trace_writer: Arc<symbiotic_memory::AsyncJsonlQueueEventStore>,
    provider_queue_dir: PathBuf,
    queue_trace_path: PathBuf,
    response_cache_root: PathBuf,
    request_debug: bool,
}

#[cfg(feature = "symbiotic-memory-adapter")]
impl ProviderRuntime {
    fn new(
        run: &SymbioticMemoryCliRun,
        config: &symbiotic_memory::MemoryConfig,
    ) -> anyhow::Result<Self> {
        let provider_queue_dir = run
            .provider_queue_dir
            .clone()
            .unwrap_or_else(|| run.run_root.join("provider-queue"));
        std::fs::create_dir_all(&provider_queue_dir)?;
        let queue_trace_path = provider_queue_dir.join("model-queue-traces.jsonl");
        let queue_trace_writer = Arc::new(symbiotic_memory::AsyncJsonlQueueEventStore::open(
            &queue_trace_path,
        )?);
        let queue_store: Arc<dyn symbiotic_memory::QueueEventStore> = queue_trace_writer.clone();
        Ok(Self {
            config: config.clone(),
            queue_registry: symbiotic_memory::QueueRegistry::new(),
            queue_store,
            queue_trace_writer,
            provider_queue_dir: provider_queue_dir.clone(),
            queue_trace_path,
            response_cache_root: run_env_value(run, "SYMEM_RESPONSE_CACHE_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| provider_queue_dir.join("responses")),
            request_debug: run_env_bool(run, "SYMEM_PROVIDER_QUEUE_DEBUG_REQUESTS", false),
        })
    }

    fn flush_trace_writers(&self, timeout: Duration) {
        if !self.queue_trace_writer.flush_blocking(timeout) {
            eprintln!("[longmemeval] timed out waiting for provider queue trace writer to drain");
        }
        let dropped = self.queue_trace_writer.dropped();
        if dropped > 0 {
            eprintln!("[longmemeval] dropped {dropped} provider queue trace events");
        }
    }

    fn debug_metadata(
        &self,
        run: &SymbioticMemoryCliRun,
    ) -> symbiotic_mem_bench::symbiotic_memory_adapter::BenchDebugMetadata {
        use symbiotic_mem_bench::symbiotic_memory_adapter::{
            BenchDebugMetadata, BenchObservedCapabilities, BenchSupportedCapabilities,
            BenchTraceCapabilities,
        };

        let mut trace_artifacts = BTreeMap::new();
        trace_artifacts.insert(
            "model_traces_jsonl".to_string(),
            portable_path(&run.run_root.join("raw").join("model-traces.jsonl")),
        );
        trace_artifacts.insert(
            "model_queue_traces_jsonl".to_string(),
            portable_path(&self.queue_trace_path),
        );
        trace_artifacts.insert(
            "provider_queue_dir".to_string(),
            portable_path(&self.provider_queue_dir),
        );
        trace_artifacts.insert(
            "response_cache_dir".to_string(),
            portable_path(&self.response_cache_root),
        );

        BenchDebugMetadata {
            capabilities: BenchTraceCapabilities {
                supported: BenchSupportedCapabilities {
                    reset: true,
                    durable_state: true,
                    ingest: true,
                    flush: true,
                    retrieve: true,
                    answer: true,
                    provider_injection: true,
                    embedding_injection: true,
                    raw_context: true,
                    score_explain: true,
                    retry_trace: true,
                    token_usage: true,
                    cache_usage: true,
                    cost_usage: true,
                    queue_events: true,
                    state_export: true,
                    native_stage_trace: true,
                    wrapped_api_trace: false,
                    provider_trace: true,
                },
                observed: BenchObservedCapabilities {
                    ingest_input: true,
                    ingest_output: true,
                    model_calls: true,
                    embedding_calls: true,
                    retrieval_queries: true,
                    retrieval_candidates: true,
                    retrieval_scores: true,
                    raw_context: true,
                    answer_prompt: true,
                    answer_output: true,
                    errors: true,
                    retries: true,
                    token_usage: true,
                    cache_usage: true,
                    timing: true,
                    cost: true,
                    scoring_verdict: run.score,
                    native_stage_trace: true,
                    wrapped_api_trace: false,
                    provider_trace: true,
                    memory_stage_events: true,
                },
            },
            models: self.model_debug_rows(run),
            trace_artifacts,
            pricing_table_version: Some("memory-config-queue-pricing".to_string()),
        }
    }

    fn model_debug_rows(
        &self,
        run: &SymbioticMemoryCliRun,
    ) -> Vec<symbiotic_mem_bench::symbiotic_memory_adapter::BenchModelDebug> {
        // Show the CONSOLIDATE (reweave) binding only when the consolidator is enabled, so the
        // dashboard surfaces that the post-distill consolidation pass is running and which model /
        // thinking / queue it uses (otherwise reweave is invisible behind the shared chat queue).
        let consolidator_on = run_env_value(run, "SYMEM_CONSOLIDATOR")
            .map(|value| {
                matches!(
                    value.trim().to_ascii_lowercase().as_str(),
                    "llm" | "on" | "reweave" | "true" | "1"
                )
            })
            .unwrap_or(false);
        let roles: &[&str] = if consolidator_on {
            &[
                "DISTILL",
                "CONSOLIDATE",
                "QUERY_PLANNER",
                "ANSWER",
                "EMBED",
                "JUDGE",
            ]
        } else {
            &["DISTILL", "QUERY_PLANNER", "ANSWER", "EMBED", "JUDGE"]
        };
        roles
            .iter()
            .filter_map(|role| self.model_debug_row(run, role))
            .collect()
    }

    fn model_debug_row(
        &self,
        run: &SymbioticMemoryCliRun,
        role: &str,
    ) -> Option<symbiotic_mem_bench::symbiotic_memory_adapter::BenchModelDebug> {
        use symbiotic_mem_bench::symbiotic_memory_adapter::BenchModelDebug;
        let base = match role {
            "DISTILL" => &self.config.providers.distill,
            "CONSOLIDATE" => &self.config.providers.distill,
            "QUERY_PLANNER" => &self.config.providers.query_planner,
            "ANSWER" => &self.config.providers.answer,
            "EMBED" => &self.config.providers.embedding,
            "JUDGE" => return Some(self.judge_model_debug_row(run)),
            _ => return None,
        };
        let adapter = if role == "EMBED" {
            effective_embedding_adapter(run, base)
        } else {
            self.role_adapter(run, role, base)
        };
        let resolved = self.config.queue.resolve_provider_queue(&adapter);
        let thinking = thinking_mode(run, role).or_else(|| default_thinking_mode(role));
        let reasoning_effort = role_reasoning_effort(run, role);
        let max_tokens = run_env_value(run, &format!("SYMEM_{role}_MAX_TOKENS"))
            .and_then(|value| value.parse::<u32>().ok())
            .or_else(|| default_role_max_tokens(role));
        Some(BenchModelDebug {
            label: role.to_ascii_lowercase(),
            operation: adapter.operation,
            operator: adapter.operator,
            model: adapter.model,
            queue_id: resolved.queue_id,
            role_binding: format!(
                "{}.{}",
                if role == "JUDGE" { "bench" } else { "memory" },
                role.to_ascii_lowercase()
            ),
            max_in_flight: resolved.max_in_flight,
            lease_seconds: resolved.timeout_seconds,
            retry_attempts: resolved.retry_attempts as u32,
            logical_retry_attempts: resolved.logical_retry_attempts as u32,
            retry_jitter_seconds: 0,
            timeout_seconds: Some(resolved.timeout_seconds),
            requests_per_minute: resolved.requests_per_minute,
            input_units_per_minute: resolved.input_units_per_minute,
            response_cache_enabled: true,
            thinking: thinking_mode_label(thinking),
            reasoning_effort,
            max_tokens,
        })
    }

    fn judge_model_debug_row(
        &self,
        run: &SymbioticMemoryCliRun,
    ) -> symbiotic_mem_bench::symbiotic_memory_adapter::BenchModelDebug {
        use symbiotic_mem_bench::symbiotic_memory_adapter::BenchModelDebug;
        let judge = resolved_judge_params(run);
        let adapter =
            symbiotic_memory::ProviderAdapterConfig::new("chat", judge.operator, judge.model);
        let resolved = self.config.queue.resolve_provider_queue(&adapter);
        BenchModelDebug {
            label: "judge".to_string(),
            operation: adapter.operation,
            operator: adapter.operator,
            model: adapter.model,
            queue_id: resolved.queue_id,
            role_binding: "bench.judge".to_string(),
            max_in_flight: resolved.max_in_flight,
            lease_seconds: resolved.timeout_seconds,
            retry_attempts: resolved.retry_attempts as u32,
            logical_retry_attempts: resolved.logical_retry_attempts as u32,
            retry_jitter_seconds: 0,
            timeout_seconds: Some(resolved.timeout_seconds),
            requests_per_minute: resolved.requests_per_minute,
            input_units_per_minute: resolved.input_units_per_minute,
            response_cache_enabled: true,
            thinking: thinking_mode_label(
                thinking_mode(run, "JUDGE").or_else(|| default_thinking_mode("JUDGE")),
            ),
            reasoning_effort: role_reasoning_effort(run, "JUDGE"),
            max_tokens: run_env_value(run, "SYMEM_JUDGE_MAX_TOKENS")
                .and_then(|value| value.parse::<u32>().ok())
                .or_else(|| default_role_max_tokens("JUDGE")),
        }
    }

    fn embedding_factory(
        &self,
        run: &SymbioticMemoryCliRun,
    ) -> anyhow::Result<Arc<dyn Fn() -> Arc<dyn symbiotic_memory::EmbeddingProvider> + Send + Sync>>
    {
        match run.embedder.as_str() {
            "hash" => {
                let shared: Arc<dyn symbiotic_memory::EmbeddingProvider> =
                    Arc::new(symbiotic_memory::CachedEmbeddingProvider::new(
                        symbiotic_memory::HashEmbeddingProvider::default(),
                        "hash-membench",
                    ));
                Ok(Arc::new(move || shared.clone()))
            }
            "gemini" => {
                let adapter = self.role_adapter(run, "EMBED", &self.config.providers.embedding);
                let queue = self.provider_queue(&adapter)?;
                let api_key = required_env(run, "GEMINI_API_KEY")?;
                let model = adapter.model.clone();
                let dims = run_env_value(run, "SYMEM_EMBED_DIMS")
                    .and_then(|value| value.parse::<usize>().ok())
                    .unwrap_or(3072);
                let max_chars = run_env_value(run, "SYMEM_EMBED_MAX_CHARS")
                    .and_then(|value| value.parse::<usize>().ok())
                    .unwrap_or(32_000);
                let transport = gemini_transport_mode(run);
                let timeout = self
                    .config
                    .queue
                    .resolve_provider_queue(&adapter)
                    .timeout_seconds;
                let provider = symbiotic_memory::providers::GeminiEmbeddingProvider::new(
                    api_key,
                    model.clone(),
                    dims,
                    symbiotic_memory::providers::GeminiEmbeddingTaskMode::Document,
                )
                .with_transport_mode(transport)
                .with_timeout_secs(timeout)
                .with_max_chars(max_chars);
                let shared: Arc<dyn symbiotic_memory::EmbeddingProvider> =
                    Arc::new(symbiotic_memory::CachedEmbeddingProvider::new(
                        symbiotic_memory::providers::QueuedEmbeddingProvider::new(provider, queue),
                        format!("gemini:{model}:{dims}:document"),
                    ));
                Ok(Arc::new(move || shared.clone()))
            }
            "ollama" => {
                let adapter = effective_embedding_adapter(run, &self.config.providers.embedding);
                let model = adapter.model.clone();
                let queue = self.provider_queue(&adapter)?;
                let base_url = run_env_value(run, "SYMEM_EMBED_BASE_URL")
                    .or_else(|| run_env_value(run, "SYMEM_OLLAMA_BASE_URL"))
                    .unwrap_or_else(|| "http://127.0.0.1:11434".to_string());
                let dims = run_env_value(run, "SYMEM_EMBED_DIMS")
                    .and_then(|value| value.parse::<usize>().ok())
                    .unwrap_or(768);
                let max_chars = run_env_value(run, "SYMEM_EMBED_MAX_CHARS")
                    .and_then(|value| value.parse::<usize>().ok())
                    .unwrap_or(32_000);
                let timeout = self
                    .config
                    .queue
                    .resolve_provider_queue(&adapter)
                    .timeout_seconds;
                let provider = symbiotic_memory::providers::OllamaCompatibleEmbeddingProvider::new(
                    base_url,
                    model.clone(),
                    dims,
                )
                .with_timeout_secs(timeout)
                .with_max_chars(max_chars);
                let shared: Arc<dyn symbiotic_memory::EmbeddingProvider> =
                    Arc::new(symbiotic_memory::CachedEmbeddingProvider::new(
                        symbiotic_memory::providers::QueuedEmbeddingProvider::new(provider, queue),
                        format!("ollama:{model}:{dims}:document"),
                    ));
                Ok(Arc::new(move || shared.clone()))
            }
            "openrouter" => {
                let adapter = effective_embedding_adapter(run, &self.config.providers.embedding);
                let model = adapter.model.clone();
                let queue = self.provider_queue(&adapter)?;
                let api_key = required_env(run, "OPENROUTER_API_KEY")?;
                let base_url = run_env_value(run, "SYMEM_EMBED_BASE_URL")
                    .or_else(|| run_env_value(run, "SYMEM_OPENROUTER_BASE_URL"))
                    .unwrap_or_else(|| "https://openrouter.ai/api/v1".to_string());
                // Owner-default stack: qwen3-embedding-8b truncated to 1024 dims (the tuned
                // arm), requesting the declared width so the response matches validation and
                // the vector store schema. Mirrors the tuning scripts' profile defaults.
                let dims = run_env_value(run, "SYMEM_EMBED_DIMS")
                    .and_then(|value| value.parse::<usize>().ok())
                    .unwrap_or(1024);
                let requested_dims = run_env_value(run, "SYMEM_EMBED_REQUEST_DIMS")
                    .and_then(|value| value.parse::<usize>().ok())
                    .or(Some(dims).filter(|dims| *dims > 0));
                let max_chars = run_env_value(run, "SYMEM_EMBED_MAX_CHARS")
                    .and_then(|value| value.parse::<usize>().ok())
                    .unwrap_or(32_000);
                let timeout = self
                    .config
                    .queue
                    .resolve_provider_queue(&adapter)
                    .timeout_seconds;
                let provider = symbiotic_memory::providers::OpenRouterEmbeddingProvider::new(
                    base_url,
                    api_key,
                    model.clone(),
                    dims,
                )
                .with_timeout_secs(timeout)
                .with_requested_dims(requested_dims)
                .with_max_chars(max_chars);
                let shared: Arc<dyn symbiotic_memory::EmbeddingProvider> =
                    Arc::new(symbiotic_memory::CachedEmbeddingProvider::new(
                        symbiotic_memory::providers::QueuedEmbeddingProvider::new(provider, queue),
                        format!("openrouter:{model}:{dims}:document"),
                    ));
                Ok(Arc::new(move || shared.clone()))
            }
            other => {
                anyhow::bail!(
                    "unknown --embedder value: {other}; expected hash, gemini, ollama, or openrouter"
                )
            }
        }
    }

    fn distiller_factory(
        &self,
        run: &SymbioticMemoryCliRun,
    ) -> anyhow::Result<Arc<dyn Fn() -> DynDistiller + Send + Sync>> {
        match run.distiller.as_str() {
            "heuristic" => Ok(Arc::new(|| {
                DynDistiller(Arc::new(symbiotic_memory::PassthroughDistiller))
            })),
            "llm" => {
                let prompt = load_memory_prompt(run, &run.distill_prompt)?;
                let chat_factory =
                    self.chat_factory(run, "DISTILL", &self.config.providers.distill)?;
                let kit_distill =
                    symbiotic_mem_bench::symbiotic_memory_adapter::kit_config().distill.clone();
                // Env override still wins per run until the MEMBENCH_* sweep.
                let turns_override = run_env_value(run, "SYMEM_DISTILL_TURNS_PER_WINDOW")
                    .and_then(|value| value.parse::<usize>().ok());
                // Semantic boundaries need an embedder; the caching layer plus the
                // provider-queue response cache make these embeds free duplicates of
                // the raw-embed stage.
                let boundary_embedder_factory = if kit_distill.window_boundary == "semantic" {
                    Some(self.embedding_factory(run)?)
                } else {
                    None
                };
                Ok(Arc::new(move || {
                    let llm = symbiotic_memory::LlmDistiller::new(chat_factory(), prompt.clone())
                        .with_distill_config(kit_distill.clone());
                    let mut windowed =
                        symbiotic_memory::WindowedDistiller::new(llm, kit_distill.turns_per_window)
                            .with_distill_config(kit_distill.clone());
                    if let Some(turns) = turns_override {
                        windowed = windowed.with_turns_per_window(turns);
                    }
                    if let Some(embedder_factory) = &boundary_embedder_factory {
                        windowed = windowed.with_boundary_embedder(embedder_factory());
                    }
                    DynDistiller(Arc::new(windowed))
                }))
            }
            other => anyhow::bail!("unknown --distiller value: {other}; expected heuristic or llm"),
        }
    }

    /// Optional LLM consolidation ("reweave") pass that runs after distill+embed and synthesizes
    /// derived memory cards (itemized count/list ledgers, temporal anchors, current-state). Enabled
    /// with `SYMEM_CONSOLIDATOR=llm`. Defaults to the distill chat binding; tune via the
    /// `CONSOLIDATE` role env (e.g. `SYMEM_CONSOLIDATE_THINKING`, `SYMEM_CONSOLIDATE_MODEL`).
    fn consolidator_factory(
        &self,
        run: &SymbioticMemoryCliRun,
    ) -> anyhow::Result<Option<Arc<dyn Fn() -> Arc<dyn symbiotic_memory::Distiller> + Send + Sync>>>
    {
        let enabled = run_env_value(run, "SYMEM_CONSOLIDATOR")
            .map(|value| {
                matches!(
                    value.trim().to_ascii_lowercase().as_str(),
                    "llm" | "on" | "reweave" | "true" | "1"
                )
            })
            .unwrap_or(false);
        if !enabled {
            return Ok(None);
        }
        let prompt = load_memory_prompt(run, "reweave")?;
        let chat_factory = self.chat_factory(run, "CONSOLIDATE", &self.config.providers.distill)?;
        let kit_distill =
            symbiotic_mem_bench::symbiotic_memory_adapter::kit_config().distill.clone();
        let turns_per_window = run_env_value(run, "SYMEM_CONSOLIDATE_TURNS_PER_WINDOW")
            .and_then(|value| value.parse::<usize>().ok())
            .or_else(|| Some(kit_distill.consolidate_turns_per_window).filter(|turns| *turns > 0))
            .unwrap_or(64);
        Ok(Some(Arc::new(move || {
            let llm = symbiotic_memory::LlmDistiller::new(chat_factory(), prompt.clone())
                .with_distill_config(kit_distill.clone());
            // The reweave pass keeps its own window size — install the section
            // first, then override the pass-specific turns.
            Arc::new(
                symbiotic_memory::WindowedDistiller::new(llm, turns_per_window)
                    .with_distill_config(kit_distill.clone())
                    .with_turns_per_window(turns_per_window),
            ) as Arc<dyn symbiotic_memory::Distiller>
        })))
    }

    fn answer_factory(
        &self,
        run: &SymbioticMemoryCliRun,
    ) -> anyhow::Result<Arc<dyn Fn() -> Arc<dyn symbiotic_memory::ChatProvider> + Send + Sync>>
    {
        if !run.answerer {
            return Ok(Arc::new(|| {
                Arc::new(symbiotic_memory::providers::DisabledChatProvider)
                    as Arc<dyn symbiotic_memory::ChatProvider>
            }));
        }
        self.chat_factory(run, "ANSWER", &self.config.providers.answer)
    }

    fn answer_retry_factory(
        &self,
        run: &SymbioticMemoryCliRun,
    ) -> anyhow::Result<
        Option<Arc<dyn Fn() -> Arc<dyn symbiotic_memory::ChatProvider> + Send + Sync>>,
    > {
        if !run.answerer || !role_has_override(run, "ANSWER_RETRY") {
            return Ok(None);
        }
        Ok(Some(self.chat_factory(
            run,
            "ANSWER_RETRY",
            &self.config.providers.answer,
        )?))
    }

    /// Builds the cross-encoder reranker (ON by default — part of the owner-default stack;
    /// disable per-run with SYMEM_RERANK=0). Recall retrieves a wide embedding candidate set
    /// and the reranker re-orders it to the answer top-k, recovering evidence (e.g. itemized
    /// count ledgers) that embeds far from the question.
    fn reranker(
        &self,
        run: &SymbioticMemoryCliRun,
    ) -> anyhow::Result<symbiotic_mem_bench::symbiotic_memory_adapter::RerankCascade> {
        let enabled = run_env_value(run, "SYMEM_RERANK")
            .map(|value| {
                matches!(
                    value.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "on"
                )
            })
            .unwrap_or(true);
        if !enabled {
            return Ok(Default::default());
        }
        // Main (stage-2) reranker. SYMEM_RERANK_BASE_URL overrides the base url (e.g. point at the
        // local Nemotron /serve server at http://localhost:8088).
        let base_url = run_env_value(run, "SYMEM_RERANK_BASE_URL")
            .unwrap_or_else(|| "https://openrouter.ai/api/v1".to_string());
        // Bench default reranker: the free Nemotron VL 1B (ties Cohere at ~92% on 50Q, costs $0).
        // This is a benchmark-harness default only — the memory library imposes no reranker default.
        // Override per-run with SYMEM_RERANK_MODEL (e.g. cohere/rerank-4-fast for a paid A/B).
        let model = run_env_value(run, "SYMEM_RERANK_MODEL")
            .unwrap_or_else(|| "nvidia/llama-nemotron-rerank-vl-1b-v2:free".to_string());
        let main = Some(self.build_reranker(run, &base_url, &model)?);

        // Optional cheap stage-1 prefilter reranker. Enabled iff SYMEM_RERANK_STAGE1_MODEL is set.
        // Its base url defaults to the OpenRouter base (so a Cohere prefilter works out of the box);
        // set SYMEM_RERANK_STAGE1_BASE_URL to route stage-1 elsewhere (e.g. the local Nemotron).
        let (stage1, stage1_top_x) = match run_env_value(run, "SYMEM_RERANK_STAGE1_MODEL") {
            Some(stage1_model) if !stage1_model.trim().is_empty() => {
                let stage1_base_url = run_env_value(run, "SYMEM_RERANK_STAGE1_BASE_URL")
                    .unwrap_or_else(|| "https://openrouter.ai/api/v1".to_string());
                let stage1 = self.build_reranker(run, &stage1_base_url, stage1_model.trim())?;
                let top_x = run_env_value(run, "SYMEM_RERANK_STAGE1_TOP_X")
                    .and_then(|value| value.trim().parse::<usize>().ok())
                    .unwrap_or(20);
                (Some(stage1), top_x)
            }
            _ => (None, 20),
        };

        Ok(
            symbiotic_mem_bench::symbiotic_memory_adapter::RerankCascade {
                main,
                stage1,
                stage1_top_x,
            },
        )
    }

    /// Builds a single queued `OpenRouterReranker` (Cohere-compatible `POST {base}/rerank`). The
    /// `base_url` may point at OpenRouter or a local Cohere-compatible server (e.g. the Nemotron
    /// /serve server at http://localhost:8088, which appends `/rerank` -> `http://localhost:8088/rerank`).
    fn build_reranker(
        &self,
        run: &SymbioticMemoryCliRun,
        base_url: &str,
        model: &str,
    ) -> anyhow::Result<Arc<dyn symbiotic_memory::Reranker>> {
        // Local single-GPU MLX rerank servers (localhost) must serialize so concurrent recalls don't
        // thrash the GPU: the operator drives the queue concurrency cap ("local" -> max_in_flight=1
        // via the foundation default; otherwise the openrouter fallback at 1000). Override with
        // SYMEM_RERANK_OPERATOR.
        let is_local = base_url.contains("localhost")
            || base_url.contains("127.0.0.1")
            || base_url.contains("[::1]");
        let operator = run_env_value(run, "SYMEM_RERANK_OPERATOR")
            .unwrap_or_else(|| if is_local { "local" } else { "openrouter" }.to_string());
        // The local server ignores api_key + model; a dummy key is fine for localhost.
        let api_key = if is_local {
            "local".to_string()
        } else {
            required_operator_api_key(run, "openrouter")?
        };
        let adapter =
            symbiotic_memory::ProviderAdapterConfig::new("rerank", operator, model.to_string());
        let queue = self.provider_queue(&adapter)?;
        let inner = symbiotic_memory::OpenRouterReranker::new(
            base_url.to_string(),
            api_key,
            model.to_string(),
        );
        Ok(Arc::new(symbiotic_memory::providers::QueuedReranker::new(
            inner, queue,
        )))
    }

    fn query_planner_factory(
        &self,
        run: &SymbioticMemoryCliRun,
    ) -> anyhow::Result<
        Option<Arc<dyn Fn() -> Arc<dyn symbiotic_memory::recall::QueryPlanner> + Send + Sync>>,
    > {
        if run.query_planner.as_deref() != Some("flash") {
            return Ok(None);
        }
        let adapter = self.role_adapter(run, "QUERY_PLANNER", &self.config.providers.query_planner);
        let chat_factory = self.chat_factory(run, "QUERY_PLANNER", &adapter)?;
        Ok(Some(Arc::new(move || {
            Arc::new(symbiotic_memory::recall::ChatQueryPlanner::new(
                chat_factory(),
            )) as Arc<dyn symbiotic_memory::recall::QueryPlanner>
        })))
    }

    fn judge_factory(
        &self,
        run: &SymbioticMemoryCliRun,
    ) -> anyhow::Result<Arc<dyn Fn() -> Arc<dyn symbiotic_memory::ChatProvider> + Send + Sync>>
    {
        let judge = resolved_judge_params(run);
        let adapter =
            symbiotic_memory::ProviderAdapterConfig::new("chat", judge.operator, judge.model);
        self.chat_factory(run, "JUDGE", &adapter)
    }

    fn chat_factory(
        &self,
        run: &SymbioticMemoryCliRun,
        role: &str,
        base_adapter: &symbiotic_memory::ProviderAdapterConfig,
    ) -> anyhow::Result<Arc<dyn Fn() -> Arc<dyn symbiotic_memory::ChatProvider> + Send + Sync>>
    {
        let adapter = self.role_adapter(run, role, base_adapter);
        let queue = self.provider_queue(&adapter)?;
        let operator = adapter.operator.clone();
        let model = adapter.model.clone();
        let base_url = run_env_value(run, &format!("SYMEM_{role}_BASE_URL"))
            .or_else(|| run_env_value(run, "SYMEM_CHAT_BASE_URL"))
            .unwrap_or_else(|| match operator.as_str() {
                "deepseek" => "https://api.deepseek.com".to_string(),
                "openrouter" => "https://openrouter.ai/api/v1".to_string(),
                _ => "https://api.openai.com/v1".to_string(),
            });
        let api_key = required_operator_api_key(run, &operator)?;
        let thinking = thinking_mode(run, role).or_else(|| default_thinking_mode(role));
        let reasoning_effort = role_reasoning_effort(run, role);
        let max_tokens = run_env_value(run, &format!("SYMEM_{role}_MAX_TOKENS"))
            .and_then(|value| value.parse::<u32>().ok())
            .or_else(|| default_role_max_tokens(role));
        let timeout = self
            .config
            .queue
            .resolve_provider_queue(&adapter)
            .timeout_seconds;
        Ok(Arc::new(move || {
            let mut provider = symbiotic_memory::providers::OpenAiCompatibleChatProvider::new(
                base_url.clone(),
                api_key.clone(),
                model.clone(),
            )
            .with_timeout_secs(timeout)
            .with_thinking(thinking.clone())
            .with_max_tokens(max_tokens);
            if let Some(reasoning_effort) = reasoning_effort.clone() {
                provider = provider.with_reasoning_effort(reasoning_effort);
            }
            Arc::new(symbiotic_memory::providers::QueuedChatProvider::new(
                provider,
                queue.clone(),
            )) as Arc<dyn symbiotic_memory::ChatProvider>
        }))
    }

    fn role_adapter(
        &self,
        run: &SymbioticMemoryCliRun,
        role: &str,
        base: &symbiotic_memory::ProviderAdapterConfig,
    ) -> symbiotic_memory::ProviderAdapterConfig {
        let mut adapter = base.clone();
        if let Some(operator) = run_env_value(run, &format!("SYMEM_{role}_OPERATOR")) {
            adapter.operator = operator;
        }
        if let Some(model) = run_env_value(run, &format!("SYMEM_{role}_MODEL")) {
            adapter.model = model;
        }
        if let Some(queue_id) = run_env_value(run, &format!("SYMEM_{role}_QUEUE_ID")) {
            adapter.queue_id = Some(queue_id);
        }
        adapter
    }

    fn provider_queue(
        &self,
        adapter: &symbiotic_memory::ProviderAdapterConfig,
    ) -> anyhow::Result<symbiotic_memory::providers::ProviderQueue> {
        let resolved = self.config.queue.resolve_provider_queue(adapter);
        let settings = symbiotic_memory::QueueSettings::new(resolved.queue_id.clone())
            .with_max_in_flight(resolved.max_in_flight)
            .with_request_timeout(Some(Duration::from_secs(resolved.timeout_seconds)))
            .with_retry_attempts(resolved.retry_attempts);
        let queue = self
            .queue_registry
            .get_or_create(settings, Some(self.queue_store.clone()));
        let provider_config =
            symbiotic_memory::providers::ProviderQueueConfig::new(resolved.queue_id.clone())
                .with_max_in_flight(resolved.max_in_flight)
                .with_request_timeout(Some(Duration::from_secs(resolved.timeout_seconds)))
                .with_retry_attempts(resolved.retry_attempts)
                .with_requests_per_minute(resolved.requests_per_minute)
                .with_input_units_per_minute(resolved.input_units_per_minute)
                .with_pricing(symbiotic_memory::providers::ProviderPricing {
                    input_token_micro_usd: resolved.pricing.input_token_micro_usd,
                    cached_input_token_micro_usd: resolved.pricing.cached_input_token_micro_usd,
                    output_token_micro_usd: resolved.pricing.output_token_micro_usd,
                });
        let provider_queue =
            symbiotic_memory::providers::ProviderQueue::from_queue(provider_config, queue)
                .with_response_cache(
                    self.response_cache_root
                        .join(sanitize_path_component(&resolved.queue_id)),
                );
        if self.request_debug {
            Ok(provider_queue.with_request_debug_root(self.provider_queue_dir.join("requests")))
        } else {
            Ok(provider_queue)
        }
    }
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn load_memory_prompt(
    run: &SymbioticMemoryCliRun,
    prompt_name: &str,
) -> anyhow::Result<symbiotic_memory::PromptTemplate> {
    let prompt_dir = run.prompt_dir.clone().unwrap_or_else(|| {
        let manifest = resolve_repo_path(&run.memory_manifest);
        manifest
            .parent()
            .unwrap_or_else(|| Path::new("../symbiotic-memory"))
            .join("prompts")
    });
    let catalog = symbiotic_memory::PromptCatalog::load_dir(resolve_repo_path(&prompt_dir))?;
    catalog.get(prompt_name).cloned().ok_or_else(|| {
        anyhow::anyhow!(
            "prompt `{prompt_name}` not found in {}",
            prompt_dir.display()
        )
    })
}

/// Loads the answerer system-prompt override from an explicitly-provided `--prompt-dir`.
///
/// Returns `Some(system)` only when the operator passed `--prompt-dir` AND that dir contains an
/// `answer` template; in every other case it returns `None` so the engine keeps using its built-in
/// hardcoded answerer system prompt (no behavior change for runs without an `answer.yaml`). The
/// crate's default `prompts/` directory is intentionally NOT consulted here — only the explicit
/// `--prompt-dir` can override the answerer system prompt.
#[cfg(feature = "symbiotic-memory-adapter")]
fn load_answer_system_prompt_override(run: &SymbioticMemoryCliRun) -> Option<String> {
    // Only an explicit --prompt-dir can supply the override.
    let prompt_dir = run.prompt_dir.as_ref()?;
    let catalog = symbiotic_memory::PromptCatalog::load_dir(resolve_repo_path(prompt_dir)).ok()?;
    let system = catalog.get("answer")?.system.trim();
    if system.is_empty() {
        return None;
    }
    Some(system.to_string())
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn required_env(run: &SymbioticMemoryCliRun, key: &str) -> anyhow::Result<String> {
    run_env_value(run, key)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("{key} is required for paid provider-backed runs"))
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn required_operator_api_key(
    run: &SymbioticMemoryCliRun,
    operator: &str,
) -> anyhow::Result<String> {
    let key = match operator {
        "deepseek" => "DEEPSEEK_API_KEY",
        "openai" => "OPENAI_API_KEY",
        "gemini" => "GEMINI_API_KEY",
        "openrouter" => "OPENROUTER_API_KEY",
        other => anyhow::bail!("unsupported chat operator `{other}`"),
    };
    required_env(run, key)
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn gemini_transport_mode(
    run: &SymbioticMemoryCliRun,
) -> symbiotic_memory::providers::GeminiEmbeddingTransportMode {
    let value = run_env_value(run, "SYMEM_GEMINI_EMBED_MODE")
        .or_else(|| run_env_value(run, "SYMEM_GEMINI_EMBED_TRANSPORT"))
        .unwrap_or_else(|| "multi-input".to_string())
        .to_ascii_lowercase();
    match value.as_str() {
        "single-request" | "single" | "embed-content" | "embedcontent" => {
            symbiotic_memory::providers::GeminiEmbeddingTransportMode::SingleRequest
        }
        _ => symbiotic_memory::providers::GeminiEmbeddingTransportMode::MultiInputRequest,
    }
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn thinking_mode(
    run: &SymbioticMemoryCliRun,
    role: &str,
) -> Option<symbiotic_memory::providers::ThinkingMode> {
    let value = run_env_value(run, &format!("SYMEM_{role}_THINKING"))
        .or_else(|| run_env_value(run, &format!("SYMEM_{role}_REASONING")))
        .unwrap_or_default()
        .to_ascii_lowercase();
    match value.as_str() {
        "off" | "disabled" | "disable" | "false" | "0" => {
            Some(symbiotic_memory::providers::ThinkingMode::Disabled)
        }
        "on" | "enabled" | "enable" | "true" | "1" | "high" | "max" => {
            Some(symbiotic_memory::providers::ThinkingMode::Enabled)
        }
        _ => None,
    }
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn default_thinking_mode(role: &str) -> Option<symbiotic_memory::providers::ThinkingMode> {
    match role {
        // The default LongMemEval answer pass favors short grounded answers.
        // Keep DeepSeek thinking disabled unless an experiment explicitly opts in.
        "ANSWER" | "QUERY_PLANNER" => Some(symbiotic_memory::providers::ThinkingMode::Disabled),
        // Judging is a strict YES/NO classification task. Keeping thinking off
        // avoids long hidden generations and makes cache/cost behavior stable.
        "JUDGE" => Some(symbiotic_memory::providers::ThinkingMode::Disabled),
        _ => None,
    }
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn role_has_override(run: &SymbioticMemoryCliRun, role: &str) -> bool {
    [
        "OPERATOR",
        "MODEL",
        "BASE_URL",
        "THINKING",
        "REASONING",
        "REASONING_EFFORT",
        "MAX_TOKENS",
    ]
    .iter()
    .any(|suffix| run_env_value(run, &format!("SYMEM_{role}_{suffix}")).is_some())
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn thinking_mode_label(mode: Option<symbiotic_memory::providers::ThinkingMode>) -> Option<String> {
    mode.map(|mode| match mode {
        symbiotic_memory::providers::ThinkingMode::Enabled => "enabled".to_string(),
        symbiotic_memory::providers::ThinkingMode::Disabled => "disabled".to_string(),
    })
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn default_role_max_tokens(role: &str) -> Option<u32> {
    match role {
        "JUDGE" => Some(64),
        "QUERY_PLANNER" => Some(512),
        _ => None,
    }
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn role_reasoning_effort(run: &SymbioticMemoryCliRun, role: &str) -> Option<String> {
    run_env_value(run, &format!("SYMEM_{role}_REASONING_EFFORT")).or_else(|| {
        let value = run_env_value(run, &format!("SYMEM_{role}_THINKING"))?.to_ascii_lowercase();
        matches!(value.as_str(), "high" | "max").then_some(value)
    })
}

#[cfg(feature = "symbiotic-memory-adapter")]
#[derive(Clone)]
struct ScoredHypothesis {
    question_id: String,
    question_type: Option<String>,
    question: String,
    gold_answer: Option<Value>,
    hypothesis: String,
}

#[cfg(feature = "symbiotic-memory-adapter")]
#[derive(Clone, serde::Serialize)]
struct NativeVerdict {
    question_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    question_type: Option<String>,
    question: String,
    answer: String,
    hypothesis: String,
    judge_raw: String,
    /// The exact judge SYSTEM prompt that was sent for this question (the per-question-type grader
    /// under the official mode). Lets a verdict PROVE which prompt graded it, not just infer it from
    /// the mode flag. Backward-compatible: absent on runs scored before this field existed.
    #[serde(skip_serializing_if = "Option::is_none")]
    judge_system_prompt: Option<String>,
    /// The exact judge USER message that was sent (question + reference answer / rubric + the model's
    /// response, fully rendered). With judge_system_prompt this is the COMPLETE judge input — the full
    /// thing that went to the grader, mirroring the answerer call trace.
    #[serde(skip_serializing_if = "Option::is_none")]
    judge_user_prompt: Option<String>,
    autoeval_label: NativeAutoEvalLabel,
    label: bool,
    is_abstention: bool,
    error: Option<String>,
}

#[cfg(feature = "symbiotic-memory-adapter")]
#[derive(Clone, serde::Serialize)]
struct NativeAutoEvalLabel {
    model: String,
    label: bool,
}

#[cfg(feature = "symbiotic-memory-adapter")]
async fn score_longmemeval_native(
    run: &SymbioticMemoryCliRun,
    rows: &[symbiotic_mem_bench::symbiotic_memory_adapter::LongMemEvalRecord],
    hypotheses_path: &Path,
    judge_factory: Arc<dyn Fn() -> Arc<dyn symbiotic_memory::ChatProvider> + Send + Sync>,
) -> anyhow::Result<()> {
    let oracle = run.oracle.as_deref().unwrap_or(&run.dataset);
    let oracle_rows =
        symbiotic_mem_bench::symbiotic_memory_adapter::load_longmemeval(oracle, None)?;
    let oracle_by_id = oracle_rows
        .into_iter()
        .map(|row| (row.question_id.clone(), row))
        .collect::<BTreeMap<_, _>>();
    let selected_ids = rows
        .iter()
        .map(|row| row.question_id.clone())
        .collect::<BTreeSet<_>>();
    let hypotheses = read_native_hypotheses(hypotheses_path)?;
    let mut scored = Vec::new();
    for hypothesis in hypotheses {
        if !selected_ids.contains(&hypothesis.question_id) {
            continue;
        }
        let oracle = oracle_by_id.get(&hypothesis.question_id).ok_or_else(|| {
            anyhow::anyhow!("oracle missing question_id {}", hypothesis.question_id)
        })?;
        scored.push(ScoredHypothesis {
            question_id: hypothesis.question_id,
            question_type: hypothesis
                .question_type
                .or_else(|| oracle.question_type.clone()),
            question: hypothesis.question,
            gold_answer: oracle.answer.clone(),
            hypothesis: hypothesis.hypothesis,
        });
    }
    if scored.len() != rows.len() {
        anyhow::bail!(
            "cannot score: found {} hypotheses for {} selected rows",
            scored.len(),
            rows.len()
        );
    }

    if run.prewarm_judge_cache > 0 {
        let prewarm_count = run.prewarm_judge_cache.min(scored.len());
        let prewarm_dir = native_raw_dir(&run.run_root).join("judge-cache-prewarm");
        score_prepared_longmemeval_native(
            run,
            &scored[..prewarm_count],
            hypotheses_path,
            &prewarm_dir,
            judge_factory.clone(),
            "prewarm",
        )
        .await?;
        if run.prewarm_pause_secs > 0 {
            eprintln!(
                "[longmemeval:score:prewarm] sleeping {}s before full score",
                run.prewarm_pause_secs
            );
            tokio::time::sleep(std::time::Duration::from_secs(run.prewarm_pause_secs)).await;
        }
    }

    score_prepared_longmemeval_native(
        run,
        &scored,
        hypotheses_path,
        &native_raw_dir(&run.run_root),
        judge_factory,
        "score",
    )
    .await
}

#[cfg(feature = "symbiotic-memory-adapter")]
async fn score_prepared_longmemeval_native(
    run: &SymbioticMemoryCliRun,
    scored: &[ScoredHypothesis],
    hypotheses_path: &Path,
    raw_dir: &Path,
    judge_factory: Arc<dyn Fn() -> Arc<dyn symbiotic_memory::ChatProvider> + Send + Sync>,
    stage_label: &str,
) -> anyhow::Result<()> {
    std::fs::create_dir_all(&raw_dir)?;
    let partial_path = raw_dir.join("partial-verdicts.jsonl");
    let verdicts_path = raw_dir.join("verdicts.jsonl");
    let scored_path = raw_dir.join("scored.json");
    let summary_path = raw_dir.join("score-summary.json");
    for path in [&partial_path, &verdicts_path] {
        if path.exists() {
            std::fs::remove_file(path)?;
        }
    }

    let judge = resolved_judge_params(run);
    let judge_model = judge.model;
    let judge_prompt_mode =
        run_env_value(run, "SYMEM_JUDGE_PROMPT_MODE").unwrap_or_else(|| "official".to_string());
    let partial_file = Arc::new(std::sync::Mutex::new(
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&partial_path)?,
    ));
    let total = scored.len();
    let judge_workers = run.judge_workers.max(1);
    eprintln!(
        "[longmemeval:{stage_label}] pending={} judge_model={} workers={} prompt_mode={}",
        total, judge_model, judge_workers, judge_prompt_mode
    );
    let started = std::time::Instant::now();
    let stream = futures::stream::iter(scored.iter().cloned().enumerate().map(|(idx, item)| {
        let judge_factory = judge_factory.clone();
        let partial_file = partial_file.clone();
        let judge_model = judge_model.clone();
        let judge_prompt_mode = judge_prompt_mode.clone();
        async move {
            let verdict =
                judge_one_longmemeval(item, judge_factory(), &judge_model, &judge_prompt_mode)
                    .await;
            match &verdict {
                Ok(verdict) => {
                    let line = serde_json::to_string(verdict)?;
                    let mut file = partial_file.lock().expect("partial verdict file lock");
                    use std::io::Write;
                    writeln!(file, "{line}")?;
                    file.flush()?;
                    eprintln!(
                        "[longmemeval:{stage_label}] {}/{} {} label={}",
                        idx + 1,
                        total,
                        verdict.question_id,
                        verdict.label
                    );
                }
                Err(err) => {
                    eprintln!(
                        "[longmemeval:{stage_label}] {}/{} error={err}",
                        idx + 1,
                        total
                    );
                }
            }
            verdict
        }
    }))
    .buffer_unordered(judge_workers);
    let verdict_results = stream.collect::<Vec<_>>().await;
    let mut verdicts = Vec::new();
    let mut judge_errors = 0u64;
    for result in verdict_results {
        match result {
            Ok(verdict) => verdicts.push(verdict),
            Err(err) => {
                judge_errors += 1;
                eprintln!("[longmemeval:{stage_label}] terminal-error={err}");
            }
        }
    }
    verdicts.sort_by(|a, b| a.question_id.cmp(&b.question_id));
    let mut verdict_lines = String::new();
    for verdict in &verdicts {
        verdict_lines.push_str(&serde_json::to_string(verdict)?);
        verdict_lines.push('\n');
    }
    std::fs::write(&verdicts_path, verdict_lines)?;
    let total_correct = verdicts.iter().filter(|verdict| verdict.label).count() as u64;
    let scored_count = verdicts.len() as u64;
    let mut per_type: BTreeMap<String, (u64, u64)> = BTreeMap::new();
    for verdict in &verdicts {
        let key = verdict
            .question_type
            .clone()
            .unwrap_or_else(|| "unknown".to_string());
        let entry = per_type.entry(key).or_insert((0, 0));
        entry.1 += 1;
        if verdict.label {
            entry.0 += 1;
        }
    }
    let per_question_type = per_type
        .into_iter()
        .map(|(question_type, (correct, total))| {
            (
                question_type,
                json!({
                    "correct": correct,
                    "total": total,
                    "accuracy": if total > 0 { Some(correct as f64 / total as f64) } else { None },
                }),
            )
        })
        .collect::<serde_json::Map<_, _>>();
    let scored_json = json!({
        "judge_model": judge_model,
        "judge_prompt_mode": judge_prompt_mode,
        "overall_accuracy": if scored_count > 0 { Some(total_correct as f64 / scored_count as f64) } else { None },
        "task_averaged_accuracy": task_averaged_accuracy(&per_question_type),
        "counts": {
            "scored": scored_count,
            "total_correct": total_correct,
            "judge_errors": judge_errors,
            "abstention_correct": verdicts.iter().filter(|v| v.is_abstention && v.label).count() as u64,
            "abstention_total": verdicts.iter().filter(|v| v.is_abstention).count() as u64,
        },
        "per_question_type": per_question_type,
    });
    std::fs::write(
        &scored_path,
        serde_json::to_string_pretty(&scored_json)? + "\n",
    )?;
    std::fs::write(
        &summary_path,
        serde_json::to_string_pretty(&json!({
            "schema_version": 1,
            "scorer": run.scorer,
            "judge_model": scored_json["judge_model"],
            "judge_prompt_mode": scored_json["judge_prompt_mode"],
            "hypotheses_file": portable_path(hypotheses_path),
            "verdicts_file": portable_path(&verdicts_path),
            "scored_file": portable_path(&scored_path),
            "elapsed_ms": started.elapsed().as_millis() as u64,
            "metrics": scored_json,
        }))? + "\n",
    )?;
    if judge_errors > 0 || scored_count != total as u64 {
        anyhow::bail!(
            "LongMemEval scoring failed: verdicts={} expected={} judge_errors={}",
            scored_count,
            total,
            judge_errors
        );
    }
    Ok(())
}

#[cfg(feature = "symbiotic-memory-adapter")]
async fn judge_one_longmemeval(
    item: ScoredHypothesis,
    judge: Arc<dyn symbiotic_memory::ChatProvider>,
    judge_model: &str,
    prompt_mode: &str,
) -> anyhow::Result<NativeVerdict> {
    let answer = gold_answer_to_string(item.gold_answer.as_ref());
    let is_abstention = is_abstention(&item.hypothesis);
    let (system, user) = judge_prompt(
        prompt_mode,
        item.question_type.as_deref(),
        &item.question,
        &answer,
        &item.hypothesis,
    );
    let response = judge.chat(&system, &user).await?;
    let label = parse_judge_label(&response.text)
        .ok_or_else(|| anyhow::anyhow!("judge returned unparsable label: {}", response.text))?;
    Ok(NativeVerdict {
        question_id: item.question_id,
        question_type: item.question_type,
        question: item.question,
        answer,
        hypothesis: item.hypothesis,
        judge_raw: response.text,
        judge_system_prompt: Some(system),
        judge_user_prompt: Some(user),
        autoeval_label: NativeAutoEvalLabel {
            model: judge_model.to_string(),
            label,
        },
        label,
        is_abstention,
        error: None,
    })
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn judge_prompt(
    prompt_mode: &str,
    question_type: Option<&str>,
    question: &str,
    answer: &str,
    hypothesis: &str,
) -> (String, String) {
    // DEFAULT → the EXACT per-question-type grader from the LongMemEval paper (Wu et al.,
    // arXiv:2410.10813). Each type carries its own leniency: off-by-one for temporal, old+new for
    // knowledge-update, rubric-satisfaction for preference. The old generic semantic grader stays
    // available behind SYMEM_JUDGE_PROMPT_MODE=semantic (aka legacy / semantic-shared-compact).
    if !matches!(
        prompt_mode,
        "semantic" | "semantic-shared-compact" | "legacy" | "generic"
    ) {
        let qtype = question_type.unwrap_or("");
        let system = match qtype {
            "temporal-reasoning" => {
                "I will give you a question, a correct answer, and a response from a model. Please answer yes if the response contains the correct answer. Otherwise, answer no. If the response is equivalent to the correct answer or contains all the intermediate steps to get the correct answer, you should also answer yes. If the response only contains a subset of the information required by the answer, answer no. In addition, do not penalize off-by-one errors for the number of days. If the question asks for the number of days/weeks/months, etc., and the model makes off-by-one errors (e.g., predicting 19 days when the answer is 18), the model's response is still correct."
            }
            "knowledge-update" => {
                "I will give you a question, a correct answer, and a response from a model. Please answer yes if the response contains the correct answer. Otherwise, answer no. If the response contains some previous information along with an updated answer, the response should be considered as correct as long as the updated answer is the required answer."
            }
            "single-session-preference" => {
                "I will give you a question, a rubric for desired personalized response, and a response from a model. Please answer yes if the response satisfies the desired response. Otherwise, answer no. The model does not need to reflect all the points in the rubric. The response is correct as long as it recalls and utilizes the user's personal information correctly."
            }
            _ => {
                "I will give you a question, a correct answer, and a response from a model. Please answer yes if the response contains the correct answer. Otherwise, answer no. If the response is equivalent to the correct answer or contains all the intermediate steps to get the correct answer, you should also answer yes. If the response only contains a subset of the information required by the answer, answer no."
            }
        };
        let ref_label = if qtype == "single-session-preference" {
            "Rubric for desired personalized response"
        } else {
            "Correct answer"
        };
        let user = format!(
            "Question:\n{question}\n\n{ref_label}:\n{answer}\n\nResponse from the model:\n{hypothesis}\n\nAnswer yes or no."
        );
        return (system.to_string(), user);
    }
    let system = "You are an evaluator for LongMemEval. Return exactly YES or NO. Mark YES when the candidate answer is semantically equivalent to the reference, is a directly inferable phrasing, or contains the required value with harmless extra words. Mark NO when it contradicts the reference, omits the requested value, substitutes a different value, or says unavailable when the reference contains the answer.";
    let user = format!(
        "Question:\n{question}\n\nReference answer:\n{answer}\n\nCandidate answer:\n{hypothesis}\n\nCorrect? Return exactly YES or NO."
    );
    (system.to_string(), user)
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn parse_judge_label(text: &str) -> Option<bool> {
    let normalized = text
        .trim()
        .trim_matches(|ch: char| !ch.is_ascii_alphabetic())
        .to_ascii_lowercase();
    if normalized.starts_with("yes") || normalized == "true" || normalized == "correct" {
        Some(true)
    } else if normalized.starts_with("no") || normalized == "false" || normalized == "incorrect" {
        Some(false)
    } else {
        None
    }
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn gold_answer_to_string(answer: Option<&Value>) -> String {
    match answer {
        Some(Value::String(value)) => value.clone(),
        Some(Value::Null) | None => String::new(),
        Some(value) => serde_json::to_string(value).unwrap_or_else(|_| value.to_string()),
    }
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn is_abstention(text: &str) -> bool {
    let text = text.to_ascii_lowercase();
    text.contains("unavailable")
        || text.contains("not enough information")
        || text.contains("don't know")
        || text.contains("do not know")
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn task_averaged_accuracy(per_question_type: &serde_json::Map<String, Value>) -> Option<f64> {
    let mut values = Vec::new();
    for value in per_question_type.values() {
        if let Some(accuracy) = value.get("accuracy").and_then(Value::as_f64) {
            values.push(accuracy);
        }
    }
    (!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64)
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn read_native_hypotheses(
    path: &Path,
) -> anyhow::Result<Vec<symbiotic_mem_bench::symbiotic_memory_adapter::BenchHypothesis>> {
    let values = read_jsonl_values(path, None)?;
    values
        .into_iter()
        .map(serde_json::from_value)
        .collect::<Result<Vec<_>, _>>()
        .map_err(Into::into)
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn select_longmemeval_rows(
    rows: Vec<symbiotic_mem_bench::symbiotic_memory_adapter::LongMemEvalRecord>,
    limit: usize,
    sample: &str,
) -> anyhow::Result<Vec<symbiotic_mem_bench::symbiotic_memory_adapter::LongMemEvalRecord>> {
    if limit >= rows.len() {
        return Ok(rows);
    }
    match sample {
        "first" => Ok(rows.into_iter().take(limit).collect()),
        "stratified" => {
            let mut type_order = Vec::new();
            let mut groups: BTreeMap<String, VecDeque<_>> = BTreeMap::new();
            for row in rows {
                let question_type = row
                    .question_type
                    .clone()
                    .unwrap_or_else(|| "unknown".to_string());
                if !groups.contains_key(&question_type) {
                    type_order.push(question_type.clone());
                }
                groups.entry(question_type).or_default().push_back(row);
            }

            let mut selected = Vec::new();
            while selected.len() < limit && groups.values().any(|group| !group.is_empty()) {
                for question_type in &type_order {
                    if selected.len() >= limit {
                        break;
                    }
                    if let Some(row) = groups.get_mut(question_type).and_then(VecDeque::pop_front) {
                        selected.push(row);
                    }
                }
            }
            Ok(selected)
        }
        other => anyhow::bail!("unknown --sample value: {other}; expected first or stratified"),
    }
}

#[allow(dead_code)]
struct JudgeCachePrewarmFiles {
    hypotheses: PathBuf,
    oracle: PathBuf,
    trace_dir: PathBuf,
    count: usize,
}

#[allow(dead_code)]
fn prepare_judge_cache_prewarm(
    run: &SymbioticMemoryCliRun,
    oracle: &Path,
) -> anyhow::Result<JudgeCachePrewarmFiles> {
    let hypotheses_path = native_hypotheses_path(&run.run_root);
    let hypotheses = read_jsonl_values(&hypotheses_path, Some(run.prewarm_judge_cache))?;
    if hypotheses.is_empty() {
        anyhow::bail!(
            "cannot prewarm judge cache: no hypotheses found at {}",
            hypotheses_path.display()
        );
    }
    let wanted_ids = hypotheses
        .iter()
        .filter_map(|value| {
            value
                .get("question_id")
                .and_then(|question_id| question_id.as_str())
                .map(ToOwned::to_owned)
        })
        .collect::<BTreeSet<_>>();
    if wanted_ids.is_empty() {
        anyhow::bail!("cannot prewarm judge cache: hypotheses have no question_id fields");
    }

    let oracle_rows = read_json(oracle)?;
    let Some(rows) = oracle_rows.as_array() else {
        anyhow::bail!("cannot prewarm judge cache: oracle must be a JSON array");
    };
    let subset = rows
        .iter()
        .filter(|row| {
            row.get("question_id")
                .and_then(|question_id| question_id.as_str())
                .is_some_and(|question_id| wanted_ids.contains(question_id))
        })
        .cloned()
        .collect::<Vec<_>>();
    if subset.len() != wanted_ids.len() {
        anyhow::bail!(
            "cannot prewarm judge cache: matched {} oracle rows for {} hypotheses",
            subset.len(),
            wanted_ids.len()
        );
    }

    let dir = run.run_root.join("raw").join("judge-cache-prewarm");
    std::fs::create_dir_all(&dir)?;
    let hypotheses_out = dir.join("hypotheses.jsonl");
    let oracle_out = dir.join("oracle.json");
    let mut hyp_lines = String::new();
    for value in &hypotheses {
        hyp_lines.push_str(&serde_json::to_string(value)?);
        hyp_lines.push('\n');
    }
    std::fs::write(&hypotheses_out, hyp_lines)?;
    std::fs::write(&oracle_out, serde_json::to_string_pretty(&subset)? + "\n")?;

    Ok(JudgeCachePrewarmFiles {
        hypotheses: hypotheses_out,
        oracle: oracle_out,
        trace_dir: dir,
        count: hypotheses.len(),
    })
}

fn write_run_params(run_root: &PathBuf, params: &serde_json::Value) -> anyhow::Result<()> {
    std::fs::create_dir_all(run_root)?;
    std::fs::write(
        run_root.join("run-params.json"),
        serde_json::to_string_pretty(params)? + "\n",
    )?;
    Ok(())
}

fn imported_run_params(import: &ImportedBenchmarkReport, limit: Option<u64>) -> serde_json::Value {
    json!({
        "schema": "membench.run_params.v1",
        "system": import.system,
        "benchmark": import.benchmark,
        "run_kind": "imported-artifact",
        "run_name": import.run_name,
        "run_root": portable_path(&import.run_root),
        "limit": limit,
        "imported_artifacts": {
            "hypotheses": true,
            "provenance": import.provenance.is_some(),
            "verdicts": import.verdicts.is_some(),
            "partial_verdicts": import.partial_verdicts.is_some(),
            "memory_traces": import.memory_traces.is_some(),
            "model_traces": import.model_traces.is_some(),
            "scored": true,
        },
        "artifact_manifest": imported_artifact_manifest(import),
    })
}

fn imported_artifact_manifest(import: &ImportedBenchmarkReport) -> serde_json::Value {
    let mut available = vec!["hypotheses", "scored"];
    if import.provenance.is_some() {
        available.push("provenance");
    }
    if import.verdicts.is_some() {
        available.push("verdicts");
    }
    if import.partial_verdicts.is_some() {
        available.push("partial_verdicts");
    }
    if import.memory_traces.is_some() {
        available.push("memory_traces");
    }
    if import.model_traces.is_some() {
        available.push("model_traces");
    }
    artifact_manifest(
        available,
        false,
        "Imported artifact runs preserve copied benchmark artifacts only; native state folders such as raw, vaults, workflow, and provider-queue may be absent.",
    )
}

fn artifact_manifest<'a>(
    available: impl IntoIterator<Item = &'a str>,
    native_state_available: bool,
    native_state_note: &str,
) -> serde_json::Value {
    let available: BTreeSet<String> = available.into_iter().map(ToOwned::to_owned).collect();
    let missing: Vec<String> = COMMON_ARTIFACT_KINDS
        .iter()
        .filter(|kind| !available.contains(**kind))
        .map(|kind| (*kind).to_string())
        .collect();
    json!({
        "available": available.into_iter().collect::<Vec<_>>(),
        "missing": missing,
        "native_state_available": native_state_available,
        "native_state_note": native_state_note,
    })
}

const DEFAULT_WORKFLOW_MAX_IN_FLIGHT: usize = 50;
const ANSWER_ONLY_WORKFLOW_MAX_IN_FLIGHT: usize = 64;

#[cfg_attr(not(feature = "symbiotic-memory-adapter"), allow(dead_code))]
#[cfg(test)]
fn effective_workflow_max_in_flight(answer_only: bool, configured: Option<usize>) -> usize {
    if answer_only {
        ANSWER_ONLY_WORKFLOW_MAX_IN_FLIGHT
    } else {
        configured.unwrap_or(DEFAULT_WORKFLOW_MAX_IN_FLIGHT).max(1)
    }
}

#[cfg_attr(not(feature = "symbiotic-memory-adapter"), allow(dead_code))]
fn workflow_max_in_flight_env_override(run: &SymbioticMemoryCliRun) -> Option<usize> {
    run_env_value(run, "SYMEM_WORKFLOW_MAX_IN_FLIGHT").and_then(|value| value.parse().ok())
}

#[cfg_attr(not(feature = "symbiotic-memory-adapter"), allow(dead_code))]
fn effective_workflow_max_in_flight_for_run(
    run: &SymbioticMemoryCliRun,
    configured: Option<usize>,
) -> usize {
    // answer-only reuses a stored vault (no ingest) so it can run wide. 64 is a validated default
    // (122Q at 732 q/min, zero rerank throttle after the rpm-bucket fix); 500 still stresses the
    // SQLite workflow queue's claim/reclaim, so cap here. Overridable via SYMEM_WORKFLOW_MAX_IN_FLIGHT.
    let base_default = if run.answer_only {
        ANSWER_ONLY_WORKFLOW_MAX_IN_FLIGHT
    } else {
        DEFAULT_WORKFLOW_MAX_IN_FLIGHT
    };
    workflow_max_in_flight_env_override(run)
        .or(if run.answer_only { None } else { configured })
        .unwrap_or(base_default)
        .max(1)
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn configured_workflow_max_in_flight(run: &SymbioticMemoryCliRun) -> Option<usize> {
    let path = run.memory_config.as_ref()?;
    symbiotic_memory::MemoryConfig::load_yaml(path)
        .ok()
        .map(|config| config.queue.workflow_max_in_flight)
}

#[cfg(not(feature = "symbiotic-memory-adapter"))]
fn configured_workflow_max_in_flight(_run: &SymbioticMemoryCliRun) -> Option<usize> {
    None
}

/// The resolved kit-config identity of this run: content hash + which dotted
/// paths were overridden (and from where). Reproducibility receipt — two runs
/// with equal hashes ran the exact same kit configuration.
#[cfg(feature = "symbiotic-memory-adapter")]
fn kit_config_record(run: &SymbioticMemoryCliRun) -> serde_json::Value {
    match resolve_kit_config(run) {
        Ok(resolved) => json!({
            "hash": resolved.hash,
            "overrides": resolved
                .provenance
                .iter()
                .map(|(path, source)| json!({ "path": path, "source": format!("{source:?}") }))
                .collect::<Vec<_>>(),
        }),
        Err(err) => json!({ "error": err.to_string() }),
    }
}

#[cfg(not(feature = "symbiotic-memory-adapter"))]
fn kit_config_record(_run: &SymbioticMemoryCliRun) -> serde_json::Value {
    serde_json::Value::Null
}

#[cfg_attr(not(feature = "symbiotic-memory-adapter"), allow(dead_code))]
fn symbiotic_memory_run_params(run: &SymbioticMemoryCliRun) -> serde_json::Value {
    let judge = resolved_judge_params(run);
    let configured_models = configured_provider_models(run);
    let runtime_models = runtime_provider_bindings(run, &judge);
    let role_settings = resolved_role_settings(run, &judge);
    let configured_workflow_max_in_flight = configured_workflow_max_in_flight(run);
    let workflow_max_in_flight =
        effective_workflow_max_in_flight_for_run(run, configured_workflow_max_in_flight);
    let workflow_max_in_flight_env_override = workflow_max_in_flight_env_override(run);
    let openrouter_http1_only = run_env_bool(
        run,
        "SYMEM_OPENROUTER_HTTP_HTTP1_ONLY",
        run.embedder == "openrouter",
    );
    let openrouter_http_client_pool_size =
        run_env_value(run, "SYMEM_OPENROUTER_HTTP_CLIENT_POOL_SIZE");
    let openrouter_http_pool_max_idle_per_host =
        run_env_value(run, "SYMEM_OPENROUTER_HTTP_POOL_MAX_IDLE_PER_HOST");
    let openrouter_http_connect_timeout_secs =
        run_env_value(run, "SYMEM_OPENROUTER_HTTP_CONNECT_TIMEOUT_SECS");
    let openrouter_http_tcp_keepalive_secs =
        run_env_value(run, "SYMEM_OPENROUTER_HTTP_TCP_KEEPALIVE_SECS");
    let chat_http1_only = run_env_bool(run, "SYMEM_CHAT_HTTP_HTTP1_ONLY", false);
    let chat_http_client_pool_size = run_env_value(run, "SYMEM_CHAT_HTTP_CLIENT_POOL_SIZE");
    let chat_http_pool_max_idle_per_host =
        run_env_value(run, "SYMEM_CHAT_HTTP_POOL_MAX_IDLE_PER_HOST");
    let chat_http_connect_timeout_secs = run_env_value(run, "SYMEM_CHAT_HTTP_CONNECT_TIMEOUT_SECS");
    let chat_http_tcp_keepalive_secs = run_env_value(run, "SYMEM_CHAT_HTTP_TCP_KEEPALIVE_SECS");
    let chat_http_timeout_secs = run_env_value(run, "SYMEM_CHAT_HTTP_TIMEOUT_SECS");
    let openrouter_embed_input_type = run_env_value(run, "SYMEM_OPENROUTER_EMBED_INPUT_TYPE")
        .or_else(|| run_env_value(run, "SYMEM_EMBED_INPUT_TYPE"));
    let openrouter_embed_send_default_input_type =
        run_env_bool(run, "SYMEM_OPENROUTER_EMBED_SEND_DEFAULT_INPUT_TYPE", false);
    let embed_batch_size = run_env_value(run, "SYMEM_EMBED_BATCH_SIZE");
    let embed_batch_max_chars = run_env_value(run, "SYMEM_EMBED_BATCH_MAX_CHARS");
    let embed_max_chars = run_env_value(run, "SYMEM_EMBED_MAX_CHARS");
    let distill_thinking = role_thinking_label(run, "DISTILL");
    let query_planner_thinking = role_thinking_label(run, "QUERY_PLANNER");
    let answer_thinking = role_thinking_label(run, "ANSWER");
    let judge_thinking = role_thinking_label(run, "JUDGE");
    // Oracle-gold mode: the answerer is fed ONLY gold-session raw turns (the "reader ceiling"
    // method), bypassing recall. Set by `--oracle-gold` (which exports SYMEM_ORACLE_GOLD=1) or the
    // env var directly — read the env so both paths land in the run record.
    let oracle_gold = run_env_bool(run, "SYMEM_ORACLE_GOLD", false);
    let rerank = resolved_rerank_params(run);
    let mut params = json!({
        "schema": "membench.run_params.v1",
        "system": "symbiotic-memory",
        "benchmark": "long-mem-eval",
        "run_kind": "native",
        "run_name": run.run_name,
        "dataset": portable_path(&run.dataset),
        "run_root": portable_path(&run.run_root),
        "limit": run.limit,
        "sample": run.sample,
        "memory_manifest": portable_path(&run.memory_manifest),
        "memory_config": run.memory_config.as_deref().map(portable_path),
        "kit_config": kit_config_record(run),
        "symem_bin": run.symem_bin.as_deref().map(portable_path),
        "distiller": run.distiller,
        "embedder": run.embedder,
        "store": run.store,
        "prompt_dir": run.prompt_dir.as_deref().map(portable_path),
        "distill_prompt": run.distill_prompt,
        "answer_output": true,
        "generative_answerer_enabled": run.answerer,
        "answerer": run.answerer,
        "routed": run.routed,
        "answer_only": run.answer_only,
        "consolidate_briefs": run.consolidate_briefs,
        "stop_after_raw_embed": effective_stop_after_raw_embed(run),
        "ingest_diagnostic": effective_ingest_diagnostic(run),
        "resume": run.resume,
        "fresh": run.fresh,
        "query_planner": run.query_planner,
        "evidence_ledger": run.evidence_ledger,
        "answer_verifier": run.answer_verifier,
        "answer_gap_retry": run.answer_gap_retry,
        "answer_unavailable_retry": run.answer_unavailable_retry,
        "score_output": run.score,
        "score": run.score,
        "oracle": run.oracle.as_deref().map(portable_path),
        "oracle_gold": oracle_gold,
        "judge_workers": run.judge_workers,
        "prewarm_judge_cache": run.prewarm_judge_cache,
        "prewarm_pause_secs": run.prewarm_pause_secs,
        "scorer": run.scorer,
        "judge_operator": judge.operator,
        "judge_model": judge.model,
        "env_file": run.env_file.as_deref().map(portable_path),
        "provider_queue_dir": run.provider_queue_dir.as_deref().map(portable_path),
        "source_vault_root": run.source_vault_root.as_deref().map(portable_path),
        "workflow_max_in_flight": workflow_max_in_flight,
        "configured_workflow_max_in_flight": configured_workflow_max_in_flight,
        "workflow_max_in_flight_env_override": workflow_max_in_flight_env_override,
        "workflow": {
            "max_in_flight": workflow_max_in_flight,
            "configured_max_in_flight": configured_workflow_max_in_flight,
            "env_override": workflow_max_in_flight_env_override,
        },
        "ingest_stop_after_raw_embed": effective_stop_after_raw_embed(run),
        "ingest_diagnostic_mode": effective_ingest_diagnostic(run),
        "provider_queue_debug_requests": run_env_bool(run, "SYMEM_PROVIDER_QUEUE_DEBUG_REQUESTS", false),
        "openrouter_http1_only": openrouter_http1_only,
        "openrouter_http_client_pool_size": openrouter_http_client_pool_size,
        "openrouter_http_pool_max_idle_per_host": openrouter_http_pool_max_idle_per_host,
        "openrouter_http_connect_timeout_secs": openrouter_http_connect_timeout_secs,
        "openrouter_http_tcp_keepalive_secs": openrouter_http_tcp_keepalive_secs,
        "chat_http1_only": chat_http1_only,
        "chat_http_client_pool_size": chat_http_client_pool_size,
        "chat_http_pool_max_idle_per_host": chat_http_pool_max_idle_per_host,
        "chat_http_connect_timeout_secs": chat_http_connect_timeout_secs,
        "chat_http_tcp_keepalive_secs": chat_http_tcp_keepalive_secs,
        "chat_http_timeout_secs": chat_http_timeout_secs,
        "openrouter_embed_input_type": openrouter_embed_input_type,
        "openrouter_embed_send_default_input_type": openrouter_embed_send_default_input_type,
        "embed_batch_size": embed_batch_size,
        "embed_batch_max_chars": embed_batch_max_chars,
        "embed_max_chars": embed_max_chars,
        "transport": {
            "embed": {
                "provider": "openrouter",
                "http1_only": openrouter_http1_only,
                "client_pool_size": openrouter_http_client_pool_size,
                "pool_max_idle_per_host": openrouter_http_pool_max_idle_per_host,
                "connect_timeout_secs": openrouter_http_connect_timeout_secs,
                "tcp_keepalive_secs": openrouter_http_tcp_keepalive_secs,
                "label": transport_label(openrouter_http1_only, openrouter_http_client_pool_size.as_deref(), openrouter_http_pool_max_idle_per_host.as_deref()),
            },
            "chat": {
                "provider": "deepseek",
                "http1_only": chat_http1_only,
                "client_pool_size": chat_http_client_pool_size,
                "pool_max_idle_per_host": chat_http_pool_max_idle_per_host,
                "connect_timeout_secs": chat_http_connect_timeout_secs,
                "tcp_keepalive_secs": chat_http_tcp_keepalive_secs,
                "timeout_secs": chat_http_timeout_secs,
                "label": transport_label(chat_http1_only, chat_http_client_pool_size.as_deref(), chat_http_pool_max_idle_per_host.as_deref()),
            }
        },
        "thinking": {
            "distill": distill_thinking,
            "query_planner": query_planner_thinking,
            "answer": answer_thinking,
            "judge": judge_thinking,
            "summary": thinking_summary_label(run),
        },
    });
    let object = params
        .as_object_mut()
        .expect("run params JSON must be an object");
    // Fold the resolved rerank binding into configured_models so the shared
    // `configured_model(params, "rerank")` lookup (registry/dashboard) finds it
    // next to answer/distill/embed.
    let mut configured_models = configured_models;
    if let Some(configured_obj) = configured_models.as_object_mut() {
        configured_obj.insert("rerank".to_string(), rerank.clone());
    }
    object.insert("configured_models".to_string(), configured_models);
    object.insert("rerank".to_string(), rerank);
    object.insert("runtime_models".to_string(), runtime_models);
    object.insert("role_settings".to_string(), role_settings);
    object.insert(
        "runtime_provider_note".to_string(),
        json!(
            "Configured models come from the requested memory config; runtime models describe the providers this membench adapter actually invoked."
        ),
    );
    object.insert(
        "provider_queue_available".to_string(),
        json!(run.distiller != "heuristic" || run.embedder != "hash" || run.score || run.answerer),
    );
    object.insert("workflow_queue_available".to_string(), json!(true));
    object.insert(
        "ephemeral_smoke_run".to_string(),
        json!(run.ephemeral_smoke_run),
    );
    params
}

#[cfg_attr(not(feature = "symbiotic-memory-adapter"), allow(dead_code))]
fn role_thinking_label(run: &SymbioticMemoryCliRun, role: &str) -> Option<String> {
    fallback_thinking_mode_label(run, role).or_else(|| {
        #[cfg(feature = "symbiotic-memory-adapter")]
        {
            thinking_mode_label(default_thinking_mode(role))
        }
        #[cfg(not(feature = "symbiotic-memory-adapter"))]
        {
            let _ = role;
            None
        }
    })
}

#[cfg_attr(not(feature = "symbiotic-memory-adapter"), allow(dead_code))]
fn thinking_summary_label(run: &SymbioticMemoryCliRun) -> String {
    let roles = ["DISTILL", "QUERY_PLANNER", "ANSWER", "JUDGE"];
    if roles
        .iter()
        .all(|role| role_thinking_label(run, role).as_deref() == Some("disabled"))
    {
        "nonthinking".to_string()
    } else {
        roles
            .iter()
            .filter_map(|role| {
                role_thinking_label(run, role).map(|value| {
                    format!("{}:{}", role.to_ascii_lowercase().replace('_', "-"), value)
                })
            })
            .collect::<Vec<_>>()
            .join(",")
    }
}

#[cfg_attr(not(feature = "symbiotic-memory-adapter"), allow(dead_code))]
fn transport_label(http1_only: bool, pool: Option<&str>, idle: Option<&str>) -> String {
    let protocol = if http1_only { "h1" } else { "h2" };
    match (pool, idle) {
        (Some(pool), Some(idle)) => format!("{protocol} {pool}x{idle}"),
        (Some(pool), None) => format!("{protocol} {pool}x?"),
        (None, Some(idle)) => format!("{protocol} ?x{idle}"),
        (None, None) => protocol.to_string(),
    }
}

#[cfg_attr(not(feature = "symbiotic-memory-adapter"), allow(dead_code))]
fn configured_provider_models(run: &SymbioticMemoryCliRun) -> serde_json::Value {
    #[cfg(feature = "symbiotic-memory-adapter")]
    {
        if let Some(path) = &run.memory_config
            && let Ok(config) = symbiotic_memory::MemoryConfig::load_yaml(path)
        {
            return json!({
                "distill": provider_binding_for_role(run, "DISTILL", &config.providers.distill),
                "query_planner": provider_binding_for_role(run, "QUERY_PLANNER", &config.providers.query_planner),
                "answer": provider_binding_for_role(run, "ANSWER", &config.providers.answer),
                "embed": provider_binding_for_role(run, "EMBED", &effective_embedding_adapter(run, &config.providers.embedding)),
                "chat_provider": config.providers.chat_provider,
                "chat_model": config.providers.chat_model,
                "embedding_provider": config.providers.embedding_provider,
                "embedding_model": config.providers.embedding_model,
                "prompt_cache": config.providers.prompt_cache,
            });
        }
    }
    let answer = if run.answerer {
        "configured-by-adapter"
    } else {
        "disabled"
    };
    json!({
        "distill": fallback_provider_binding_for_role(run, "DISTILL", "chat", "deepseek", "deepseek-v4-flash"),
        "query_planner": fallback_provider_binding_for_role(run, "QUERY_PLANNER", "chat", "deepseek", "deepseek-v4-flash"),
        "answer": if run.answerer {
            fallback_provider_binding_for_role(run, "ANSWER", "chat", "deepseek", answer)
        } else {
            json!(answer)
        },
        "embed": if run.embedder == "ollama" {
            fallback_provider_binding_for_role(run, "EMBED", "embedding", "ollama", "nomic-embed-text")
        } else {
            fallback_provider_binding_for_role(run, "EMBED", "embedding", "gemini", "gemini-embedding-2")
        },
    })
}

#[cfg_attr(feature = "symbiotic-memory-adapter", allow(dead_code))]
fn fallback_provider_binding_for_role(
    run: &SymbioticMemoryCliRun,
    role: &str,
    operation: &str,
    default_operator: &str,
    default_model: &str,
) -> serde_json::Value {
    let operator = run_env_value(run, &format!("SYMEM_{role}_OPERATOR"))
        .unwrap_or_else(|| default_operator.to_string());
    let model = run_env_value(run, &format!("SYMEM_{role}_MODEL"))
        .unwrap_or_else(|| default_model.to_string());
    let queue_id = run_env_value(run, &format!("SYMEM_{role}_QUEUE_ID"))
        .unwrap_or_else(|| format!("{operation}:{operator}:{model}"));
    json!({
        "operation": operation,
        "operator": operator,
        "model": model,
        "queue_id": queue_id,
    })
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn provider_binding(adapter: &symbiotic_memory::ProviderAdapterConfig) -> serde_json::Value {
    let queue_id = adapter
        .queue_id
        .clone()
        .unwrap_or_else(|| adapter.default_queue_id());
    json!({
        "operation": adapter.operation,
        "operator": adapter.operator,
        "model": adapter.model,
        "queue_id": queue_id,
    })
}

fn queued_embedder(run: &SymbioticMemoryCliRun) -> bool {
    matches!(run.embedder.as_str(), "gemini" | "ollama" | "openrouter")
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn effective_embedding_adapter(
    run: &SymbioticMemoryCliRun,
    base: &symbiotic_memory::ProviderAdapterConfig,
) -> symbiotic_memory::ProviderAdapterConfig {
    if matches!(run.embedder.as_str(), "ollama" | "openrouter") {
        let default_operator = if run.embedder == "openrouter" {
            "openrouter"
        } else {
            "ollama"
        };
        let operator = run_env_value(run, "SYMEM_EMBED_OPERATOR")
            .unwrap_or_else(|| default_operator.to_string());
        let model = run_env_value(run, "SYMEM_EMBED_MODEL")
            .or_else(|| run_env_value(run, "SYMEM_OLLAMA_EMBED_MODEL"))
            .unwrap_or_else(|| {
                if run.embedder == "openrouter" {
                    "qwen/qwen3-embedding-8b".to_string()
                } else {
                    "nomic-embed-text".to_string()
                }
            });
        let mut adapter =
            symbiotic_memory::ProviderAdapterConfig::new("embedding", operator, model);
        if let Some(queue_id) = run_env_value(run, "SYMEM_EMBED_QUEUE_ID") {
            adapter.queue_id = Some(queue_id);
        }
        return adapter;
    }
    provider_adapter_for_role(run, "EMBED", base)
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn provider_binding_for_role(
    run: &SymbioticMemoryCliRun,
    role: &str,
    base: &symbiotic_memory::ProviderAdapterConfig,
) -> serde_json::Value {
    provider_binding(&provider_adapter_for_role(run, role, base))
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn provider_adapter_for_role(
    run: &SymbioticMemoryCliRun,
    role: &str,
    base: &symbiotic_memory::ProviderAdapterConfig,
) -> symbiotic_memory::ProviderAdapterConfig {
    let mut adapter = base.clone();
    if let Some(operator) = run_env_value(run, &format!("SYMEM_{role}_OPERATOR")) {
        adapter.operator = operator;
    }
    if let Some(model) = run_env_value(run, &format!("SYMEM_{role}_MODEL")) {
        adapter.model = model;
    }
    if let Some(queue_id) = run_env_value(run, &format!("SYMEM_{role}_QUEUE_ID")) {
        adapter.queue_id = Some(queue_id);
    }
    adapter
}

#[cfg_attr(not(feature = "symbiotic-memory-adapter"), allow(dead_code))]
fn runtime_provider_bindings(
    run: &SymbioticMemoryCliRun,
    judge: &ResolvedJudgeParams,
) -> serde_json::Value {
    #[cfg(feature = "symbiotic-memory-adapter")]
    if let Some(path) = &run.memory_config
        && let Ok(config) = symbiotic_memory::MemoryConfig::load_yaml(path)
    {
        let distill = provider_adapter_for_role(run, "DISTILL", &config.providers.distill);
        let embed = effective_embedding_adapter(run, &config.providers.embedding);
        let answer = provider_adapter_for_role(run, "ANSWER", &config.providers.answer);
        let query_planner =
            provider_adapter_for_role(run, "QUERY_PLANNER", &config.providers.query_planner);
        return json!({
            "distill": if run.distiller == "llm" {
                format!("queued:{}:{}", distill.operator, distill.model)
            } else {
                "local:heuristic-v1".to_string()
            },
            "embed": if queued_embedder(run) {
                format!("queued:{}:{}", embed.operator, embed.model)
            } else {
                "local:hash-embedding-v1".to_string()
            },
            "answer": if run.answerer {
                format!("queued:{}:{}", answer.operator, answer.model)
            } else {
                "local:extractive-answer".to_string()
            },
            "query_planner": if run.query_planner.as_deref() == Some("flash") {
                format!("queued:{}:{}", query_planner.operator, query_planner.model)
            } else {
                format!(
                    "local:{}",
                    run.query_planner.as_deref().unwrap_or("config-default")
                )
            },
            "judge": if run.score {
                format!("queued:{}:{}", judge.operator, judge.model)
            } else {
                "not-run".to_string()
            },
        });
    }
    let distill_binding =
        runtime_env_binding(run, "DISTILL").unwrap_or_else(|| "configured-chat".to_string());
    let embed_binding =
        runtime_env_binding(run, "EMBED").unwrap_or_else(|| "configured-embedding".to_string());
    let answer_binding =
        runtime_env_binding(run, "ANSWER").unwrap_or_else(|| "configured-chat".to_string());
    let query_binding =
        runtime_env_binding(run, "QUERY_PLANNER").unwrap_or_else(|| "configured-chat".to_string());
    json!({
        "distill": if run.distiller == "llm" { format!("queued:{distill_binding}") } else { "local:heuristic-v1".to_string() },
        "embed": if queued_embedder(run) { format!("queued:{embed_binding}") } else { "local:hash-embedding-v1".to_string() },
        "answer": if run.answerer {
            format!("queued:{answer_binding}")
        } else {
            "local:extractive-answer".to_string()
        },
        "query_planner": if run.query_planner.as_deref() == Some("flash") {
            format!("queued:{query_binding}")
        } else {
            format!(
                "local:{}",
                run.query_planner.as_deref().unwrap_or("config-default")
            )
        },
        "judge": if run.score {
            format!("queued:{}:{}", judge.operator, judge.model)
        } else {
            "not-run".to_string()
        },
    })
}

#[cfg_attr(not(feature = "symbiotic-memory-adapter"), allow(dead_code))]
fn resolved_role_settings(
    run: &SymbioticMemoryCliRun,
    judge: &ResolvedJudgeParams,
) -> serde_json::Value {
    #[cfg(not(feature = "symbiotic-memory-adapter"))]
    let _ = judge;

    #[cfg(feature = "symbiotic-memory-adapter")]
    if let Some(path) = &run.memory_config
        && let Ok(config) = symbiotic_memory::MemoryConfig::load_yaml(path)
    {
        let distill = provider_adapter_for_role(run, "DISTILL", &config.providers.distill);
        let embed = effective_embedding_adapter(run, &config.providers.embedding);
        let answer = provider_adapter_for_role(run, "ANSWER", &config.providers.answer);
        let query_planner =
            provider_adapter_for_role(run, "QUERY_PLANNER", &config.providers.query_planner);
        let judge_adapter = symbiotic_memory::ProviderAdapterConfig::new(
            "chat",
            judge.operator.clone(),
            judge.model.clone(),
        );
        return json!({
            "distill": role_setting_for_adapter(run, "DISTILL", &config, &distill, run.distiller == "llm"),
            "embed": role_setting_for_adapter(run, "EMBED", &config, &embed, queued_embedder(run)),
            "answer": role_setting_for_adapter(run, "ANSWER", &config, &answer, run.answerer),
            "query_planner": role_setting_for_adapter(run, "QUERY_PLANNER", &config, &query_planner, run.query_planner.as_deref() == Some("flash")),
            "judge": role_setting_for_adapter(run, "JUDGE", &config, &judge_adapter, run.score),
        });
    }
    json!({
        "distill": fallback_role_setting(run, "DISTILL", run.distiller == "llm"),
        "embed": fallback_role_setting(run, "EMBED", queued_embedder(run)),
        "answer": fallback_role_setting(run, "ANSWER", run.answerer),
        "query_planner": fallback_role_setting(run, "QUERY_PLANNER", run.query_planner.as_deref() == Some("flash")),
        "judge": fallback_role_setting(run, "JUDGE", run.score),
    })
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn role_setting_for_adapter(
    run: &SymbioticMemoryCliRun,
    role: &str,
    config: &symbiotic_memory::MemoryConfig,
    adapter: &symbiotic_memory::ProviderAdapterConfig,
    active: bool,
) -> serde_json::Value {
    let resolved = config.queue.resolve_provider_queue(adapter);
    json!({
        "active": active,
        "operation": adapter.operation,
        "operator": adapter.operator,
        "model": adapter.model,
        "queue_id": resolved.queue_id,
        "max_in_flight": resolved.max_in_flight,
        "requests_per_minute": resolved.requests_per_minute,
        "input_units_per_minute": resolved.input_units_per_minute,
        "timeout_seconds": resolved.timeout_seconds,
        "retry_attempts": resolved.retry_attempts,
        "logical_retry_attempts": resolved.logical_retry_attempts,
        "thinking": thinking_mode_label(thinking_mode(run, role).or_else(|| default_thinking_mode(role))),
        "reasoning_effort": role_reasoning_effort(run, role),
        "max_tokens": run_env_value(run, &format!("SYMEM_{role}_MAX_TOKENS"))
            .and_then(|value| value.parse::<u32>().ok())
            .or_else(|| default_role_max_tokens(role)),
    })
}

fn fallback_role_setting(
    run: &SymbioticMemoryCliRun,
    role: &str,
    active: bool,
) -> serde_json::Value {
    json!({
        "active": active,
        "thinking": fallback_thinking_mode_label(run, role),
        "reasoning_effort": fallback_role_reasoning_effort(run, role),
        "max_tokens": run_env_value(run, &format!("SYMEM_{role}_MAX_TOKENS"))
            .and_then(|value| value.parse::<u32>().ok()),
    })
}

fn fallback_thinking_mode_label(run: &SymbioticMemoryCliRun, role: &str) -> Option<String> {
    let value = run_env_value(run, &format!("SYMEM_{role}_THINKING"))
        .or_else(|| run_env_value(run, &format!("SYMEM_{role}_REASONING")))?;
    Some(match value.to_ascii_lowercase().as_str() {
        "off" | "disabled" | "disable" | "false" | "0" => "disabled".to_string(),
        "on" | "enabled" | "enable" | "true" | "1" | "high" | "max" => "enabled".to_string(),
        _ => value,
    })
}

fn fallback_role_reasoning_effort(run: &SymbioticMemoryCliRun, role: &str) -> Option<String> {
    run_env_value(run, &format!("SYMEM_{role}_REASONING_EFFORT")).or_else(|| {
        let value = run_env_value(run, &format!("SYMEM_{role}_THINKING"))?.to_ascii_lowercase();
        matches!(value.as_str(), "high" | "max").then_some(value)
    })
}

#[cfg_attr(feature = "symbiotic-memory-adapter", allow(dead_code))]
fn runtime_env_binding(run: &SymbioticMemoryCliRun, role: &str) -> Option<String> {
    let operator = run_env_value(run, &format!("SYMEM_{role}_OPERATOR"));
    let model = run_env_value(run, &format!("SYMEM_{role}_MODEL"));
    match (operator, model) {
        (Some(operator), Some(model)) => Some(format!("{operator}:{model}")),
        (Some(operator), None) => Some(format!("{operator}:configured-model")),
        (None, Some(model)) => Some(format!("configured-operator:{model}")),
        (None, None) => None,
    }
}

#[cfg_attr(not(feature = "symbiotic-memory-adapter"), allow(dead_code))]
struct ResolvedJudgeParams {
    operator: String,
    model: String,
}

#[cfg_attr(not(feature = "symbiotic-memory-adapter"), allow(dead_code))]
fn resolved_judge_params(run: &SymbioticMemoryCliRun) -> ResolvedJudgeParams {
    let operator =
        run_env_value(run, "SYMEM_JUDGE_OPERATOR").unwrap_or_else(|| "deepseek".to_string());
    let model = run_env_value(run, "SYMEM_JUDGE_MODEL")
        .or_else(|| scorer_judge_model(&run.scorer).map(ToOwned::to_owned))
        .unwrap_or_else(|| "deepseek-v4-flash".to_string());
    ResolvedJudgeParams { operator, model }
}

#[cfg_attr(not(feature = "symbiotic-memory-adapter"), allow(dead_code))]
fn scorer_judge_model(scorer: &str) -> Option<&str> {
    scorer.strip_prefix("queued-longmemeval-")
}

#[cfg_attr(not(feature = "symbiotic-memory-adapter"), allow(dead_code))]
/// Warn when a tuning knob is set while the gate that enables it is OFF. Such knobs silently no-op,
/// which has previously invalidated whole experiment sweeps. Each warning is a single clear line.
/// Detection uses `run_env_value` (process env first, then the run's env file) so it matches whatever
/// the engine's `std::env::var` gate reads. `redo_stage` is the resolved redo stage (e.g. "embed").
#[cfg_attr(not(feature = "symbiotic-memory-adapter"), allow(dead_code))]
fn warn_inert_tuning_knobs(run: &SymbioticMemoryCliRun, redo_stage: Option<&str>) {
    let is_set = |key: &str| run_env_value(run, key).is_some();

    // (a) Consolidator knobs require the consolidator gate: SYMEM_CONSOLIDATOR truthy AND
    //     consolidate_briefs on (i.e. not disabled via --no-consolidate-briefs).
    let consolidator_truthy = run_env_value(run, "SYMEM_CONSOLIDATOR")
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "llm" | "on" | "reweave" | "true" | "1"
            )
        })
        .unwrap_or(false);
    let consolidator_enabled = consolidator_truthy && run.consolidate_briefs;
    if !consolidator_enabled {
        for key in [
            "SYMEM_CONSOLIDATE_INPUT",
            "SYMEM_CONSOLIDATE_MODEL",
            "SYMEM_CONSOLIDATE_THINKING",
            "SYMEM_CONSOLIDATE_TURNS_PER_WINDOW",
        ] {
            if is_set(key) {
                eprintln!(
                    "WARNING: {key} is set but the consolidator is disabled (no truthy SYMEM_CONSOLIDATOR / --no-consolidate-briefs) — it will have NO effect."
                );
            }
        }
    }

    // (b) Re-embed knobs require --re-embed or SYMEM_REDO=embed.
    let reembed_active = run.re_embed || redo_stage == Some("embed");
    if !reembed_active {
        for key in ["SYMEM_REEMBED_CHUNK", "SYMEM_REEMBED_CONCURRENCY"] {
            if is_set(key) {
                eprintln!(
                    "WARNING: {key} is set but re-embed is off (no --re-embed / SYMEM_REDO=embed) — it will have NO effect."
                );
            }
        }
    }

    // (c) Multi-hop sub-knobs require the SYMEM_MULTIHOP gate.
    let multihop_enabled = run_env_value(run, "SYMEM_MULTIHOP")
        .map(|value| matches!(value.trim(), "1" | "true" | "on"))
        .unwrap_or(false);
    if !multihop_enabled {
        for key in [
            "SYMEM_MULTIHOP_SEED",
            "SYMEM_MULTIHOP_ENTITIES",
            "SYMEM_MULTIHOP_ROUND2_K",
            "SYMEM_MULTIHOP_ALL",
        ] {
            if is_set(key) {
                eprintln!(
                    "WARNING: {key} is set but multi-hop is off (SYMEM_MULTIHOP not 1/true/on) — it will have NO effect."
                );
            }
        }
    }

    // (d) Temporal-filter min-keep requires the SYMEM_TEMPORAL_FILTER gate.
    let temporal_filter_enabled = run_env_value(run, "SYMEM_TEMPORAL_FILTER")
        .map(|value| matches!(value.trim(), "1" | "true" | "on"))
        .unwrap_or(false);
    if !temporal_filter_enabled && is_set("SYMEM_TEMPORAL_FILTER_MIN") {
        eprintln!(
            "WARNING: SYMEM_TEMPORAL_FILTER_MIN is set but the temporal filter is off (SYMEM_TEMPORAL_FILTER not 1/true/on) — it will have NO effect."
        );
    }
}

fn run_env_value(run: &SymbioticMemoryCliRun, key: &str) -> Option<String> {
    if let Some(value) = std::env::var(key).ok().filter(|value| !value.is_empty()) {
        return Some(value);
    }
    let env_file = run.env_file.clone().or_else(|| default_env_file(run))?;
    load_env_file(&env_file)
        .ok()?
        .into_iter()
        .find_map(|(env_key, value)| (env_key == key && !value.is_empty()).then_some(value))
}

/// Resolve the kit's typed config for this run: defaults overlaid with every
/// `SYMBIOTIC_MEMORY__*` pair from the run env-file, then from the process
/// environment (process wins — same precedence as `run_env_value`). This is
/// the bench's config surface: arms are declared as config overrides, and the
/// resolved hash goes into the run record so every run is attributable to
/// exactly one configuration.
#[cfg(feature = "symbiotic-memory-adapter")]
fn resolve_kit_config(
    run: &SymbioticMemoryCliRun,
) -> anyhow::Result<symbiotic_memory_config::Resolved> {
    let mut env: Vec<(String, String)> = Vec::new();
    if let Some(env_file) = run.env_file.clone().or_else(|| default_env_file(run)) {
        if let Ok(pairs) = load_env_file(&env_file) {
            env.extend(
                pairs
                    .into_iter()
                    .filter(|(key, _)| key.starts_with("SYMBIOTIC_MEMORY__")),
            );
        }
    }
    // Process env after the file so equal keys resolve to the process value
    // (later pairs win inside the resolver's env layer).
    env.extend(symbiotic_memory_config::ConfigLayers::env_from_process());
    symbiotic_memory_config::resolve(&symbiotic_memory_config::ConfigLayers {
        file: None,
        env,
        flags: Vec::new(),
    })
    .map_err(|err| anyhow::anyhow!("kit config resolution failed: {err}"))
}

#[cfg_attr(not(feature = "symbiotic-memory-adapter"), allow(dead_code))]
fn run_env_bool(run: &SymbioticMemoryCliRun, key: &str, default: bool) -> bool {
    run_env_value(run, key)
        .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
        .unwrap_or(default)
}

fn effective_stop_after_raw_embed(run: &SymbioticMemoryCliRun) -> bool {
    run.stop_after_raw_embed || run_env_bool(run, "SYMEM_INGEST_STOP_AFTER_RAW_EMBED", false)
}

/// Resolve the reranker binding (model/operator/base url + optional stage-1 prefilter) from the
/// `SYMEM_RERANK*` env knobs WITHOUT building the actual reranker. Mirrors `reranker()` /
/// `build_reranker()` so the run record reports the same model the recall engine actually used.
/// Returns `{ "enabled": false }` when rerank is off.
fn resolved_rerank_params(run: &SymbioticMemoryCliRun) -> serde_json::Value {
    let enabled = run_env_value(run, "SYMEM_RERANK")
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "on"
            )
        })
        .unwrap_or(true);
    if !enabled {
        return json!({ "enabled": false });
    }
    let base_url = run_env_value(run, "SYMEM_RERANK_BASE_URL")
        .unwrap_or_else(|| "https://openrouter.ai/api/v1".to_string());
    let model = run_env_value(run, "SYMEM_RERANK_MODEL")
        .unwrap_or_else(|| "nvidia/llama-nemotron-rerank-vl-1b-v2:free".to_string());
    let is_local = base_url.contains("localhost")
        || base_url.contains("127.0.0.1")
        || base_url.contains("[::1]");
    let operator = run_env_value(run, "SYMEM_RERANK_OPERATOR")
        .unwrap_or_else(|| if is_local { "local" } else { "openrouter" }.to_string());
    let stage1_model =
        run_env_value(run, "SYMEM_RERANK_STAGE1_MODEL").filter(|value| !value.trim().is_empty());
    json!({
        "enabled": true,
        "operation": "rerank",
        "operator": operator,
        "model": model,
        "base_url": base_url,
        "queue_id": format!("rerank:{operator}:{model}"),
        "stage1_model": stage1_model,
    })
}

fn effective_ingest_diagnostic(run: &SymbioticMemoryCliRun) -> Option<String> {
    if effective_stop_after_raw_embed(run) {
        return Some("raw-embed".to_string());
    }
    run.ingest_diagnostic
        .clone()
        .or_else(|| run_env_value(run, "SYMEM_INGEST_DIAGNOSTIC_MODE"))
        .and_then(normalize_ingest_diagnostic)
}

fn normalize_ingest_diagnostic(value: String) -> Option<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "" | "none" | "off" | "false" | "0" => None,
        "raw" | "raw-embed" | "raw_embed" | "raw-only" | "raw_only" => {
            Some("raw-embed".to_string())
        }
        "distill" | "distill-only" | "distill_only" => Some("distill".to_string()),
        "raw-embed-distill" | "raw_embed_distill" | "raw+distill" | "raw_distill"
        | "embed-distill" | "embed_distill" => Some("raw-embed-distill".to_string()),
        _ => None,
    }
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn adapter_ingest_diagnostic_mode(
    run: &SymbioticMemoryCliRun,
) -> symbiotic_memory::IngestDiagnosticMode {
    match effective_ingest_diagnostic(run).as_deref() {
        Some("raw-embed") => symbiotic_memory::IngestDiagnosticMode::RawEmbedOnly,
        Some("distill") => symbiotic_memory::IngestDiagnosticMode::DistillOnly,
        Some("raw-embed-distill") => symbiotic_memory::IngestDiagnosticMode::RawEmbedAndDistill,
        _ => symbiotic_memory::IngestDiagnosticMode::None,
    }
}

fn default_native_run_name() -> String {
    let now = Utc::now();
    let seed = format!(
        "{}:{}:{}",
        now.timestamp_nanos_opt().unwrap_or_default(),
        std::process::id(),
        repo_root().display()
    );
    let hash = Sha256::digest(seed.as_bytes());
    format!(
        "{}-{:08x}",
        now.format("%Y%m%d-%H%M%S"),
        u32::from_be_bytes(hash[..4].try_into().unwrap_or_default())
    )
}

#[allow(dead_code)]
fn apply_symbiotic_memory_env(
    run: &SymbioticMemoryCliRun,
    cmd: &mut std::process::Command,
) -> anyhow::Result<()> {
    let env_file = run.env_file.clone().or_else(|| default_env_file(run));
    if let Some(env_file) = env_file {
        for (key, value) in load_env_file(&env_file)? {
            if std::env::var_os(&key).is_none() {
                cmd.env(key, value);
            }
        }
    }
    let provider_queue_dir = run
        .provider_queue_dir
        .clone()
        .unwrap_or_else(|| run.run_root.join("provider-queue"));
    cmd.env("SYMEM_PROVIDER_QUEUE_DIR", provider_queue_dir);
    if let Some(memory_config) = &run.memory_config {
        cmd.env("SYMEM_CONFIG", memory_config);
    }
    set_env_default(cmd, "SYMEM_JUDGE_OPERATOR", "deepseek");
    set_env_default(cmd, "SYMEM_JUDGE_BASE_URL", "https://api.deepseek.com");
    set_env_default(cmd, "SYMEM_JUDGE_MODEL", "deepseek-v4-flash");
    set_env_default(cmd, "SYMEM_JUDGE_THINKING", "disabled");
    set_env_default(cmd, "SYMEM_JUDGE_MAX_TOKENS", "64");
    set_env_default(cmd, "SYMEM_DISTILL_PARSE_RETRIES", "4");
    set_env_default(cmd, "SYMEM_DISTILL_WINDOW_TIMEOUT_SECS", "0");
    set_env_default(cmd, "SYMEM_DISTILL_TURNS_PER_WINDOW", "16");
    set_env_default(cmd, "SYMEM_QUESTION_TIMEOUT_SECS", "0");
    set_env_default(cmd, "SYMEM_TRACE_JSONL", "1");
    set_env_default(cmd, "SYMEM_QUEUE_TRACE_JSONL", "1");
    Ok(())
}

#[allow(dead_code)]
fn apply_symbiotic_memory_prewarm_env(
    prewarm: &JudgeCachePrewarmFiles,
    cmd: &mut std::process::Command,
) {
    let provider_queue_dir = prewarm.trace_dir.join("provider-queue");
    cmd.env("SYMEM_PROVIDER_QUEUE_DIR", &provider_queue_dir);
    cmd.env(
        "SYMEM_TRACE_JSONL_PATH",
        prewarm.trace_dir.join("model-traces.jsonl"),
    );
    cmd.env(
        "SYMEM_QUEUE_TRACE_JSONL_PATH",
        provider_queue_dir.join("model-queue-traces.jsonl"),
    );
}

fn default_env_file(_run: &SymbioticMemoryCliRun) -> Option<PathBuf> {
    let bench_env = repo_root().join(".env.test.local");
    bench_env.exists().then_some(bench_env)
}

#[allow(dead_code)]
fn set_env_default(cmd: &mut std::process::Command, key: &str, value: &str) {
    if std::env::var_os(key).is_none() {
        cmd.env(key, value);
    }
}

fn load_env_file(path: &PathBuf) -> anyhow::Result<BTreeMap<String, String>> {
    let raw = std::fs::read_to_string(path)?;
    let mut out = BTreeMap::new();
    for (idx, line) in raw.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line);
        let Some((key, value)) = line.split_once('=') else {
            anyhow::bail!("invalid env line {} in {}", idx + 1, path.display());
        };
        let key = key.trim();
        if key.is_empty() {
            anyhow::bail!("empty env key on line {} in {}", idx + 1, path.display());
        }
        let value = unquote_env_value(value.trim());
        out.insert(key.to_string(), value);
    }
    Ok(out)
}

fn unquote_env_value(value: &str) -> String {
    if value.len() >= 2 {
        let bytes = value.as_bytes();
        if (bytes[0] == b'"' && bytes[value.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[value.len() - 1] == b'\'')
        {
            return value[1..value.len() - 1].to_string();
        }
    }
    value.to_string()
}

#[derive(Debug, PartialEq, Eq)]
#[allow(dead_code)]
struct PlannedCommand {
    program: std::ffi::OsString,
    args: Vec<std::ffi::OsString>,
}

/// Build the shared, pure run plan from the CLI's execution struct. The CLI and
/// the dashboard plan the exact same `symem` command through `runner`.
#[allow(dead_code)]
fn to_symem_plan(run: &SymbioticMemoryCliRun) -> runner::SymemRunPlan {
    runner::SymemRunPlan {
        repo_root: repo_root(),
        run_root: run.run_root.clone(),
        run_root_explicit: true,
        dataset: run.dataset.clone(),
        dataset_explicit: true,
        limit: run.limit,
        sample: run.sample.clone(),
        distiller: run.distiller.clone(),
        embedder: run.embedder.clone(),
        store: run.store.clone(),
        prompt_dir: run.prompt_dir.clone(),
        distill_prompt: run.distill_prompt.clone(),
        answerer: run.answerer,
        routed: run.routed,
        answer_only: run.answer_only,
        source_vault_root: run.source_vault_root.clone(),
        consolidate_briefs: run.consolidate_briefs,
        ingest_diagnostic: effective_ingest_diagnostic(run),
        resume: run.resume,
        fresh: run.fresh,
        query_planner: run.query_planner.clone(),
        score: run.score,
        oracle: run.oracle.clone(),
        judge_workers: run.judge_workers,
        prewarm_judge_cache: run.prewarm_judge_cache,
        prewarm_pause_secs: run.prewarm_pause_secs,
        scorer: run.scorer.clone(),
        symem_bin: run.symem_bin.clone(),
        memory_manifest: run.memory_manifest.clone(),
        memory_manifest_explicit: true,
        memory_config: run.memory_config.clone(),
        smoke: run.distiller == "heuristic" && run.embedder == "hash" && !run.score,
    }
}

#[allow(dead_code)]
fn from_runner_command(planned: runner::PlannedCommand) -> PlannedCommand {
    PlannedCommand {
        program: std::ffi::OsString::from(planned.program),
        args: planned
            .args
            .into_iter()
            .map(std::ffi::OsString::from)
            .collect(),
    }
}

#[allow(dead_code)]
fn plan_symbiotic_memory_command(run: &SymbioticMemoryCliRun) -> PlannedCommand {
    from_runner_command(to_symem_plan(run).run_command())
}

#[allow(dead_code)]
fn plan_symbiotic_memory_score_command(
    run: &SymbioticMemoryCliRun,
    oracle: &Path,
) -> PlannedCommand {
    let mut plan = to_symem_plan(run);
    plan.score = true;
    plan.oracle = Some(oracle.to_path_buf());
    from_runner_command(plan.run_command())
}

fn summarize_queue_events(path: PathBuf) -> anyhow::Result<()> {
    let raw = std::fs::read_to_string(path)?;
    let mut events = Vec::new();
    for (idx, line) in raw.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let event: BenchQueueEvent = serde_json::from_str(line)
            .map_err(|err| anyhow::anyhow!("invalid queue event on line {}: {err}", idx + 1))?;
        events.push(event);
    }
    let summary = summarize_queue_timing(&events);
    println!("{}", serde_json::to_string_pretty(&summary)?);
    Ok(())
}

fn summarize_model_traces(path: PathBuf) -> anyhow::Result<()> {
    let path = resolve_repo_path(&path);
    let summary = cost::rollup_model_trace_file(&path)
        .ok_or_else(|| anyhow::anyhow!("no model traces found at {}", path.display()))?;
    println!("{}", serde_json::to_string_pretty(&summary)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_run(symem_bin: Option<PathBuf>) -> SymbioticMemoryCliRun {
        SymbioticMemoryCliRun {
            dataset: PathBuf::from("data/longmemeval.json"),
            run_root: PathBuf::from("runs/symbiotic-memory/long-mem-eval/3/sample"),
            run_name: "sample".to_string(),
            limit: 3,
            sample: "stratified".to_string(),
            memory_manifest: PathBuf::from("../symbiotic-memory/Cargo.toml"),
            memory_config: None,
            symem_bin,
            distiller: "llm".to_string(),
            embedder: "gemini".to_string(),
            store: "zvec-hybrid".to_string(),
            prompt_dir: None,
            distill_prompt: "distill".to_string(),
            answerer: true,
            routed: false,
            answer_only: true,
            re_embed: false,
            consolidate_briefs: false,
            stop_after_raw_embed: false,
            ingest_diagnostic: None,
            resume: true,
            fresh: false,
            query_planner: Some("off".to_string()),
            evidence_ledger: false,
            answer_verifier: false,
            answer_gap_retry: false,
            answer_unavailable_retry: false,
            score: true,
            oracle: None,
            judge_workers: 400,
            prewarm_judge_cache: 0,
            prewarm_pause_secs: 10,
            scorer: "queued-longmemeval-deepseek-v4-flash".to_string(),
            env_file: None,
            provider_queue_dir: None,
            source_vault_root: None,
            ephemeral_smoke_run: false,
        }
    }

    fn arg_strings(plan: &PlannedCommand) -> Vec<String> {
        plan.args
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn answer_only_uses_sane_workflow_window_by_default() {
        assert_eq!(effective_workflow_max_in_flight(true, Some(25)), 64);
        assert_eq!(effective_workflow_max_in_flight(false, Some(25)), 25);
        assert_eq!(effective_workflow_max_in_flight(false, None), 50);
    }

    #[test]
    fn gold_turn_ids_uses_session_id_and_turn_index_of_answer_turns() {
        use symbiotic_mem_bench::symbiotic_memory_adapter::{
            LongMemEvalMessage, LongMemEvalRecord,
        };
        let msg = |has_answer: bool| LongMemEvalMessage {
            role: "user".to_string(),
            content: String::new(),
            has_answer,
        };
        let record = LongMemEvalRecord {
            question_id: "q".to_string(),
            question_type: None,
            question: String::new(),
            question_date: None,
            answer: None,
            answer_session_ids: vec!["answer_530960c1".to_string()],
            haystack_dates: Vec::new(),
            haystack_session_ids: vec!["sess_a".to_string(), "answer_530960c1".to_string()],
            haystack_sessions: vec![
                // sess_a: no answer turns -> contributes nothing.
                vec![msg(false), msg(false)],
                // answer_530960c1: turn index 1 carries the answer.
                vec![msg(false), msg(true), msg(false)],
            ],
        };
        let gold = gold_turn_ids(&record);
        // The gold turn id is `<session_id>:<turn_index>`, turn-level, not the
        // session id alone.
        assert_eq!(
            gold.into_iter().collect::<Vec<_>>(),
            vec!["answer_530960c1:1".to_string()]
        );
    }

    #[test]
    fn raw_only_ranks_normalize_merged_and_separate_traces_identically() {
        // The same three raw turns + the same fact, expressed as a MERGED trace
        // (raw:-prefixed raw turns interleaved with a fact, polluted global
        // embedding_rank) and as a SEPARATE raw_turn trace (bare ids, raw-only).
        // After re-ranking raw candidates among themselves, the deepest gold
        // rank must match — that is the merged-vs-separate normalization.
        let gold: BTreeSet<String> = ["s:1".to_string()].into_iter().collect();

        let merged = json!([{
            "candidate_type": "merged",
            "candidates": [
                // A fact sits at the top of the merged list (rank 0) but must be
                // ignored when ranking raw turns.
                {"candidate_id": "fact:mem-x", "embedding_score": 0.99, "rerank_score": 0.99},
                {"candidate_id": "raw:s:0", "embedding_score": 0.8, "rerank_score": 0.2},
                {"candidate_id": "raw:s:1", "embedding_score": 0.5, "rerank_score": 0.9},
                {"candidate_id": "raw:s:2", "embedding_score": 0.3, "rerank_score": 0.1},
            ],
        }]);
        let separate = json!([
            {"candidate_type": "fact", "candidates": [
                {"candidate_id": "mem-x", "embedding_score": 0.99, "rerank_score": 0.99},
            ]},
            {"candidate_type": "raw_turn", "candidates": [
                {"candidate_id": "s:0", "embedding_score": 0.8, "rerank_score": 0.2},
                {"candidate_id": "s:1", "embedding_score": 0.5, "rerank_score": 0.9},
                {"candidate_id": "s:2", "embedding_score": 0.3, "rerank_score": 0.1},
            ]},
        ]);

        for trace in [&merged, &separate] {
            let cands = raw_turn_candidates(trace.as_array().unwrap());
            // The fact is excluded; only the three raw turns remain.
            assert_eq!(cands.len(), 3, "raw-only candidate set");
            // By embed score desc: s:0(0.8), s:1(0.5), s:2(0.3) -> gold s:1 is #2.
            assert_eq!(
                deepest_gold_rank(&cands, &gold, |c| c.embedding_score),
                Some(2)
            );
            // By rerank score desc: s:1(0.9), s:0(0.2), s:2(0.1) -> gold s:1 is #1.
            assert_eq!(
                deepest_gold_rank(&cands, &gold, |c| c.rerank_score),
                Some(1)
            );
        }
    }

    #[test]
    fn deepest_gold_rank_reports_worst_of_multiple_gold_turns_and_none_when_absent() {
        let cands = raw_turn_candidates(
            json!([{
                "candidate_type": "raw_turn",
                "candidates": [
                    {"candidate_id": "s:0", "embedding_score": 0.9, "rerank_score": 0.9},
                    {"candidate_id": "s:1", "embedding_score": 0.5, "rerank_score": 0.5},
                    {"candidate_id": "s:2", "embedding_score": 0.1, "rerank_score": 0.1},
                ],
            }])
            .as_array()
            .unwrap(),
        );
        // Two gold turns (s:0 best, s:2 worst) -> deepest is the worst = rank 3.
        let two: BTreeSet<String> = ["s:0".to_string(), "s:2".to_string()].into_iter().collect();
        assert_eq!(
            deepest_gold_rank(&cands, &two, |c| c.embedding_score),
            Some(3)
        );
        // A gold turn that never appears in the candidate set -> None (not in set).
        let absent: BTreeSet<String> = ["s:99".to_string()].into_iter().collect();
        assert_eq!(
            deepest_gold_rank(&cands, &absent, |c| c.embedding_score),
            None
        );
    }

    #[test]
    fn raw_turn_candidates_fall_back_to_final_rank_when_no_rerank_score() {
        // When a candidate has only `final_rank` (no rerank_score), the rerank
        // order is reconstructed as -final_rank (rank 0 = highest score).
        let cands = raw_turn_candidates(
            json!([{
                "candidate_type": "raw_turn",
                "candidates": [
                    {"candidate_id": "s:0", "embedding_score": 0.1, "final_rank": 2},
                    {"candidate_id": "s:1", "embedding_score": 0.9, "final_rank": 0},
                ],
            }])
            .as_array()
            .unwrap(),
        );
        let gold: BTreeSet<String> = ["s:0".to_string()].into_iter().collect();
        // final_rank: s:1 (rank 0) then s:0 (rank 2) -> gold s:0 is rerank #2.
        assert_eq!(
            deepest_gold_rank(&cands, &gold, |c| c.rerank_score),
            Some(2)
        );
    }

    #[cfg(feature = "symbiotic-memory-adapter")]
    #[test]
    fn paid_provider_runs_require_single_process_lock() {
        let mut run = sample_run(None);
        run.ephemeral_smoke_run = false;
        run.distiller = "llm".to_string();
        run.embedder = "gemini".to_string();
        run.score = true;
        assert!(requires_paid_provider_lock(&run));

        run.ephemeral_smoke_run = true;
        assert!(!requires_paid_provider_lock(&run));

        run.ephemeral_smoke_run = false;
        run.distiller = "heuristic".to_string();
        run.embedder = "hash".to_string();
        run.score = false;
        run.scorer = "none".to_string();
        assert!(!requires_paid_provider_lock(&run));
    }

    #[cfg(feature = "symbiotic-memory-adapter")]
    #[test]
    fn paid_provider_lock_refuses_second_owner() {
        let dir = tempfile::tempdir().unwrap();
        let run = sample_run(None);
        let first = PaidProviderRunLock::acquire_in(dir.path().join(".locks"), &run).unwrap();
        let err = PaidProviderRunLock::acquire_in(dir.path().join(".locks"), &run).unwrap_err();
        assert!(
            err.to_string()
                .contains("another paid provider-backed membench run appears to be active")
        );
        drop(first);
        PaidProviderRunLock::acquire_in(dir.path().join(".locks"), &run).unwrap();
    }

    #[cfg(feature = "symbiotic-memory-adapter")]
    #[test]
    fn paid_provider_lock_reclaims_dead_owner() {
        let dir = tempfile::tempdir().unwrap();
        let run = sample_run(None);
        let lock_root = dir.path().join(".locks");
        let lock_path = lock_root.join("paid-provider-run.lock");
        std::fs::create_dir_all(&lock_path).unwrap();
        std::fs::write(
            lock_path.join("owner.json"),
            serde_json::to_vec_pretty(&json!({
                "schema": "membench.paid_provider_run_lock.v1",
                "pid": u32::MAX,
            }))
            .unwrap(),
        )
        .unwrap();

        let lock = PaidProviderRunLock::acquire_in(lock_root, &run).unwrap();
        let owner: Value =
            serde_json::from_slice(&std::fs::read(lock.path.join("owner.json")).unwrap()).unwrap();

        assert_eq!(owner["pid"].as_u64(), Some(std::process::id() as u64));
    }

    #[cfg(all(feature = "symbiotic-memory-adapter", unix))]
    #[test]
    fn prepares_answer_only_linked_vaults_without_copying_heavy_state() {
        use symbiotic_mem_bench::symbiotic_memory_adapter::{
            LongMemEvalMessage, LongMemEvalRecord,
        };

        let dir = tempfile::tempdir().unwrap();
        let source_root = dir.path().join("source-vaults");
        let run_root = dir.path().join("run");
        let source_vault = source_root.join("q1");
        std::fs::create_dir_all(source_vault.join("archive/memories")).unwrap();
        std::fs::create_dir_all(source_vault.join("zvec-hybrid")).unwrap();
        std::fs::write(source_vault.join("manifest.json"), r#"{"source":"stable"}"#).unwrap();
        std::fs::write(source_vault.join("memory.sqlite"), b"sqlite").unwrap();
        std::fs::write(source_vault.join("archive/memories/fact.md"), "fact").unwrap();
        std::fs::write(
            source_vault.join("zvec-hybrid/index-manifest.json"),
            r#"{"source":"zvec"}"#,
        )
        .unwrap();
        let rows = vec![LongMemEvalRecord {
            question_id: "q1".to_string(),
            question_type: Some("direct".to_string()),
            question: "What happened?".to_string(),
            answer: None,
            question_date: None,
            haystack_dates: Vec::new(),
            haystack_session_ids: Vec::new(),
            haystack_sessions: vec![vec![LongMemEvalMessage {
                role: "user".to_string(),
                content: "fact".to_string(),
            }]],
        }];

        prepare_answer_only_linked_vaults(&run_root, &source_root, &rows).unwrap();

        let target_vault = run_root.join("vaults/q1");
        assert!(
            std::fs::symlink_metadata(target_vault.join("memory.sqlite"))
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert!(
            std::fs::symlink_metadata(target_vault.join("archive"))
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert!(
            !std::fs::symlink_metadata(target_vault.join("manifest.json"))
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert!(
            std::fs::symlink_metadata(target_vault.join("zvec-hybrid"))
                .unwrap()
                .file_type()
                .is_symlink()
        );
        remove_path_if_exists(&target_vault.join("zvec-hybrid")).unwrap();
        std::fs::create_dir_all(target_vault.join("zvec-hybrid")).unwrap();
        std::fs::write(
            target_vault.join("zvec-hybrid/index-manifest.json"),
            r#"{"source":"stale-target"}"#,
        )
        .unwrap();
        prepare_answer_only_linked_vaults(&run_root, &source_root, &rows).unwrap();
        assert!(
            std::fs::symlink_metadata(target_vault.join("zvec-hybrid"))
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(
            std::fs::read_to_string(target_vault.join("zvec-hybrid/index-manifest.json")).unwrap(),
            r#"{"source":"zvec"}"#
        );

        std::fs::write(target_vault.join("manifest.json"), r#"{"source":"target"}"#).unwrap();
        assert_eq!(
            std::fs::read_to_string(source_vault.join("manifest.json")).unwrap(),
            r#"{"source":"stable"}"#
        );
    }

    #[cfg(unix)]
    #[test]
    fn vault_store_save_copy_records_meta_and_resolves_by_name() {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join("store");
        let run_root = dir.path().join("run-abc");
        let vaults = run_root.join("vaults");
        std::fs::create_dir_all(vaults.join("q1")).unwrap();
        std::fs::create_dir_all(vaults.join("q2")).unwrap();
        std::fs::write(vaults.join("q1/memory.sqlite"), b"sqlite").unwrap();
        std::fs::write(
            run_root.join("benchmark-report.json"),
            r#"{"metrics":{"accuracy":{"value":0.874}}}"#,
        )
        .unwrap();

        // COPY (default): store is populated, source survives intact.
        vault_save_in(&store, &run_root, Some("golden"), false).unwrap();
        let stored_vaults = store.join("golden/vaults");
        assert!(stored_vaults.join("q1/memory.sqlite").is_file());
        assert!(vaults.join("q1/memory.sqlite").is_file());
        assert!(
            !std::fs::symlink_metadata(&vaults)
                .unwrap()
                .file_type()
                .is_symlink()
        );

        let meta: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(store.join("golden/store-meta.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(meta["vault_count"].as_u64(), Some(2));
        assert_eq!(meta["accuracy"].as_f64(), Some(0.874));
        assert_eq!(meta["moved"].as_bool(), Some(false));
        assert!(meta["saved_at"].as_str().is_some());

        // Re-saving the same key is refused.
        assert!(vault_save_in(&store, &run_root, Some("golden"), false).is_err());

        // By-name discovery: a bare key resolves to the store's vaults dir.
        let resolved =
            resolve_source_vault_root_with_store(Path::new("golden"), Some(store.as_path()));
        assert_eq!(resolved, stored_vaults);
        // An existing path always wins over the store and is never overridden.
        let direct = resolve_source_vault_root_with_store(&vaults, Some(store.as_path()));
        assert_eq!(direct, vaults);
        // Unknown key with no store falls through to the (non-existent) literal path.
        let miss = resolve_source_vault_root_with_store(Path::new("nope"), Some(store.as_path()));
        assert_eq!(miss, resolve_repo_path(Path::new("nope")));

        // listing and path lookups succeed against the populated store.
        vault_list_in(&store).unwrap();
        assert!(vault_path_resolved(&store, "golden").unwrap().is_dir());

        // MOVE: relocates the tree and leaves a symlink behind at the run's vaults/.
        let move_run = dir.path().join("run-move");
        let move_vaults = move_run.join("vaults");
        std::fs::create_dir_all(move_vaults.join("q1")).unwrap();
        vault_save_in(&store, &move_run, Some("moved-key"), true).unwrap();
        assert!(store.join("moved-key/vaults/q1").is_dir());
        let link = std::fs::symlink_metadata(&move_vaults).unwrap();
        assert!(link.file_type().is_symlink());
        assert_eq!(
            std::fs::read_link(&move_vaults).unwrap(),
            store.join("moved-key/vaults")
        );
    }

    // Mirror of `vault_path`'s store-relative resolution for the test above.
    fn vault_path_resolved(store: &Path, key: &str) -> anyhow::Result<PathBuf> {
        let vaults = store.join(sanitize_path_component(key)).join("vaults");
        if !vaults.is_dir() {
            anyhow::bail!("no stored vaults for key '{key}'");
        }
        Ok(vaults)
    }

    #[test]
    fn prepares_judge_cache_prewarm_subset_files() {
        let dir = tempfile::tempdir().unwrap();
        let run_root = dir.path().join("run");
        std::fs::create_dir_all(native_raw_dir(&run_root)).unwrap();
        std::fs::write(
            native_hypotheses_path(&run_root),
            [
                r#"{"question_id":"q1","question":"one","hypothesis":"a"}"#,
                r#"{"question_id":"q2","question":"two","hypothesis":"b"}"#,
                r#"{"question_id":"q3","question":"three","hypothesis":"c"}"#,
            ]
            .join("\n")
                + "\n",
        )
        .unwrap();
        let oracle = dir.path().join("oracle.json");
        std::fs::write(
            &oracle,
            r#"[
              {"question_id":"q1","question":"one","answer":"a","haystack_dates":[],"haystack_session_ids":[],"haystack_sessions":[]},
              {"question_id":"q2","question":"two","answer":"b","haystack_dates":[],"haystack_session_ids":[],"haystack_sessions":[]},
              {"question_id":"q3","question":"three","answer":"c","haystack_dates":[],"haystack_session_ids":[],"haystack_sessions":[]}
            ]"#,
        )
        .unwrap();
        let mut run = sample_run(None);
        run.run_root = run_root.clone();
        run.prewarm_judge_cache = 2;

        let files = prepare_judge_cache_prewarm(&run, &oracle).unwrap();

        assert_eq!(files.count, 2);
        assert_eq!(
            std::fs::read_to_string(files.hypotheses)
                .unwrap()
                .lines()
                .count(),
            2
        );
        let oracle_subset = read_json(&files.oracle).unwrap();
        assert_eq!(oracle_subset.as_array().unwrap().len(), 2);
        assert!(
            run_root
                .join("raw/judge-cache-prewarm/oracle.json")
                .exists()
        );
        assert_eq!(
            files.trace_dir,
            run_root.join("raw").join("judge-cache-prewarm")
        );
    }

    #[test]
    fn prewarm_env_uses_isolated_queue_and_trace_paths() {
        let prewarm = JudgeCachePrewarmFiles {
            hypotheses: PathBuf::from("run/raw/judge-cache-prewarm/hypotheses.jsonl"),
            oracle: PathBuf::from("run/raw/judge-cache-prewarm/oracle.json"),
            trace_dir: PathBuf::from("run/raw/judge-cache-prewarm"),
            count: 5,
        };
        let mut cmd = std::process::Command::new("symem");

        apply_symbiotic_memory_prewarm_env(&prewarm, &mut cmd);

        let envs = cmd
            .get_envs()
            .filter_map(|(key, value)| {
                Some((
                    key.to_string_lossy().into_owned(),
                    value?.to_string_lossy().into_owned(),
                ))
            })
            .collect::<BTreeMap<_, _>>();
        assert_eq!(
            envs.get("SYMEM_PROVIDER_QUEUE_DIR").map(String::as_str),
            Some("run/raw/judge-cache-prewarm/provider-queue")
        );
        assert_eq!(
            envs.get("SYMEM_TRACE_JSONL_PATH").map(String::as_str),
            Some("run/raw/judge-cache-prewarm/model-traces.jsonl")
        );
        assert_eq!(
            envs.get("SYMEM_QUEUE_TRACE_JSONL_PATH").map(String::as_str),
            Some("run/raw/judge-cache-prewarm/provider-queue/model-queue-traces.jsonl")
        );
    }

    #[test]
    fn parses_system_and_benchmark_flags() {
        let cli = Cli::parse_from([
            "membench",
            "--system",
            "symbiotic-memory",
            "--benchmark",
            "long-mem-eval",
            "--dataset",
            "dataset.json",
            "--run-root",
            "runs/symbiotic-memory/long-mem-eval/10/sample",
            "--limit",
            "50",
        ]);

        assert_eq!(cli.system.as_deref(), Some("symbiotic-memory"));
        assert_eq!(cli.benchmark.as_deref(), Some("long-mem-eval"));
        assert_eq!(cli.limit, 50);
        assert_eq!(cli.sample, "stratified");
        assert!(cli.command.is_none());
    }

    #[test]
    fn parses_short_system_and_benchmark_aliases() {
        let cli = Cli::parse_from([
            "membench",
            "--symbiotic-memory",
            "--long-mem-eval",
            "--dataset",
            "dataset.json",
            "--run-root",
            "runs/symbiotic-memory/long-mem-eval/10/sample",
        ]);

        assert!(cli.symbiotic_memory);
        assert!(cli.long_mem_eval);
        assert!(cli.command.is_none());
    }

    #[cfg(feature = "symbiotic-memory-adapter")]
    #[test]
    fn stratified_longmemeval_sample_round_robins_question_types() {
        use serde_json::Value;
        use symbiotic_mem_bench::symbiotic_memory_adapter::LongMemEvalRecord;

        fn row(question_id: &str, question_type: &str) -> LongMemEvalRecord {
            LongMemEvalRecord {
                question_id: question_id.to_string(),
                question_type: Some(question_type.to_string()),
                question: "q".to_string(),
                question_date: None,
                answer: Some(Value::String("a".to_string())),
                haystack_dates: Vec::new(),
                haystack_session_ids: Vec::new(),
                haystack_sessions: Vec::new(),
            }
        }

        let rows = vec![
            row("a1", "alpha"),
            row("a2", "alpha"),
            row("a3", "alpha"),
            row("b1", "beta"),
            row("b2", "beta"),
            row("c1", "gamma"),
        ];

        let selected = select_longmemeval_rows(rows, 4, "stratified").unwrap();
        let ids = selected
            .into_iter()
            .map(|row| row.question_id)
            .collect::<Vec<_>>();

        assert_eq!(ids, ["a1", "b1", "c1", "a2"]);
    }

    #[test]
    fn default_longmemeval_dataset_lives_under_ignored_runs_inputs() {
        let path = default_longmemeval_dataset_path();

        assert!(path.ends_with("runs/inputs/longmemeval-cleaned/longmemeval_s_cleaned.json"));
        assert!(path.starts_with(repo_root()));
    }

    #[test]
    fn explicit_missing_dataset_does_not_auto_download_arbitrary_path() {
        let err =
            resolve_longmemeval_dataset(Some(PathBuf::from("missing/custom-longmemeval.json")))
                .unwrap_err();

        assert!(err.to_string().contains("dataset does not exist"));
        assert!(err.to_string().contains("Omit --dataset to auto-download"));
    }

    #[test]
    fn rejects_conflicting_system_aliases() {
        let err = selected_value(
            Some("mem0"),
            true,
            "symbiotic-memory",
            "--system or --symbiotic-memory",
        )
        .unwrap_err();

        assert!(
            err.to_string()
                .contains("specified conflicting values: mem0 and symbiotic-memory")
        );
    }

    #[test]
    fn native_runs_are_fresh_by_default() {
        assert!(effective_fresh(false, false, false).unwrap());
        assert!(effective_fresh(false, false, true).unwrap());
        assert!(!effective_fresh(true, false, false).unwrap());
        assert!(!effective_fresh(false, true, false).unwrap());
        assert!(effective_fresh(true, false, true).is_err());
        assert!(effective_fresh(false, true, true).is_err());
    }

    #[test]
    fn paid_runs_are_default_and_local_smoke_is_explicit() {
        let cli = Cli::parse_from([
            "membench",
            "--system",
            "symbiotic-memory",
            "--benchmark",
            "long-mem-eval",
        ]);
        let score = enabled_by_default("score", cli.score, cli.no_score).unwrap();
        assert_eq!(cli.distiller, "llm");
        assert_eq!(cli.embedder, "openrouter");
        assert!(score);
        assert!(!is_ephemeral_native_smoke_run(
            &cli, false, "llm", "openrouter", score
        ));

        let smoke = Cli::parse_from([
            "membench",
            "--system",
            "symbiotic-memory",
            "--benchmark",
            "long-mem-eval",
            "--smoke",
        ]);
        assert!(smoke.smoke);
        assert!(is_ephemeral_native_smoke_run(
            &smoke,
            false,
            "heuristic",
            "hash",
            false
        ));
        assert!(!is_ephemeral_native_smoke_run(
            &smoke,
            true,
            "heuristic",
            "hash",
            false
        ));

        let keep = Cli::parse_from([
            "membench",
            "--system",
            "symbiotic-memory",
            "--benchmark",
            "long-mem-eval",
            "--smoke",
            "--keep-smoke-run",
        ]);
        assert!(!is_ephemeral_native_smoke_run(
            &keep,
            false,
            "heuristic",
            "hash",
            false
        ));

        let scored = Cli::parse_from([
            "membench",
            "--system",
            "symbiotic-memory",
            "--benchmark",
            "long-mem-eval",
            "--score",
        ]);
        let scored_score = enabled_by_default("score", scored.score, scored.no_score).unwrap();
        assert!(!is_ephemeral_native_smoke_run(
            &scored,
            false,
            "llm",
            "gemini",
            scored_score
        ));
    }

    #[test]
    fn gemini_embedder_rejects_openrouter_embedding_env() {
        let dir = tempfile::tempdir().unwrap();
        let env_file = dir.path().join(".env.test.local");
        std::fs::write(
            &env_file,
            "SYMEM_EMBED_OPERATOR=openrouter\nSYMEM_EMBED_MODEL=qwen/qwen3-embedding-8b\n",
        )
        .unwrap();
        let mut run = sample_run(None);
        run.env_file = Some(env_file);

        let err = validate_provider_role_selection(&run).unwrap_err();
        assert!(err.to_string().contains("--embedder openrouter"));
    }

    #[test]
    fn openrouter_embedder_accepts_qwen_embedding_env() {
        let dir = tempfile::tempdir().unwrap();
        let env_file = dir.path().join(".env.test.local");
        std::fs::write(
            &env_file,
            "SYMEM_EMBED_OPERATOR=openrouter\nSYMEM_EMBED_MODEL=qwen/qwen3-embedding-8b\n",
        )
        .unwrap();
        let mut run = sample_run(None);
        run.embedder = "openrouter".to_string();
        run.env_file = Some(env_file);

        validate_provider_role_selection(&run).unwrap();
    }

    #[test]
    fn smoke_run_root_stays_outside_dashboard_registry() {
        let runs = repo_root().join("runs");
        let root = default_smoke_run_root(&runs, "symbiotic-memory", "long-mem-eval", 10, "smoke");
        assert!(root.starts_with(runs.join(".tmp")));
        assert!(!root.starts_with(runs.join("symbiotic-memory")));
    }

    #[test]
    fn relative_paths_resolve_inside_bench_repo() {
        assert_eq!(
            resolve_repo_path(std::path::Path::new("runs")),
            repo_root().join("runs")
        );
        assert_eq!(
            resolve_repo_path(std::path::Path::new("records")),
            repo_root().join("records")
        );
    }

    #[test]
    fn native_runs_group_under_limit_segment() {
        let root = default_native_run_root(
            std::path::Path::new("runs"),
            "symbiotic-memory",
            "long-mem-eval",
            500,
            "20260617-120000-abcd1234",
        );

        assert_eq!(
            root,
            PathBuf::from("runs/symbiotic-memory/long-mem-eval/500/20260617-120000-abcd1234")
        );
    }

    #[test]
    fn imported_runs_group_under_scored_total_segment() {
        let dir = tempfile::tempdir().unwrap();
        let scored = dir.path().join("hyp.scored.json");
        std::fs::write(
            &scored,
            r#"{"counts":{"total_correct":45,"scored":50},"overall_accuracy":0.9}"#,
        )
        .unwrap();

        let root = default_import_run_root(
            std::path::Path::new("runs"),
            "symbiotic-memory",
            "long-mem-eval",
            &scored,
            "candidate",
        )
        .unwrap();

        assert_eq!(
            root,
            PathBuf::from("runs/symbiotic-memory/long-mem-eval/50/candidate")
        );
    }

    #[test]
    fn imported_report_writes_hard_numbers_and_run_params() {
        let dir = tempfile::tempdir().unwrap();
        let hyp = dir.path().join("hyp.jsonl");
        let verdicts = dir.path().join("hyp.verdicts.jsonl");
        let memory_traces = dir.path().join("memory-traces.jsonl");
        let model_traces = dir.path().join("model-traces.jsonl");
        let scored = dir.path().join("hyp.scored.json");
        let run_root = dir.path().join("report");
        std::fs::write(&hyp, "{\"question_id\":\"q1\",\"hypothesis\":\"a\"}\n").unwrap();
        std::fs::write(&verdicts, "{\"question_id\":\"q1\",\"label\":\"yes\"}\n").unwrap();
        std::fs::write(&memory_traces, "{\"trace_id\":\"m1\"}\n").unwrap();
        std::fs::write(&model_traces, "{\"trace_id\":\"p1\"}\n").unwrap();
        std::fs::write(
            &scored,
            r#"{"overall_accuracy":0.942,"task_averaged_accuracy":0.9375,"abstention_accuracy":0.8667,"counts":{"total_correct":471,"scored":500,"abstention_correct":26,"abstention_total":30}}"#,
        )
        .unwrap();

        import_benchmark_report(ImportedBenchmarkReport {
            system: "symbiotic-memory".to_string(),
            benchmark: "long-mem-eval".to_string(),
            run_root: run_root.clone(),
            run_name: "baseline-clean".to_string(),
            hypotheses: hyp,
            provenance: None,
            verdicts: Some(verdicts),
            partial_verdicts: None,
            memory_traces: Some(memory_traces),
            model_traces: Some(model_traces),
            scored,
        })
        .unwrap();

        let report: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(run_root.join("benchmark-report.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(report["schema"], "membench.report.v1");
        assert_eq!(report["system"], "symbiotic-memory");
        assert_eq!(report["benchmark"], "long-mem-eval");
        assert_eq!(report["run_kind"], "imported-artifact");
        assert_eq!(report["run_name"], "baseline-clean");
        assert_eq!(report["metrics"]["accuracy"]["correct"], 471);
        assert_eq!(report["metrics"]["accuracy"]["total"], 500);
        assert_eq!(report["metrics"]["accuracy"]["value"], 0.942);
        assert_eq!(report["artifacts"]["hypotheses"]["non_empty_lines"], 1);
        assert_eq!(report["artifacts"]["verdicts"]["non_empty_lines"], 1);
        assert_eq!(report["artifacts"]["memory_traces"]["non_empty_lines"], 1);
        assert_eq!(report["artifacts"]["model_traces"]["non_empty_lines"], 1);
        assert_eq!(report["artifact_manifest"]["native_state_available"], false);
        assert!(
            report["artifact_manifest"]["missing"]
                .as_array()
                .unwrap()
                .contains(&json!("provenance"))
        );

        let params: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(run_root.join("run-params.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(params["schema"], "membench.run_params.v1");
        assert_eq!(params["run_name"], "baseline-clean");
        assert_eq!(params["limit"], 500);
        assert_eq!(params["imported_artifacts"]["hypotheses"], true);
        assert_eq!(params["imported_artifacts"]["verdicts"], true);
        assert_eq!(params["imported_artifacts"]["scored"], true);
        assert_eq!(params["artifact_manifest"]["native_state_available"], false);
        assert!(run_root.join("artifacts/hypotheses.jsonl").exists());
        assert!(run_root.join("artifacts/verdicts.jsonl").exists());
        assert!(run_root.join("artifacts/memory-traces.jsonl").exists());
        assert!(run_root.join("artifacts/model-traces.jsonl").exists());
        assert!(run_root.join("artifacts/scored.json").exists());
    }

    #[test]
    fn native_provenance_indexes_routes_and_memory_traces() {
        let dir = tempfile::tempdir().unwrap();
        let run_root = dir.path().join("run");
        std::fs::create_dir_all(native_raw_dir(&run_root)).unwrap();
        let hypotheses = native_hypotheses_path(&run_root);
        let memory_traces = run_root.join("memory-traces.jsonl");
        std::fs::write(
            &hypotheses,
            r#"{"question_id":"q1","router_initial":"configured-policy","router_final":"configured-policy","router_reason":"single configured recall policy","debug_artifact":"vaults/q1/debug/hypotheses/hyp/question-debug.json"}"#,
        )
        .unwrap();
        std::fs::write(
            &memory_traces,
            concat!(
                r#"{"trace_id":"m1","source_id":"q1","operation":"capture"}"#,
                "\n",
                r#"{"trace_id":"m2","question_id":"q1","operation":"answer"}"#,
                "\n",
                r#"{"trace_id":"other","source_id":"q2","operation":"answer"}"#,
                "\n"
            ),
        )
        .unwrap();
        let mut run = sample_run(None);
        run.run_root = run_root.clone();
        run.run_name = "native-provenance".to_string();
        run.routed = true;
        run.query_planner = Some("flash".to_string());

        let provenance_path =
            write_native_provenance(&run, &hypotheses, Some(&memory_traces)).unwrap();
        assert_eq!(provenance_path, native_provenance_path(&run_root));
        let records = std::fs::read_to_string(provenance_path).unwrap();
        let record: serde_json::Value = serde_json::from_str(records.trim()).unwrap();

        assert_eq!(record["schema"], "membench.provenance.v1");
        assert_eq!(record["question_id"], "q1");
        assert_eq!(record["initial_pick"], "configured-policy");
        assert_eq!(record["final_pick"], "configured-policy");
        assert_eq!(record["router_reason"], "single configured recall policy");
        assert_eq!(record["query_planner"], "flash");
        assert_eq!(record["memory_trace_ids"], json!(["m1", "m2"]));
    }

    #[test]
    fn save_record_copies_normalized_run_to_records_root() {
        let dir = tempfile::tempdir().unwrap();
        let run_root = dir
            .path()
            .join("runs/symbiotic-memory/long-mem-eval/50/candidate");
        let records_root = dir.path().join("records");
        std::fs::create_dir_all(run_root.join("artifacts")).unwrap();
        std::fs::write(run_root.join("run-params.json"), "{}\n").unwrap();
        std::fs::write(
            run_root.join("benchmark-report.json"),
            r#"{"system":"symbiotic-memory","benchmark":"long-mem-eval","run_name":"candidate","run_params":{"limit":50},"metrics":{"accuracy":{"total":50,"value":0.9}}}"#,
        )
        .unwrap();
        std::fs::write(run_root.join("artifacts/hypotheses.jsonl"), "{}\n").unwrap();

        save_record(run_root, records_root.clone(), None, false).unwrap();

        let saved = records_root.join("symbiotic-memory/long-mem-eval/50/candidate");
        assert!(saved.join("benchmark-report.json").exists());
        assert!(saved.join("artifacts/hypotheses.jsonl").exists());
    }

    #[test]
    fn native_report_reads_raw_outputs_without_root_cleanup() {
        let dir = tempfile::tempdir().unwrap();
        let run_root = dir.path().join("run");
        std::fs::create_dir_all(native_raw_dir(&run_root)).unwrap();
        std::fs::create_dir_all(run_root.join("traces")).unwrap();
        std::fs::create_dir_all(run_root.join("vaults/q1")).unwrap();
        std::fs::write(
            native_hypotheses_path(&run_root),
            r#"{"question_id":"q1","hypothesis":"answer"}"#,
        )
        .unwrap();
        std::fs::write(
            run_root.join("traces/memory-events.jsonl"),
            r#"{"trace_id":"m1","question_id":"q1","operation":"answer"}"#,
        )
        .unwrap();

        let mut run = sample_run(None);
        run.run_root = run_root.clone();
        run.run_name = "native-configured-policy".to_string();

        write_native_benchmark_report(&run).unwrap();

        assert!(!run_root.join("hyp.jsonl").exists());
        assert!(!run_root.join("provenance.jsonl").exists());
        assert!(run_root.join("raw/hypotheses.jsonl").exists());
        assert!(run_root.join("raw/provenance.jsonl").exists());
        assert!(run_root.join("artifacts/hypotheses.jsonl").exists());
        assert!(run_root.join("artifacts/provenance.jsonl").exists());
        assert!(run_root.join("artifacts/memory-traces.jsonl").exists());
        assert!(run_root.join("vaults/q1").exists());
    }

    #[test]
    fn explorer_renders_report_numbers_and_params() {
        let report = json!({
            "schema": "membench.report.v1",
            "system": "symbiotic-memory",
            "benchmark": "long-mem-eval",
            "run_kind": "imported-artifact",
            "run_name": "baseline-clean",
            "metrics": {
                "accuracy": {"correct": 471, "total": 500, "value": 0.942},
                "task_averaged_accuracy": 0.9375,
                "abstention_accuracy": {"correct": 26, "total": 30, "value": 0.8667}
            },
            "run_params": {
                "system": "symbiotic-memory",
                "benchmark": "long-mem-eval",
                "run_kind": "imported-artifact",
                "run_name": "baseline-clean",
                "limit": 500
            },
            "artifact_manifest": {
                "available": ["hypotheses", "scored"],
                "missing": ["memory_traces", "model_traces"],
                "native_state_available": false,
                "native_state_note": "Imported artifact run."
            },
            "artifacts": {
                "hypotheses": {"path": "hyp.jsonl", "non_empty_lines": 500, "sha256": "abcdef1234567890"}
            }
        });

        let rendered = render_benchmark_report(&report);

        assert!(rendered.contains("accuracy: 471/500 = 0.942"));
        assert!(rendered.contains("task_averaged_accuracy: 0.938"));
        assert!(rendered.contains("run_name: baseline-clean"));
        assert!(rendered.contains("native_state_available: false"));
        assert!(rendered.contains("missing: memory_traces, model_traces"));
        assert!(rendered.contains("hypotheses: rows=500 sha256=abcdef123456"));
    }

    #[test]
    fn registry_list_summary_distinguishes_imports_and_native_runs() {
        let imported = json!({
            "native_state_available": false,
            "missing": ["provenance", "memory_traces"]
        });
        let native = json!({
            "native_state_available": true,
            "missing": []
        });

        assert_eq!(
            artifact_manifest_list_summary(Some(&imported)),
            "artifact-only missing=2"
        );
        assert_eq!(
            artifact_manifest_list_summary(Some(&native)),
            "native-state missing=none"
        );
        assert_eq!(artifact_manifest_list_summary(None), "artifacts=unknown");
    }

    #[test]
    fn plans_direct_symem_binary_when_provided() {
        let run = sample_run(Some(PathBuf::from("target/release/membench")));
        let plan = plan_symbiotic_memory_command(&run);

        assert_eq!(
            plan.program,
            std::ffi::OsString::from("target/release/membench")
        );
        let args = arg_strings(&plan);
        assert_eq!(args.first().map(String::as_str), Some("--symbiotic-memory"));
        assert_eq!(args.get(1).map(String::as_str), Some("--long-mem-eval"));
        assert!(!args.contains(&"run".to_string()));
        assert!(!args.contains(&"--manifest-path".to_string()));
        assert!(args.contains(&"--answerer".to_string()));
        assert!(args.contains(&"--answer-only".to_string()));
        assert!(args.contains(&"--resume".to_string()));
        assert!(!args.contains(&"--out".to_string()));
        assert!(!args.contains(&"../symbiotic-memory/prompts".to_string()));
    }

    #[test]
    fn plans_fresh_for_normal_native_run() {
        let mut run = sample_run(Some(PathBuf::from("target/release/symem")));
        run.answer_only = false;
        run.resume = false;
        run.fresh = true;

        let plan = plan_symbiotic_memory_command(&run);
        let args = arg_strings(&plan);

        assert!(!args.contains(&"--fresh".to_string()));
        assert!(!args.contains(&"--resume".to_string()));
        assert!(!args.contains(&"--answer-only".to_string()));
    }

    #[test]
    fn plans_cargo_manifest_fallback_without_symem_binary() {
        let run = sample_run(None);
        let plan = plan_symbiotic_memory_command(&run);

        assert_eq!(plan.program, std::ffi::OsString::from("cargo"));
        let args = arg_strings(&plan);
        assert_eq!(
            &args[..9],
            &[
                "run",
                "--release",
                "--features",
                "symbiotic-memory-adapter",
                "--bin",
                "membench",
                "--",
                "--symbiotic-memory",
                "--long-mem-eval",
            ]
        );
        assert!(args.contains(&"--query-planner".to_string()));
    }

    #[test]
    fn explicit_prompt_dir_overrides_manifest_default() {
        let mut run = sample_run(Some(PathBuf::from("target/release/symem")));
        run.prompt_dir = Some(PathBuf::from("custom-prompts"));

        let args = arg_strings(&plan_symbiotic_memory_command(&run));

        assert!(args.contains(&"custom-prompts".to_string()));
        assert!(!args.contains(&"../symbiotic-memory/prompts".to_string()));
    }

    #[test]
    fn plans_queued_score_command() {
        let mut run = sample_run(Some(PathBuf::from("target/release/membench")));
        run.score = true;
        run.oracle = Some(PathBuf::from("data/oracle.json"));
        run.judge_workers = 400;

        let plan = plan_symbiotic_memory_score_command(&run, run.oracle.as_ref().unwrap());
        let args = arg_strings(&plan);

        assert_eq!(args.first().map(String::as_str), Some("--symbiotic-memory"));
        assert!(args.contains(&"--score".to_string()));
        assert!(args.contains(&"--oracle".to_string()));
    }

    #[test]
    fn memory_config_is_recorded_and_exported() {
        let mut run = sample_run(Some(PathBuf::from("target/release/symem")));
        run.memory_config = Some(PathBuf::from(
            "config/symbiotic-memory/longmemeval-raw-light.yaml",
        ));

        let params = symbiotic_memory_run_params(&run);
        assert_eq!(
            params["memory_config"],
            serde_json::json!("config/symbiotic-memory/longmemeval-raw-light.yaml")
        );

        let mut cmd = std::process::Command::new("true");
        apply_symbiotic_memory_env(&run, &mut cmd).unwrap();
        let exported = cmd
            .get_envs()
            .find_map(|(key, value)| {
                (key == "SYMEM_CONFIG").then(|| value.map(|value| value.to_owned()))
            })
            .flatten();
        assert_eq!(
            exported.as_deref(),
            Some(std::ffi::OsStr::new(
                "config/symbiotic-memory/longmemeval-raw-light.yaml"
            ))
        );
    }

    #[test]
    fn native_run_params_record_default_deepseek_flash_judge() {
        let run = sample_run(None);
        let params = symbiotic_memory_run_params(&run);
        assert_eq!(params["answer_output"], serde_json::json!(true));
        assert_eq!(
            params["generative_answerer_enabled"],
            serde_json::json!(true)
        );
        assert_eq!(params["answerer"], serde_json::json!(true));
        assert_eq!(params["score_output"], serde_json::json!(true));
        assert_eq!(params["score"], serde_json::json!(true));
        assert_eq!(
            params["scorer"],
            serde_json::json!("queued-longmemeval-deepseek-v4-flash")
        );
        assert_eq!(params["judge_operator"], serde_json::json!("deepseek"));
        assert_eq!(
            params["judge_model"],
            serde_json::json!("deepseek-v4-flash")
        );
        assert_eq!(
            params["runtime_models"]["distill"],
            serde_json::json!("queued:configured-chat")
        );
        assert_eq!(
            params["runtime_models"]["embed"],
            serde_json::json!("queued:configured-embedding")
        );
        assert_eq!(
            params["runtime_models"]["answer"],
            serde_json::json!("queued:configured-chat")
        );
        assert_eq!(params["workflow_max_in_flight"], serde_json::json!(64));
        assert_eq!(params["provider_queue_available"], serde_json::json!(true));
        assert_eq!(params["workflow_queue_available"], serde_json::json!(true));
    }

    #[test]
    fn native_run_params_record_oracle_and_rerank_off_by_default() {
        // Empty env file isolates the assertion from any repo-root .env.test.local.
        let dir = tempfile::tempdir().unwrap();
        let env_file = dir.path().join(".env.test.local");
        std::fs::write(&env_file, "").unwrap();
        let mut run = sample_run(None);
        run.env_file = Some(env_file);

        let params = symbiotic_memory_run_params(&run);
        assert_eq!(params["oracle_gold"], serde_json::json!(false));
        assert_eq!(params["rerank"]["enabled"], serde_json::json!(false));
        assert!(params["configured_models"]["rerank"]["model"].is_null());
    }

    #[test]
    fn native_run_params_record_rerank_model_and_oracle_gold() {
        let dir = tempfile::tempdir().unwrap();
        let env_file = dir.path().join(".env.test.local");
        std::fs::write(
            &env_file,
            "SYMEM_ORACLE_GOLD=1\nSYMEM_RERANK=1\nSYMEM_RERANK_MODEL=cohere/rerank-4-fast\n",
        )
        .unwrap();
        let mut run = sample_run(None);
        run.env_file = Some(env_file);

        let params = symbiotic_memory_run_params(&run);
        assert_eq!(params["oracle_gold"], serde_json::json!(true));
        assert_eq!(params["rerank"]["enabled"], serde_json::json!(true));
        assert_eq!(
            params["rerank"]["model"],
            serde_json::json!("cohere/rerank-4-fast")
        );
        assert_eq!(
            params["rerank"]["operator"],
            serde_json::json!("openrouter")
        );
        // Folded into configured_models so the shared rerank lookup finds it.
        assert_eq!(
            params["configured_models"]["rerank"]["model"],
            serde_json::json!("cohere/rerank-4-fast")
        );
    }

    #[test]
    fn native_run_params_record_judge_env_override() {
        let dir = tempfile::tempdir().unwrap();
        let env_file = dir.path().join(".env.test.local");
        std::fs::write(
            &env_file,
            "SYMEM_JUDGE_OPERATOR=deepseek\nSYMEM_JUDGE_MODEL=deepseek-v4-pro\n",
        )
        .unwrap();
        let mut run = sample_run(None);
        run.env_file = Some(env_file);
        run.scorer = "queued-longmemeval-deepseek-v4-pro".to_string();

        let params = symbiotic_memory_run_params(&run);
        assert_eq!(params["judge_operator"], serde_json::json!("deepseek"));
        assert_eq!(params["judge_model"], serde_json::json!("deepseek-v4-pro"));
    }

    #[test]
    fn native_run_params_record_answer_model_env_override() {
        let dir = tempfile::tempdir().unwrap();
        let env_file = dir.path().join(".env.test.local");
        std::fs::write(
            &env_file,
            "SYMEM_ANSWER_OPERATOR=deepseek\nSYMEM_ANSWER_MODEL=deepseek-v4-pro\n",
        )
        .unwrap();
        let mut run = sample_run(None);
        run.env_file = Some(env_file);
        run.memory_config = Some(PathBuf::from(
            "config/symbiotic-memory/longmemeval-raw-light.yaml",
        ));

        let params = symbiotic_memory_run_params(&run);

        assert_eq!(
            params["configured_models"]["answer"]["model"],
            serde_json::json!("deepseek-v4-pro")
        );
        assert_eq!(
            params["runtime_models"]["answer"],
            serde_json::json!("queued:deepseek:deepseek-v4-pro")
        );
    }

    #[test]
    fn env_file_parser_accepts_export_and_quotes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".env.test.local");
        std::fs::write(
            &path,
            "export DEEPSEEK_API_KEY='abc'\nGEMINI_API_KEY=\"def\"\n# ignored\n",
        )
        .unwrap();

        let env = load_env_file(&path).unwrap();

        assert_eq!(env.get("DEEPSEEK_API_KEY").map(String::as_str), Some("abc"));
        assert_eq!(env.get("GEMINI_API_KEY").map(String::as_str), Some("def"));
    }
}
