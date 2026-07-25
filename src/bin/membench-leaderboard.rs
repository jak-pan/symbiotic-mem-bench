//! `membench-leaderboard` — export the static leaderboard document.
//!
//! Scans the tracked records tree and emits a `membench.leaderboard.v1` JSON
//! document (see `docs/schemas.md`). This is the publishable counterpart of the
//! dashboard's live `/api/leaderboard`: same cohorts, plus provenance and
//! per-row verification levels.
//!
//! Paths: a relative `--records-root` resolves against the repository root
//! (the crate manifest directory), matching the `membench` CLI convention, so
//! the command can be run from anywhere. Run ids inside the document stay
//! repo-relative.

use clap::{Parser, Subcommand};
use membench::leaderboard_export::{self, ExportOptions};
use membench::registry;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "membench-leaderboard",
    about = "Export the membench.leaderboard.v1 leaderboard document"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Export the leaderboard document (default when no command is given).
    Export(ExportArgs),
}

#[derive(Clone, Debug, clap::Args)]
struct ExportArgs {
    /// Records root to scan. Relative paths resolve from the repo root.
    #[arg(long, default_value = "records")]
    records_root: PathBuf,
    /// Write the document here instead of stdout.
    #[arg(long)]
    out: Option<PathBuf>,
    /// Replace volatile fields (generated_at, git_sha, per-row modified_ms)
    /// with fixed values so repeated exports diff cleanly.
    #[arg(long)]
    deterministic: bool,
}

impl Default for ExportArgs {
    fn default() -> Self {
        Self {
            records_root: PathBuf::from("records"),
            out: None,
            deterministic: false,
        }
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let args = match cli.command {
        Some(Commands::Export(args)) => args,
        None => ExportArgs::default(),
    };

    let records_root = if args.records_root.is_absolute() {
        args.records_root.clone()
    } else {
        repo_root().join(&args.records_root)
    };

    let records = registry::scan_registry(std::slice::from_ref(&records_root), &repo_root());
    let summaries: Vec<_> = records.iter().map(registry::summarize).collect();

    let options = ExportOptions {
        records_root: args.records_root.to_string_lossy().replace('\\', "/"),
        // Recomputable provenance: whoever reads the document can hash the same
        // tree and see whether it still describes these records.
        records_digest: leaderboard_export::records_digest(&records_root),
        git_sha: option_env!("GIT_SHA").unwrap_or("unknown").to_string(),
        methodology: leaderboard_export::DEFAULT_METHODOLOGY.to_string(),
        deterministic: args.deterministic,
    };
    let generated_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let document = leaderboard_export::build_document(&summaries, &generated_at, &options);
    let rendered = serde_json::to_string_pretty(&document)?;

    match &args.out {
        Some(path) => {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(path, format!("{rendered}\n"))?;
            eprintln!("wrote {}", path.display());
        }
        None => println!("{rendered}"),
    }
    Ok(())
}
