# Agent Instructions

This repository is the neutral benchmark harness for memory systems. Keep it easy for the next
agent to run, inspect, and publish results without reverse-engineering local scratch state.

## Core Rules

- Work from this repository root when possible.
- Prefer `cargo run --bin membench -- ...` over ad hoc scripts.
- Do not add Python scoring scripts or manual score entry paths.
- Do not commit `runs/`, `.debug-session/`, `target/`, secrets, provider queues, or raw local
  datasets.
- Use `CARGO_TARGET_DIR=/tmp/symbiotic-mem-bench-target` for validation when preparing the repo for
  publication, so `target/` is not recreated in the repo.
- Keep benchmark records portable: repo-relative paths in reports, no local absolute source paths.
- Do not store raw prompts or secrets in tracked records. Raw content is allowed only in explicit
  local debug artifacts under ignored scratch paths.

## Validate

```bash
CARGO_TARGET_DIR=/tmp/symbiotic-mem-bench-target cargo fmt -- --check
CARGO_TARGET_DIR=/tmp/symbiotic-mem-bench-target cargo test
```

If you need to run from another repository, use:

```bash
CARGO_TARGET_DIR=/tmp/symbiotic-mem-bench-target \
  cargo test --manifest-path ../symbiotic-mem-bench/Cargo.toml
```

## Codex Skill

The repo ships a Codex skill at `skills/membench/SKILL.md`. Use it as the compact workflow entry
point for benchmark runs, imports, explorer usage, publication checks, and run-shape rules. Its
longer command reference lives at `skills/membench/references/membench-commands.md`.

## Run Registry Shape

Scratch runs:

```text
runs/{system}/{benchmark}/{limit}/{run_name}/
```

Tracked curated records:

```text
records/{system}/{benchmark}/{limit}/{run_name}/
```

Every complete run should contain:

```text
run-params.json
benchmark-report.json
artifacts/
```

Use the same shape for all systems, not just Symbiotic Memory.

For native runs, `artifacts/` is the portable public bundle and `raw/` keeps executor-native outputs.
Do not delete `vaults/`, `workflow/`, or `provider-queue/` from local scratch runs unless you are
intentionally making a minimal exported record.

Imported runs can be artifact-only. Their `artifact_manifest.native_state_available` must be `false`,
and their `artifact_manifest.missing` list must make absent traces or state folders explicit.

## Native Symbiotic Memory Run

Paid provider-backed runs read `.env.test.local` from this benchmark repository by default. Start
from the tracked template:

```bash
cp .env.example .env.test.local
```

Do not commit real `.env` files. Use `--env-file path/to/file` only when intentionally testing a
different environment; the runner does not implicitly load sibling repo env files. See
`docs/environment.md` for the env file and queue/cache boundary.

Run native Symbiotic Memory benchmarks through the adapter:

```bash
CARGO_TARGET_DIR=/tmp/symbiotic-mem-bench-target cargo run \
  --features symbiotic-memory-adapter \
  --bin membench -- \
  --system symbiotic-memory \
  --benchmark long-mem-eval \
  --limit 50
```

Current adapter status:

```text
wired:   explicit `--smoke` local no-network run-shape tests
wired:   default paid provider-backed LLM/Gemini ingestion and queued LongMemEval scoring
pending: compact normalized `artifacts/model-traces.jsonl` export
```

Do not restore `symem` benchmark subcommands. Benchmark orchestration, scoring, records, and
dashboard artifacts belong in this repository.

Default launches are paid, provider-backed, and scored. A local no-network smoke test must be
requested explicitly:

```bash
CARGO_TARGET_DIR=/tmp/symbiotic-mem-bench-target cargo run \
  --features symbiotic-memory-adapter \
  --bin membench -- \
  --system symbiotic-memory \
  --benchmark long-mem-eval \
  --smoke
```

Explicit local smoke tests map internally to local deterministic providers and no scorer.
They are temporary by default: without an explicit `--run-root` or hidden `--keep-smoke-run`, they
run outside the dashboard registry and delete themselves after success.

`membench` chooses a run root automatically:

```text
runs/symbiotic-memory/long-mem-eval/{limit}/{timestamp-shortid}/
```

Normal native benchmark runs are fresh by default. `membench` passes `--fresh`, resets the run root,
and re-ingests every selected question from source. Only reuse state when explicitly testing resume
behavior:

```text
default      fresh re-ingest
--resume     continue an interrupted run root
--answer-only reuse an already ingested run root
```

Only pass `--run-root` for a deliberate, named local run. If you do pass `--run-root` without
`--resume` or `--answer-only`, expect it to be reset.

## Async Adapter Contract

Adapters must preserve the system under test's native pipeline:

- no duplicate benchmark-owned ingestion, embedding, or recall logic;
- no artificial phase barrier between capture, Distillery, raw embedding, fact embedding, indexing,
  answer, and score;
- write stage outputs and traces incrementally;
- acknowledge durable work only after outputs are written;
- cap provider concurrency by model queue id, not by benchmark stage name.

## Import Existing Artifacts

Use imports when artifacts already exist and should be normalized without regenerating answers:

```bash
cargo run --bin membench -- \
  --system symbiotic-memory \
  --benchmark long-mem-eval \
  --import-report \
  --run-name baseline-clean \
  --hypotheses path/to/hypotheses.jsonl \
  --verdicts path/to/verdicts.jsonl \
  --partial-verdicts path/to/partial-verdicts.jsonl \
  --memory-traces path/to/memory-traces.jsonl \
  --model-traces path/to/model-traces.jsonl \
  --scored path/to/scored.json
```

The importer derives `{limit}` from `scored.json`, copies artifacts into the run folder, and records
only portable artifact presence and missing-artifact metadata in `run-params.json`.

## Explore And Promote

List runs:

```bash
cargo run --bin membench -- explore
```

Inspect one run:

```bash
cargo run --bin membench -- explore \
  --run-root runs/symbiotic-memory/long-mem-eval/500/baseline-clean
```

Promote a local run to tracked records:

```bash
cargo run --bin membench -- save-record \
  --run-root runs/symbiotic-memory/long-mem-eval/500/baseline-clean
```

Records must keep the same `{system}/{benchmark}/{limit}/{run_name}` shape.

## Naming Runs

Use neutral names that describe the benchmark condition, not private debugging history.

Good:

```text
baseline-clean
candidate-answer-thinking
candidate-count-ledger
diagnostic-clean-baseline
```

Avoid:

```text
task numbers
private branch names
provider incident names
raw local script names
```

## Publication Hygiene

Before publishing or opening a PR:

```bash
CARGO_TARGET_DIR=/tmp/symbiotic-mem-bench-target cargo fmt -- --check
CARGO_TARGET_DIR=/tmp/symbiotic-mem-bench-target cargo test
cargo run --bin membench -- explore
```

Then check:

```bash
git status --short --ignored
rg -n "source_path|task-[0-9]+|task152|v12|legacy python" . \
  -g '!target' -g '!runs' -g '!AGENTS.md'
```

Expected ignored local paths are `runs/` and any external target directory. `.debug-session/` and
`target/` should not be present in the repo when preparing for publication.
