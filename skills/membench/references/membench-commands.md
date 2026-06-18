# Membench Commands

## Repository Root

Prefer running from the `symbiotic-mem-bench` repo root:

```bash
cd ../symbiotic-mem-bench
```

From another repo, use:

```bash
cargo run --manifest-path ../symbiotic-mem-bench/Cargo.toml --bin membench -- ...
```

## Explore Runs

List local scratch runs:

```bash
cargo run --bin membench -- explore
```

Inspect one run:

```bash
cargo run --bin membench -- explore \
  --run-root runs/symbiotic-memory/long-mem-eval/500/baseline-clean
```

The list view shows `kind=native` or `kind=imported-artifact`, score, native-state availability, and
missing artifact count.

## Native Symbiotic Memory LongMemEval

Paid provider-backed runs load `.env.test.local` from this benchmark repository. Create it from the
tracked template before running real provider calls:

```bash
cp .env.example .env.test.local
```

The runner does not implicitly load env files from sibling repositories.

Run a fresh default benchmark:

```bash
CARGO_TARGET_DIR=/tmp/symbiotic-mem-bench-target cargo run \
  --features symbiotic-memory-adapter \
  --bin membench -- \
  --system symbiotic-memory \
  --benchmark long-mem-eval \
  --limit 10
```

When `--dataset` is omitted, the runner uses
`runs/inputs/longmemeval-cleaned/longmemeval_s_cleaned.json`. If that file is missing, it downloads
the cleaned S dataset from `xiaowu0162/longmemeval-cleaned` first. Pass `--dataset` only for a custom
local dataset file.

Small runs default to `--sample stratified`, which round-robins question types in first-seen order.
Pass `--sample first` only when reproducing an old first-N row-order run.

Normal native runs are fresh by default. `membench` passes `--fresh`, resets the selected run root,
and re-ingests source questions. Use reuse flags only when deliberately testing state reuse:

```text
--resume       continue an interrupted run root
--answer-only  reuse an already ingested run root and write new answers
```

The current raw-light LongMemEval profile is
`config/symbiotic-memory/longmemeval-raw-light.yaml`: facts 20, raw primary 10, raw fallback 10,
scripted query planner, and shared provider queue defaults.

Default native launches are paid, provider-backed, and scored:

```text
distiller        llm
embedder         gemini
answerer         enabled
routed           enabled
consolidation    enabled
query_planner    scripted
score            enabled
memory_config    config/symbiotic-memory/longmemeval-raw-light.yaml
```

Current adapter status:

```text
wired:   explicit --smoke local no-network run-shape tests
wired:   default provider-backed LLM/Gemini ingestion and queued LongMemEval scoring
pending: compact normalized artifacts/model-traces.jsonl export
```

Do not restore benchmark subcommands in `symem`; paid benchmark orchestration and scoring belong in
`membench`.

Run a local no-network smoke explicitly:

```bash
CARGO_TARGET_DIR=/tmp/symbiotic-mem-bench-target cargo run \
  --features symbiotic-memory-adapter \
  --bin membench -- \
  --system symbiotic-memory \
  --benchmark long-mem-eval \
  --smoke
```

## Queued Scoring

Scoring will default to the `semantic-shared-compact` LongMemEval judge prompt. It uses one reusable
prefix for better prompt-cache behavior and accepts equivalent or inferable answers that the original
verbose prompt can mark as false negatives. The selected mode is written to `scored.json` as
`judge_prompt_mode`. For audit comparisons, run with `SYMEM_JUDGE_PROMPT_MODE=official`; for
official-style cache experiments, use `SYMEM_JUDGE_PROMPT_MODE=category-prefix`.

DeepSeek judge cache prewarm should remain opt-in and mainly for rejudge/score-heavy runs:

```bash
cargo run --bin membench -- \
  --system symbiotic-memory \
  --benchmark long-mem-eval \
  --dataset path/to/longmemeval.json \
  --limit 500 \
  --score \
  --oracle path/to/longmemeval.json \
  --prewarm-judge-cache 5 \
  --prewarm-pause-secs 10
```

This example is not an active command until scoring is ported. The prewarm should write temporary
files under `raw/judge-cache-prewarm/`, score only those rows through the same queued scorer, wait,
then run the real score. Its local queue and traces stay under that same raw subdirectory, so final
score cost/latency artifacts exclude warmup. It is disabled by default so full ingest/answer
pipelines stay fully async.

## Import Existing Artifacts

Use imports for frozen or external artifacts:

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

The importer derives `{limit}` from `scored.json`, copies files into `artifacts/`, avoids original
absolute source paths, and writes artifact availability into `artifact_manifest`.

## Promote Records

Promote a scratch run to tracked records:

```bash
cargo run --bin membench -- save-record \
  --run-root runs/{system}/{benchmark}/{limit}/{run_name}
```

Use `--record-name` only to give the tracked record a clearer public name. Use `--force` only when
intentionally replacing an existing record.

## Queue Timing

Summarize queue events:

```bash
cargo run --bin membench -- summarize-queue-events \
  --jsonl runs/{system}/{benchmark}/{limit}/{run_name}/provider-queue/model-queue-traces.jsonl
```

The queue summary groups by `queue_id` and `item_id`, deriving wait time, run time, total time,
attempt count, and final status.

## Required Run Shape

All complete runs:

```text
run-params.json
benchmark-report.json
artifacts/
```

Native runs can also have:

```text
raw/
vaults/
workflow/
provider-queue/
```

Imported runs may be artifact-only. Do not create fake native state to make old runs look complete.
Native adapters write executor-owned outputs directly under `raw/`; do not add end-of-run cleanup
steps that move root-level temporary files into place.

## Validation

Use an external target dir:

```bash
CARGO_TARGET_DIR=/tmp/symbiotic-mem-bench-target cargo fmt -- --check
CARGO_TARGET_DIR=/tmp/symbiotic-mem-bench-target cargo test
CARGO_TARGET_DIR=/tmp/symbiotic-mem-bench-target cargo clippy --all-targets -- -D warnings
```
