use chrono::Utc;
use clap::{Parser, Subcommand};
#[cfg(feature = "symbiotic-memory-adapter")]
use futures::StreamExt;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
#[cfg(feature = "symbiotic-memory-adapter")]
use std::collections::VecDeque;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
#[cfg(feature = "symbiotic-memory-adapter")]
use std::sync::Arc;
#[cfg(feature = "symbiotic-memory-adapter")]
use std::time::Duration;
use symbiotic_mem_bench::{BenchQueueEvent, cost, registry, runner, summarize_queue_timing};

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
    #[arg(long, default_value = "gemini")]
    embedder: String,
    #[arg(long, default_value = "sqlite")]
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
    #[arg(long)]
    consolidate_briefs: bool,
    #[arg(long)]
    no_consolidate_briefs: bool,
    #[arg(long)]
    resume: bool,
    #[arg(long)]
    fresh: bool,
    #[arg(long, default_value = "scripted")]
    query_planner: Option<String>,
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
        };
    }

    run_selected_benchmark(cli)
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
            let routed = if cli.smoke {
                false
            } else {
                enabled_by_default("routed", cli.routed, cli.no_routed)?
            };
            let consolidate_briefs = if cli.smoke {
                false
            } else {
                enabled_by_default(
                    "consolidate-briefs",
                    cli.consolidate_briefs,
                    cli.no_consolidate_briefs,
                )?
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
            let fresh = effective_fresh(cli.resume, cli.answer_only, cli.fresh)?;
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
                consolidate_briefs,
                resume: cli.resume,
                fresh,
                query_planner: cli.query_planner,
                score,
                oracle: cli.oracle,
                judge_workers: cli.judge_workers,
                prewarm_judge_cache: cli.prewarm_judge_cache,
                prewarm_pause_secs: cli.prewarm_pause_secs,
                scorer: cli.scorer,
                env_file: cli.env_file,
                provider_queue_dir: cli.provider_queue_dir,
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

fn effective_fresh(resume: bool, answer_only: bool, explicit_fresh: bool) -> anyhow::Result<bool> {
    if resume && explicit_fresh {
        anyhow::bail!("choose either --resume or --fresh, not both");
    }
    if answer_only && explicit_fresh {
        anyhow::bail!(
            "--answer-only reuses an existing run root and cannot be combined with --fresh"
        );
    }
    Ok(!resume && !answer_only)
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
    let artifact_manifest = imported_artifact_manifest(&import);
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
        "artifact_manifest": artifact_manifest,
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
    let summaries: Vec<registry::RunSummary> = records.iter().map(registry::summarize).collect();
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
        "runtime_provider_note",
        "provider_queue_available",
        "workflow_queue_available",
        "ephemeral_smoke_run",
        "routed",
        "answer_only",
        "consolidate_briefs",
        "query_planner",
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
    consolidate_briefs: bool,
    resume: bool,
    fresh: bool,
    query_planner: Option<String>,
    score: bool,
    oracle: Option<PathBuf>,
    judge_workers: usize,
    prewarm_judge_cache: usize,
    prewarm_pause_secs: u64,
    scorer: String,
    env_file: Option<PathBuf>,
    provider_queue_dir: Option<PathBuf>,
    ephemeral_smoke_run: bool,
}

fn run_symbiotic_memory_longmemeval(run: SymbioticMemoryCliRun) -> anyhow::Result<()> {
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

#[cfg(feature = "symbiotic-memory-adapter")]
fn run_symbiotic_memory_longmemeval_native(run: SymbioticMemoryCliRun) -> anyhow::Result<()> {
    if run.fresh && run.run_root.exists() {
        std::fs::remove_dir_all(&run.run_root)?;
    }
    std::fs::create_dir_all(&run.run_root)?;
    write_run_params(&run.run_root, &symbiotic_memory_run_params(&run))?;

    let config = run
        .memory_config
        .as_ref()
        .map(symbiotic_memory::MemoryConfig::load_yaml)
        .transpose()?
        .unwrap_or_default();
    let provider_runtime = ProviderRuntime::new(&run, &config)?;
    let rows = symbiotic_mem_bench::symbiotic_memory_adapter::load_longmemeval(&run.dataset, None)?;
    let rows = select_longmemeval_rows(rows, run.limit, &run.sample)?;
    let mut policy = config.recall.clone();
    policy.answerer_enabled = run.answerer;
    if let Some(query_planner) = &run.query_planner {
        policy.query_planner = match query_planner.as_str() {
            "off" => symbiotic_memory::QueryPlannerMode::Off,
            "scripted" => symbiotic_memory::QueryPlannerMode::Scripted,
            "flash" => symbiotic_memory::QueryPlannerMode::Flash,
            other => anyhow::bail!("unknown --query-planner value: {other}"),
        };
    }
    symbiotic_mem_bench::symbiotic_memory_adapter::clear_score_artifacts(
        &run.run_root,
        native_hypotheses_path(&run.run_root),
    )?;
    let memory_trace_sink: Option<std::sync::Arc<dyn symbiotic_memory::MemoryTraceSink>> = Some(
        std::sync::Arc::new(symbiotic_memory::JsonlMemoryTraceSink::open(
            run.run_root.join("traces").join("memory-events.jsonl"),
        )?),
    );
    let runtime = tokio::runtime::Runtime::new()?;
    let hypotheses_path = native_hypotheses_path(&run.run_root);
    if let Some(parent) = hypotheses_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if run.store == "sqlite" {
        let embedder_factory = provider_runtime.embedding_factory(&run)?;
        let distiller_factory = provider_runtime.distiller_factory(&run)?;
        let answer_factory = provider_runtime.answer_factory(&run)?;
        let planner_factory = provider_runtime.query_planner_factory(&run)?;
        runtime.block_on(
            symbiotic_mem_bench::symbiotic_memory_adapter::run_longmemeval_sqlite_with_planner(
                &rows,
                &run.run_root,
                move || embedder_factory(),
                move || distiller_factory(),
                None,
                move || answer_factory(),
                planner_factory,
                None,
                memory_trace_sink,
                policy,
                hypotheses_path.clone(),
                run.routed,
                run.answer_only,
                run.consolidate_briefs,
                Some(config.queue.workflow_max_in_flight),
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
    write_native_benchmark_report(&run)?;
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
struct ProviderRuntime {
    config: symbiotic_memory::MemoryConfig,
    queue_registry: symbiotic_memory::QueueRegistry,
    queue_store: Arc<dyn symbiotic_memory::QueueEventStore>,
    response_cache_root: PathBuf,
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
        let queue_store: Arc<dyn symbiotic_memory::QueueEventStore> =
            Arc::new(symbiotic_memory::JsonlQueueEventStore::open(
                provider_queue_dir.join("model-queue-traces.jsonl"),
            )?);
        Ok(Self {
            config: config.clone(),
            queue_registry: symbiotic_memory::QueueRegistry::new(),
            queue_store,
            response_cache_root: run_env_value(run, "SYMEM_RESPONSE_CACHE_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| provider_queue_dir.join("responses")),
        })
    }

    fn embedding_factory(
        &self,
        run: &SymbioticMemoryCliRun,
    ) -> anyhow::Result<Arc<dyn Fn() -> Arc<dyn symbiotic_memory::EmbeddingProvider> + Send + Sync>>
    {
        match run.embedder.as_str() {
            "hash" => Ok(Arc::new(|| {
                Arc::new(symbiotic_memory::CachedEmbeddingProvider::new(
                    symbiotic_memory::HashEmbeddingProvider::default(),
                    "hash-membench",
                )) as Arc<dyn symbiotic_memory::EmbeddingProvider>
            })),
            "gemini" => {
                let adapter = self.role_adapter(run, "EMBED", &self.config.providers.embedding);
                let queue = self.provider_queue(&adapter)?;
                let api_key = required_env(run, "GEMINI_API_KEY")?;
                let model = adapter.model.clone();
                let dims = run_env_value(run, "SYMEM_EMBED_DIMS")
                    .and_then(|value| value.parse::<usize>().ok())
                    .unwrap_or(3072);
                let max_chars = run_env_value(run, "SYMEM_EMBED_BATCH_MAX_CHARS")
                    .or_else(|| run_env_value(run, "SYMEM_EMBED_MAX_CHARS"))
                    .and_then(|value| value.parse::<usize>().ok())
                    .unwrap_or(12_000);
                let transport = gemini_transport_mode(run);
                Ok(Arc::new(move || {
                    let provider = symbiotic_memory::providers::GeminiEmbeddingProvider::new(
                        api_key.clone(),
                        model.clone(),
                        dims,
                        symbiotic_memory::providers::GeminiEmbeddingTaskMode::Document,
                    )
                    .with_transport_mode(transport.clone())
                    .with_max_chars(max_chars);
                    Arc::new(symbiotic_memory::CachedEmbeddingProvider::new(
                        symbiotic_memory::providers::QueuedEmbeddingProvider::new(
                            provider,
                            queue.clone(),
                        ),
                        format!("gemini:{model}:{dims}:document"),
                    )) as Arc<dyn symbiotic_memory::EmbeddingProvider>
                }))
            }
            other => anyhow::bail!("unknown --embedder value: {other}; expected hash or gemini"),
        }
    }

    fn distiller_factory(
        &self,
        run: &SymbioticMemoryCliRun,
    ) -> anyhow::Result<Arc<dyn Fn() -> DynDistiller + Send + Sync>> {
        match run.distiller.as_str() {
            "heuristic" => Ok(Arc::new(|| {
                DynDistiller(Arc::new(symbiotic_memory::HeuristicDistiller))
            })),
            "llm" => {
                let prompt = load_memory_prompt(run, &run.distill_prompt)?;
                let chat_factory =
                    self.chat_factory(run, "DISTILL", &self.config.providers.distill)?;
                let turns_per_window = run_env_value(run, "SYMEM_DISTILL_TURNS_PER_WINDOW")
                    .and_then(|value| value.parse::<usize>().ok())
                    .unwrap_or(16);
                Ok(Arc::new(move || {
                    let llm = symbiotic_memory::LlmDistiller::new(chat_factory(), prompt.clone());
                    DynDistiller(Arc::new(symbiotic_memory::WindowedDistiller::new(
                        llm,
                        turns_per_window,
                    )))
                }))
            }
            other => anyhow::bail!("unknown --distiller value: {other}; expected heuristic or llm"),
        }
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

    fn query_planner_factory(
        &self,
        run: &SymbioticMemoryCliRun,
    ) -> anyhow::Result<
        Option<Arc<dyn Fn() -> Arc<dyn symbiotic_memory::recall::QueryPlanner> + Send + Sync>>,
    > {
        if run.query_planner.as_deref() != Some("flash") {
            return Ok(None);
        }
        let adapter = symbiotic_memory::ProviderAdapterConfig::new(
            "chat",
            run_env_value(run, "SYMEM_QUERY_PLANNER_OPERATOR")
                .unwrap_or_else(|| "deepseek".to_string()),
            run_env_value(run, "SYMEM_QUERY_PLANNER_MODEL")
                .unwrap_or_else(|| "deepseek-v4-flash".to_string()),
        );
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
                .with_pricing(symbiotic_memory::providers::ProviderPricing {
                    input_token_micro_usd: resolved.pricing.input_token_micro_usd,
                    cached_input_token_micro_usd: resolved.pricing.cached_input_token_micro_usd,
                    output_token_micro_usd: resolved.pricing.output_token_micro_usd,
                });
        Ok(
            symbiotic_memory::providers::ProviderQueue::from_queue(provider_config, queue)
                .with_response_cache(
                    self.response_cache_root
                        .join(sanitize_path_component(&resolved.queue_id)),
                ),
        )
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
        // Judging is a strict YES/NO classification task. Keeping thinking off
        // avoids long hidden generations and makes cache/cost behavior stable.
        "JUDGE" => Some(symbiotic_memory::providers::ThinkingMode::Disabled),
        _ => None,
    }
}

#[cfg(feature = "symbiotic-memory-adapter")]
fn default_role_max_tokens(role: &str) -> Option<u32> {
    match role {
        "JUDGE" => Some(64),
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
    let judge_prompt_mode = run_env_value(run, "SYMEM_JUDGE_PROMPT_MODE")
        .unwrap_or_else(|| "semantic-shared-compact".to_string());
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
    let (system, user) = judge_prompt(prompt_mode, &item.question, &answer, &item.hypothesis);
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
    question: &str,
    answer: &str,
    hypothesis: &str,
) -> (String, String) {
    let system = match prompt_mode {
        "official" => {
            "You are an evaluator for LongMemEval. Decide whether the candidate answer correctly answers the question according to the reference answer. Return exactly YES or NO."
        }
        _ => {
            "You are an evaluator for LongMemEval. Return exactly YES or NO. Mark YES when the candidate answer is semantically equivalent to the reference, is a directly inferable phrasing, or contains the required value with harmless extra words. Mark NO when it contradicts the reference, omits the requested value, substitutes a different value, or says unavailable when the reference contains the answer."
        }
    };
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

#[cfg_attr(not(feature = "symbiotic-memory-adapter"), allow(dead_code))]
fn symbiotic_memory_run_params(run: &SymbioticMemoryCliRun) -> serde_json::Value {
    let judge = resolved_judge_params(run);
    let configured_models = configured_provider_models(run);
    let runtime_models = runtime_provider_bindings(run, &judge);
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
        "resume": run.resume,
        "fresh": run.fresh,
        "query_planner": run.query_planner,
        "score_output": run.score,
        "score": run.score,
        "oracle": run.oracle.as_deref().map(portable_path),
        "judge_workers": run.judge_workers,
        "prewarm_judge_cache": run.prewarm_judge_cache,
        "prewarm_pause_secs": run.prewarm_pause_secs,
        "scorer": run.scorer,
        "judge_operator": judge.operator,
        "judge_model": judge.model,
        "env_file": run.env_file.as_deref().map(portable_path),
        "provider_queue_dir": run.provider_queue_dir.as_deref().map(portable_path),
    });
    let object = params
        .as_object_mut()
        .expect("run params JSON must be an object");
    object.insert("configured_models".to_string(), configured_models);
    object.insert("runtime_models".to_string(), runtime_models);
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
fn configured_provider_models(run: &SymbioticMemoryCliRun) -> serde_json::Value {
    #[cfg(feature = "symbiotic-memory-adapter")]
    {
        if let Some(path) = &run.memory_config
            && let Ok(config) = symbiotic_memory::MemoryConfig::load_yaml(path)
        {
            return json!({
                "distill": provider_binding(&config.providers.distill),
                "answer": provider_binding(&config.providers.answer),
                "embed": provider_binding(&config.providers.embedding),
                "chat_provider": config.providers.chat_provider,
                "chat_model": config.providers.chat_model,
                "embedding_provider": config.providers.embedding_provider,
                "embedding_model": config.providers.embedding_model,
                "prompt_cache": config.providers.prompt_cache,
            });
        }
    }
    json!({
        "distill": run.distiller,
        "answer": if run.answerer { "configured-by-adapter" } else { "disabled" },
        "embed": run.embedder,
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

#[cfg_attr(not(feature = "symbiotic-memory-adapter"), allow(dead_code))]
fn runtime_provider_bindings(
    run: &SymbioticMemoryCliRun,
    judge: &ResolvedJudgeParams,
) -> serde_json::Value {
    #[cfg(feature = "symbiotic-memory-adapter")]
    if let Some(path) = &run.memory_config
        && let Ok(config) = symbiotic_memory::MemoryConfig::load_yaml(path)
    {
        return json!({
            "distill": if run.distiller == "llm" {
                format!(
                    "queued:{}:{}",
                    config.providers.distill.operator, config.providers.distill.model
                )
            } else {
                "local:heuristic-v1".to_string()
            },
            "embed": if run.embedder == "gemini" {
                format!(
                    "queued:{}:{}",
                    config.providers.embedding.operator, config.providers.embedding.model
                )
            } else {
                "local:hash-embedding-v1".to_string()
            },
            "answer": if run.answerer {
                format!(
                    "queued:{}:{}",
                    config.providers.answer.operator, config.providers.answer.model
                )
            } else {
                "local:extractive-answer".to_string()
            },
            "judge": if run.score {
                format!("queued:{}:{}", judge.operator, judge.model)
            } else {
                "not-run".to_string()
            },
        });
    }
    json!({
        "distill": if run.distiller == "llm" { "queued:configured-chat" } else { "local:heuristic-v1" },
        "embed": if run.embedder == "gemini" { "queued:configured-embedding" } else { "local:hash-embedding-v1" },
        "answer": if run.answerer {
            "queued:configured-chat"
        } else {
            "local:extractive-answer"
        },
        "judge": if run.score {
            format!("queued:{}:{}", judge.operator, judge.model)
        } else {
            "not-run".to_string()
        },
    })
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
    set_env_default(cmd, "SYMEM_DISTILL_OPERATOR", "deepseek");
    set_env_default(cmd, "SYMEM_DISTILL_BASE_URL", "https://api.deepseek.com");
    set_env_default(cmd, "SYMEM_DISTILL_MODEL", "deepseek-v4-flash");
    set_env_default(cmd, "SYMEM_ANSWER_OPERATOR", "deepseek");
    set_env_default(cmd, "SYMEM_ANSWER_BASE_URL", "https://api.deepseek.com");
    set_env_default(cmd, "SYMEM_ANSWER_MODEL", "deepseek-v4-pro");
    set_env_default(cmd, "SYMEM_QUERY_PLANNER_OPERATOR", "deepseek");
    set_env_default(
        cmd,
        "SYMEM_QUERY_PLANNER_BASE_URL",
        "https://api.deepseek.com",
    );
    set_env_default(cmd, "SYMEM_TEMPORAL_ANSWER_OPERATOR", "deepseek");
    set_env_default(
        cmd,
        "SYMEM_TEMPORAL_ANSWER_BASE_URL",
        "https://api.deepseek.com",
    );
    set_env_default(cmd, "SYMEM_JUDGE_OPERATOR", "deepseek");
    set_env_default(cmd, "SYMEM_JUDGE_BASE_URL", "https://api.deepseek.com");
    set_env_default(cmd, "SYMEM_JUDGE_MODEL", "deepseek-v4-flash");
    set_env_default(cmd, "SYMEM_JUDGE_THINKING", "disabled");
    set_env_default(cmd, "SYMEM_JUDGE_MAX_TOKENS", "64");
    set_env_default(cmd, "SYMEM_EMBED_MODEL", "gemini-embedding-2");
    set_env_default(cmd, "SYMEM_EMBED_DIMS", "3072");
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
        consolidate_briefs: run.consolidate_briefs,
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
            store: "sqlite".to_string(),
            prompt_dir: None,
            distill_prompt: "distill".to_string(),
            answerer: true,
            routed: false,
            answer_only: true,
            consolidate_briefs: false,
            resume: true,
            fresh: false,
            query_planner: Some("off".to_string()),
            score: true,
            oracle: None,
            judge_workers: 400,
            prewarm_judge_cache: 0,
            prewarm_pause_secs: 10,
            scorer: "queued-longmemeval-deepseek-v4-flash".to_string(),
            env_file: None,
            provider_queue_dir: None,
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
        assert_eq!(cli.embedder, "gemini");
        assert!(score);
        assert!(!is_ephemeral_native_smoke_run(
            &cli, false, "llm", "gemini", score
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
            r#"{"question_id":"q1","router_initial":"raw-with-facts","router_final":"direct-raw","router_reason":"fallback after low fact support","debug_artifact":"vaults/q1/debug/hypotheses/hyp/question-debug.json"}"#,
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
        run.query_planner = Some("scripted".to_string());

        let provenance_path =
            write_native_provenance(&run, &hypotheses, Some(&memory_traces)).unwrap();
        assert_eq!(provenance_path, native_provenance_path(&run_root));
        let records = std::fs::read_to_string(provenance_path).unwrap();
        let record: serde_json::Value = serde_json::from_str(records.trim()).unwrap();

        assert_eq!(record["schema"], "membench.provenance.v1");
        assert_eq!(record["question_id"], "q1");
        assert_eq!(record["initial_pick"], "raw-with-facts");
        assert_eq!(record["final_pick"], "direct-raw");
        assert_eq!(record["router_reason"], "fallback after low fact support");
        assert_eq!(record["query_planner"], "scripted");
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
        run.run_name = "native-direct-raw".to_string();

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
            &args[..8],
            &[
                "run",
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
        assert_eq!(params["provider_queue_available"], serde_json::json!(true));
        assert_eq!(params["workflow_queue_available"], serde_json::json!(true));
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
