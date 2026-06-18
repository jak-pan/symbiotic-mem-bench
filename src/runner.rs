//! Dashboard command planning for Symbiotic Memory runs.
//!
//! The default `membench` execution path is the paid provider-backed benchmark
//! shape. The no-network local deterministic path is an explicit
//! smoke mode. This module is the typed preview/schema surface used by the
//! dashboard; previews should target `membench`, omit internal paths, and avoid
//! hiding whether a run is scored.

use crate::jsonutil::{nested_bool, nested_str, nested_u64};
use serde::Serialize;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

/// A planned external command: program + argument vector.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct PlannedCommand {
    pub program: String,
    pub args: Vec<String>,
}

impl PlannedCommand {
    /// Render as a copy-pasteable shell line.
    pub fn to_shell(&self) -> String {
        let mut parts = vec![shell_quote(&self.program)];
        parts.extend(self.args.iter().map(|arg| shell_quote(arg)));
        parts.join(" ")
    }
}

fn shell_quote(value: &str) -> String {
    if value.is_empty() {
        return "''".to_string();
    }
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '/' | '=' | ':'))
    {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

/// All inputs needed to preview a Symbiotic Memory LongMemEval run.
#[derive(Clone, Debug)]
pub struct SymemRunPlan {
    pub repo_root: PathBuf,
    pub run_root: PathBuf,
    pub run_root_explicit: bool,
    pub dataset: PathBuf,
    pub dataset_explicit: bool,
    pub limit: usize,
    pub sample: String,
    pub distiller: String,
    pub embedder: String,
    pub store: String,
    pub prompt_dir: Option<PathBuf>,
    pub distill_prompt: String,
    pub answerer: bool,
    pub routed: bool,
    pub answer_only: bool,
    pub consolidate_briefs: bool,
    pub resume: bool,
    pub fresh: bool,
    pub query_planner: Option<String>,
    pub score: bool,
    pub oracle: Option<PathBuf>,
    pub judge_workers: usize,
    pub prewarm_judge_cache: usize,
    pub prewarm_pause_secs: u64,
    pub scorer: String,
    pub symem_bin: Option<PathBuf>,
    pub memory_manifest: PathBuf,
    pub memory_manifest_explicit: bool,
    pub memory_config: Option<PathBuf>,
    pub smoke: bool,
}

fn path_string(path: &Path) -> String {
    path.display().to_string()
}

impl SymemRunPlan {
    fn path_arg(&self, path: &Path) -> String {
        path.strip_prefix(&self.repo_root)
            .unwrap_or(path)
            .display()
            .to_string()
    }

    fn is_default_dataset(&self) -> bool {
        let rendered = self.path_arg(&self.dataset);
        rendered.is_empty()
            || rendered == "runs/inputs/longmemeval-cleaned/longmemeval_s_cleaned.json"
    }

    /// Arguments for the native `membench` LongMemEval command.
    pub fn membench_args(&self) -> Vec<String> {
        let mut args = vec![
            "--symbiotic-memory".to_string(),
            "--long-mem-eval".to_string(),
        ];
        if self.dataset_explicit && !self.is_default_dataset() {
            args.push("--dataset".to_string());
            args.push(self.path_arg(&self.dataset));
        }
        if self.run_root_explicit {
            args.push("--run-root".to_string());
            args.push(self.path_arg(&self.run_root));
        }
        if self.limit != 10 {
            args.push("--limit".to_string());
            args.push(self.limit.to_string());
        }
        if self.sample != "stratified" {
            args.push("--sample".to_string());
            args.push(self.sample.clone());
        }
        if self.memory_manifest_explicit
            && self.path_arg(&self.memory_manifest) != "../symbiotic-memory/Cargo.toml"
        {
            args.push("--memory-manifest".to_string());
            args.push(self.path_arg(&self.memory_manifest));
        }
        if let Some(memory_config) = &self.memory_config {
            args.push("--memory-config".to_string());
            args.push(self.path_arg(memory_config));
        }
        if self.smoke {
            args.push("--smoke".to_string());
        } else {
            if self.distiller != "llm" {
                args.push("--distiller".to_string());
                args.push(self.distiller.clone());
            }
            if self.embedder != "gemini" {
                args.push("--embedder".to_string());
                args.push(self.embedder.clone());
            }
            if self.store != "sqlite" {
                args.push("--store".to_string());
                args.push(self.store.clone());
            }
            if let Some(prompt_dir) = &self.prompt_dir {
                args.push("--prompt-dir".to_string());
                args.push(self.path_arg(prompt_dir));
            }
            if self.distill_prompt != "distill" {
                args.push("--distill-prompt".to_string());
                args.push(self.distill_prompt.clone());
            }
            if self.answerer {
                args.push("--answerer".to_string());
            }
            if self.routed {
                args.push("--routed".to_string());
            }
        }
        if self.answer_only {
            args.push("--answer-only".to_string());
        }
        if !self.smoke && self.consolidate_briefs {
            args.push("--consolidate-briefs".to_string());
        }
        if self.resume {
            args.push("--resume".to_string());
        }
        if !self.smoke
            && let Some(query_planner) = &self.query_planner
        {
            args.push("--query-planner".to_string());
            args.push(query_planner.clone());
        }
        if self.smoke {
            // `--smoke` is the public shorthand for local no-network/no-score.
        } else if self.score {
            args.push("--score".to_string());
            if let Some(oracle) = &self.oracle {
                args.push("--oracle".to_string());
                args.push(self.path_arg(oracle));
            }
            if self.judge_workers != 400 {
                args.push("--judge-workers".to_string());
                args.push(self.judge_workers.to_string());
            }
            if self.prewarm_judge_cache > 0 {
                args.push("--prewarm-judge-cache".to_string());
                args.push(self.prewarm_judge_cache.to_string());
                args.push("--prewarm-pause-secs".to_string());
                args.push(self.prewarm_pause_secs.to_string());
            }
            if self.scorer != "queued-longmemeval-deepseek-v4-flash" {
                args.push("--scorer".to_string());
                args.push(self.scorer.clone());
            }
        } else {
            args.push("--no-score".to_string());
        }
        args
    }

    fn wrap_membench_command(&self, membench_args: Vec<String>) -> PlannedCommand {
        if let Some(symem_bin) = &self.symem_bin {
            return PlannedCommand {
                program: path_string(symem_bin),
                args: membench_args,
            };
        }
        let mut args = vec![
            "run".to_string(),
            "--features".to_string(),
            "symbiotic-memory-adapter".to_string(),
            "--bin".to_string(),
            "membench".to_string(),
            "--".to_string(),
        ];
        args.extend(membench_args);
        PlannedCommand {
            program: "cargo".to_string(),
            args,
        }
    }

    /// The primary run command preview.
    pub fn run_command(&self) -> PlannedCommand {
        self.wrap_membench_command(self.membench_args())
    }

    /// Scoring is now part of the primary `membench` command when supported.
    pub fn score_command(&self) -> Option<PlannedCommand> {
        None
    }

    /// The planned cache-prewarm score command preview. It is intentionally
    /// separate from the real score command so callers can point it at temporary
    /// files after the scorer is ported.
    pub fn prewarm_score_command(
        &self,
        _hypotheses: &Path,
        _oracle: &Path,
        _workers: usize,
    ) -> PlannedCommand {
        self.run_command()
    }

    /// Non-secret environment defaults the CLI applies (only when the caller has
    /// not already set them). Mirrors the CLI's env setup, minus the `.env`
    /// secret file.
    pub fn env_defaults(&self) -> Vec<(String, String)> {
        Vec::new()
    }

    /// Pre-flight warnings: inputs the run needs that are missing on disk.
    pub fn warnings(&self) -> Vec<String> {
        let mut warnings = Vec::new();
        if self.dataset_explicit && !self.dataset.exists() {
            warnings.push(format!("dataset not found: {}", path_string(&self.dataset)));
        }
        if let Some(symem_bin) = &self.symem_bin {
            if !symem_bin.exists() {
                warnings.push(format!(
                    "membench binary not found: {} (will fall back to cargo build)",
                    path_string(symem_bin)
                ));
            }
        } else if self.memory_manifest_explicit && !self.memory_manifest.exists() {
            warnings.push(format!(
                "memory manifest not found: {}",
                path_string(&self.memory_manifest)
            ));
        }
        if self.smoke {
            return warnings;
        }
        if self.score {
            if let Some(oracle) = &self.oracle
                && !oracle.exists()
            {
                warnings.push(format!("oracle not found: {}", path_string(oracle)));
            }
        }
        if let Some(memory_config) = &self.memory_config
            && !memory_config.exists()
        {
            warnings.push(format!(
                "memory config not found: {}",
                path_string(memory_config)
            ));
        }
        warnings
    }
}

/// Serializable preview of a planned run for the dashboard.
#[derive(Clone, Debug, Serialize)]
pub struct RunnerPreview {
    pub run_root: String,
    pub run_command: PlannedCommand,
    pub run_shell: String,
    pub score_command: Option<PlannedCommand>,
    pub score_shell: Option<String>,
    pub env_defaults: Vec<(String, String)>,
    pub warnings: Vec<String>,
}

impl SymemRunPlan {
    pub fn preview(&self) -> RunnerPreview {
        let score_command = self.score_command();
        RunnerPreview {
            run_root: path_string(&self.run_root),
            run_shell: self.run_command().to_shell(),
            run_command: self.run_command(),
            score_shell: score_command.as_ref().map(PlannedCommand::to_shell),
            score_command,
            env_defaults: self.env_defaults(),
            warnings: self.warnings(),
        }
    }
}

/// One tunable parameter, described for a dynamic form.
#[derive(Clone, Debug, Serialize)]
pub struct ParamField {
    pub name: String,
    pub label: String,
    /// `path` | `int` | `bool` | `enum` | `string`.
    pub kind: String,
    pub default: Value,
    pub options: Vec<String>,
    pub group: String,
    pub help: String,
    pub required: bool,
}

#[allow(clippy::too_many_arguments)]
fn field(
    name: &str,
    label: &str,
    kind: &str,
    default: Value,
    options: &[&str],
    group: &str,
    help: &str,
    required: bool,
) -> ParamField {
    ParamField {
        name: name.to_string(),
        label: label.to_string(),
        kind: kind.to_string(),
        default,
        options: options.iter().map(|opt| opt.to_string()).collect(),
        group: group.to_string(),
        help: help.to_string(),
        required,
    }
}

/// The base parameter schema for the Symbiotic Memory LongMemEval runner. The
/// server may enrich `options` with values observed in existing runs.
///
/// Provider-backed execution fields remain here so the dashboard can keep a
/// stable form while the native provider/scorer port moves from `symem` into
/// `membench`.
pub fn symem_param_schema() -> Vec<ParamField> {
    vec![
        field(
            "dataset",
            "Dataset",
            "path",
            Value::Null,
            &[],
            "Inputs",
            "LongMemEval dataset JSON path.",
            true,
        ),
        field(
            "oracle",
            "Oracle",
            "path",
            Value::Null,
            &[],
            "Inputs",
            "Oracle JSON for scoring (often the same as dataset).",
            false,
        ),
        field(
            "limit",
            "Limit",
            "int",
            json!(10),
            &[],
            "Inputs",
            "Number of questions. Use --smoke for local no-network checks; 10/50/500 are provider-backed sizes.",
            true,
        ),
        field(
            "sample",
            "Sample",
            "enum",
            json!("stratified"),
            &["stratified", "first"],
            "Inputs",
            "Question selection strategy for small runs.",
            false,
        ),
        field(
            "symem_bin",
            "membench binary",
            "path",
            Value::Null,
            &[],
            "Inputs",
            "Optional prebuilt membench binary. Leave empty to use cargo run.",
            false,
        ),
        field(
            "smoke",
            "Smoke mode",
            "bool",
            json!(false),
            &[],
            "Inputs",
            "Run local no-network smoke mode. Internally uses deterministic local providers and no scorer.",
            false,
        ),
        field(
            "memory_manifest",
            "Memory manifest",
            "path",
            json!("../symbiotic-memory/Cargo.toml"),
            &[],
            "Inputs",
            "Cargo.toml of the system under test.",
            false,
        ),
        field(
            "memory_config",
            "Memory config",
            "path",
            json!("config/symbiotic-memory/longmemeval-raw-light.yaml"),
            &[],
            "Memory",
            "YAML profile, e.g. longmemeval-raw-light.yaml.",
            false,
        ),
        field(
            "distiller",
            "Distiller",
            "enum",
            json!("llm"),
            &["heuristic", "llm"],
            "Memory",
            "Distillation strategy. Default is the paid provider-backed path.",
            false,
        ),
        field(
            "embedder",
            "Embedder",
            "enum",
            json!("gemini"),
            &["hash", "gemini"],
            "Memory",
            "Embedding backend. Default is the paid provider-backed path.",
            false,
        ),
        field(
            "store",
            "Store",
            "enum",
            json!("sqlite"),
            &["sqlite"],
            "Memory",
            "Vector/state store.",
            false,
        ),
        field(
            "query_planner",
            "Query planner",
            "enum",
            json!("scripted"),
            &["off", "scripted"],
            "Memory",
            "Retrieval query planner.",
            false,
        ),
        field(
            "answerer",
            "Answerer",
            "bool",
            json!(true),
            &[],
            "Memory",
            "Enable the memory engine's generative answerer policy; LongMemEval still writes hypotheses either way.",
            false,
        ),
        field(
            "routed",
            "Routed",
            "bool",
            json!(true),
            &[],
            "Memory",
            "Use routed retrieval.",
            false,
        ),
        field(
            "consolidate_briefs",
            "Consolidate briefs",
            "bool",
            json!(true),
            &[],
            "Memory",
            "Create source-backed chronological brief facts after base ingest.",
            false,
        ),
        field(
            "distill_prompt",
            "Distill prompt",
            "string",
            json!("distill"),
            &[],
            "Memory",
            "Distillation prompt name.",
            false,
        ),
        field(
            "score",
            "Score",
            "bool",
            json!(true),
            &[],
            "Scoring",
            "Run the judge after answering. Default benchmark launches are paid and scored.",
            false,
        ),
        field(
            "judge_workers",
            "Judge workers",
            "int",
            json!(400),
            &[],
            "Scoring",
            "Judge fan-out (capped by queue concurrency).",
            false,
        ),
        field(
            "prewarm_judge_cache",
            "Prewarm judge cache",
            "int",
            json!(0),
            &[],
            "Scoring",
            "Run a small same-prompt score batch before the real score; 0 disables.",
            false,
        ),
        field(
            "prewarm_pause_secs",
            "Prewarm pause seconds",
            "int",
            json!(10),
            &[],
            "Scoring",
            "Pause after judge cache prewarm before the real score.",
            false,
        ),
        field(
            "scorer",
            "Scorer",
            "string",
            json!("queued-longmemeval-deepseek-v4-flash"),
            &[],
            "Scoring",
            "Scorer id.",
            false,
        ),
        field(
            "answer_only",
            "Answer only",
            "bool",
            json!(false),
            &[],
            "Lifecycle",
            "Reuse an ingested run root and only re-answer.",
            false,
        ),
        field(
            "resume",
            "Resume",
            "bool",
            json!(false),
            &[],
            "Lifecycle",
            "Continue an interrupted run root.",
            false,
        ),
    ]
}

/// Build a [`SymemRunPlan`] from loosely typed params (the dashboard form),
/// applying the same defaults the CLI uses. `repo_root` anchors a default run
/// root when none is supplied.
pub fn plan_from_params(params: &Value, repo_root: &Path) -> SymemRunPlan {
    let str_field = |key: &str, default: &str| -> String {
        nested_str(params, &[key]).unwrap_or(default).to_string()
    };
    let opt_path = |key: &str| -> Option<PathBuf> {
        nested_str(params, &[key])
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
    };
    let bool_field = |key: &str, default: bool| nested_bool(params, &[key]).unwrap_or(default);

    let limit = nested_u64(params, &["limit"]).unwrap_or(10) as usize;
    let resume = bool_field("resume", false);
    let answer_only = bool_field("answer_only", false);
    let run_root_explicit = opt_path("run_root").is_some();
    let dataset_explicit = opt_path("dataset").is_some();
    let memory_manifest_explicit = opt_path("memory_manifest").is_some();
    let run_root = opt_path("run_root").unwrap_or_else(|| {
        PathBuf::from("runs/.tmp/symbiotic-memory/long-mem-eval")
            .join(limit.to_string())
            .join("<new-run>")
    });

    SymemRunPlan {
        repo_root: repo_root.to_path_buf(),
        run_root,
        run_root_explicit,
        dataset: opt_path("dataset").unwrap_or_default(),
        dataset_explicit,
        limit,
        sample: str_field("sample", "stratified"),
        smoke: bool_field("smoke", false),
        distiller: str_field("distiller", "llm"),
        embedder: str_field("embedder", "gemini"),
        store: str_field("store", "sqlite"),
        prompt_dir: opt_path("prompt_dir"),
        distill_prompt: str_field("distill_prompt", "distill"),
        answerer: bool_field("answerer", true),
        routed: bool_field("routed", true),
        answer_only,
        consolidate_briefs: bool_field("consolidate_briefs", true),
        resume,
        // Native runs are fresh by default unless reuse is explicitly requested.
        fresh: !resume && !answer_only,
        query_planner: nested_str(params, &["query_planner"])
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .or_else(|| Some("scripted".to_string())),
        score: if bool_field("smoke", false) {
            false
        } else {
            nested_bool(params, &["score"]).unwrap_or(true)
        },
        oracle: opt_path("oracle"),
        judge_workers: nested_u64(params, &["judge_workers"]).unwrap_or(400) as usize,
        prewarm_judge_cache: nested_u64(params, &["prewarm_judge_cache"]).unwrap_or(0) as usize,
        prewarm_pause_secs: nested_u64(params, &["prewarm_pause_secs"]).unwrap_or(10),
        scorer: str_field("scorer", "queued-longmemeval-deepseek-v4-flash"),
        symem_bin: opt_path("symem_bin"),
        memory_manifest: opt_path("memory_manifest")
            .unwrap_or_else(|| PathBuf::from("../symbiotic-memory/Cargo.toml")),
        memory_manifest_explicit,
        memory_config: opt_path("memory_config"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(symem_bin: Option<&str>) -> SymemRunPlan {
        SymemRunPlan {
            repo_root: PathBuf::from("/repo"),
            run_root: PathBuf::from("runs/symbiotic-memory/long-mem-eval/3/sample"),
            run_root_explicit: false,
            dataset: PathBuf::from("data/longmemeval.json"),
            dataset_explicit: true,
            limit: 3,
            sample: "stratified".to_string(),
            distiller: "heuristic".to_string(),
            embedder: "hash".to_string(),
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
            score: false,
            oracle: None,
            judge_workers: 400,
            prewarm_judge_cache: 0,
            prewarm_pause_secs: 10,
            scorer: "queued-longmemeval-deepseek-v4-flash".to_string(),
            symem_bin: symem_bin.map(PathBuf::from),
            memory_manifest: PathBuf::from("../symbiotic-memory/Cargo.toml"),
            memory_manifest_explicit: false,
            memory_config: None,
            smoke: false,
        }
    }

    #[test]
    fn direct_binary_command_skips_cargo_and_uses_membench_args() {
        let plan = sample(Some("target/release/membench"));
        let command = plan.run_command();
        assert_eq!(command.program, "target/release/membench");
        assert_eq!(
            &command.args[..2],
            &["--symbiotic-memory", "--long-mem-eval"]
        );
        assert!(command.args.contains(&"--answerer".to_string()));
        assert!(!command.args.contains(&"--out".to_string()));
        assert!(!command.args.contains(&"--prompt-dir".to_string()));
    }

    #[test]
    fn cargo_fallback_without_binary() {
        let plan = sample(None);
        let command = plan.run_command();
        assert_eq!(command.program, "cargo");
        assert_eq!(
            &command.args[..8],
            &[
                "run",
                "--features",
                "symbiotic-memory-adapter",
                "--bin",
                "membench",
                "--",
                "--symbiotic-memory",
                "--long-mem-eval"
            ]
        );
    }

    #[test]
    fn explicit_prompt_dir_overrides_default() {
        let mut plan = sample(Some("target/release/membench"));
        plan.prompt_dir = Some(PathBuf::from("custom-prompts"));
        let args = plan.run_command().args;
        assert!(args.contains(&"custom-prompts".to_string()));
        assert!(!args.contains(&"../symbiotic-memory/prompts".to_string()));
    }

    #[test]
    fn score_command_is_embedded_in_native_run() {
        let mut plan = sample(Some("target/release/membench"));
        assert!(plan.score_command().is_none());
        plan.score = true;
        plan.oracle = Some(PathBuf::from("data/oracle.json"));
        assert!(plan.score_command().is_none());
        assert!(
            !plan
                .warnings()
                .iter()
                .any(|warning| warning.contains("not wired"))
        );
    }

    #[test]
    fn plan_from_params_applies_defaults_and_freshness() {
        let plan = plan_from_params(
            &json!({"dataset": "d.json", "limit": 50}),
            Path::new("/repo"),
        );
        assert_eq!(plan.limit, 50);
        assert!(plan.fresh);
        assert_eq!(plan.distiller, "llm");
        assert_eq!(plan.embedder, "gemini");
        assert!(plan.answerer);
        assert!(plan.routed);
        assert!(plan.consolidate_briefs);
        assert_eq!(plan.query_planner.as_deref(), Some("scripted"));
        assert!(plan.score);
        assert!(!plan.smoke);
        assert_eq!(plan.judge_workers, 400);
        assert!(!plan.run_root_explicit);
        assert!(plan.dataset_explicit);

        let reuse = plan_from_params(&json!({"answer_only": true}), Path::new("/repo"));
        assert!(!reuse.fresh);
    }

    #[test]
    fn smoke_mode_uses_one_clear_flag() {
        let plan = plan_from_params(&json!({"smoke": true}), Path::new("/repo"));
        let command = plan.run_command();
        assert!(command.args.contains(&"--smoke".to_string()));
        assert!(!command.args.contains(&"--distiller".to_string()));
        assert!(!command.args.contains(&"--embedder".to_string()));
        assert!(!command.args.contains(&"--score".to_string()));
        assert!(!command.args.contains(&"--no-score".to_string()));
    }

    #[test]
    fn membench_command_omits_default_paths_and_values() {
        let plan = plan_from_params(
            &json!({
                "dataset": "runs/inputs/longmemeval-cleaned/longmemeval_s_cleaned.json",
                "limit": 10,
                "memory_config": "config/symbiotic-memory/longmemeval-raw-light.yaml",
                "answerer": true,
                "routed": true,
                "consolidate_briefs": true,
                "query_planner": "scripted"
            }),
            Path::new("/repo"),
        );
        let command = plan.run_command();
        assert_eq!(command.program, "cargo");
        assert!(!command.args.contains(&"--dataset".to_string()));
        assert!(!command.args.contains(&"--run-root".to_string()));
        assert!(!command.args.contains(&"--limit".to_string()));
        assert!(command.args.contains(&"--memory-config".to_string()));
        assert!(command.args.contains(&"--answerer".to_string()));
        assert!(command.args.contains(&"--routed".to_string()));
        assert!(command.args.contains(&"--consolidate-briefs".to_string()));
        assert!(command.args.contains(&"--score".to_string()));
        assert_eq!(
            command.to_shell(),
            "cargo run --features symbiotic-memory-adapter --bin membench -- --symbiotic-memory --long-mem-eval --memory-config config/symbiotic-memory/longmemeval-raw-light.yaml --answerer --routed --consolidate-briefs --query-planner scripted --score"
        );
    }

    #[test]
    fn shell_rendering_quotes_when_needed() {
        let command = PlannedCommand {
            program: "symem".to_string(),
            args: vec!["--path".to_string(), "a b.json".to_string()],
        };
        assert_eq!(command.to_shell(), "symem --path 'a b.json'");
    }
}
