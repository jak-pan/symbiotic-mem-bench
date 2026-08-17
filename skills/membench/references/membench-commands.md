# Membench Commands

## Repository Root

Prefer running from the `symbiotic-mem-bench` repo root:

```bash
cd ../symbiotic-mem-bench
```

From another repo, use:

```bash
cargo run --manifest-path ../symbiotic-mem-bench/adapters/symbiotic-memory/Cargo.toml --bin membench -- ...
```

## Explore Runs

List local scratch runs:

```bash
cargo run --manifest-path adapters/symbiotic-memory/Cargo.toml --bin membench -- explore
```

Inspect one run:

```bash
cargo run --manifest-path adapters/symbiotic-memory/Cargo.toml --bin membench -- explore \
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
CARGO_TARGET_DIR=/tmp/symbiotic-mem-bench-target cargo run --release \
  --manifest-path adapters/symbiotic-memory/Cargo.toml \
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

Answer-only native reruns default to `workflow_max_in_flight=500` so recall/answer-only comparisons
can make full use of the provider queue caps. Fresh ingest runs use the memory config's workflow
window.

Paid provider-backed native runs are serialized per benchmark repo clone with an atomic lock at:

```text
runs/.locks/paid-provider-run.lock/
```

This is deliberate: large benches should run one-by-one while each run keeps its own
`provider-queue/` traces and response cache. A second paid run fails before starting provider calls.
If a killed process leaves the lock behind, inspect `owner.json` and remove the directory only after
confirming the recorded process is no longer alive.

The reference clock sent to Symbiotic Memory's query planner and answerer defaults to the current
RFC3339 datetime with timezone for normal memory use. LongMemEval maps each row's `question_date`
into an RFC3339 timestamp so temporal questions replay against the benchmark's pinned clock. Override
it only for a named experiment:

```bash
MEMBENCH_REFERENCE_DATETIME=2026-06-19T15:15:42+08:00 \
CARGO_TARGET_DIR=/tmp/symbiotic-mem-bench-target cargo run --release \
  --manifest-path adapters/symbiotic-memory/Cargo.toml \
  --bin membench -- \
  --system symbiotic-memory \
  --benchmark long-mem-eval \
  --limit 10
```

LongMemEval still records the original row `question_date` in debug provenance; the answerer and
query planner receive the parsed RFC3339 timestamp as the reference clock.

The current raw-light LongMemEval profile is
`config/symbiotic-memory/longmemeval-raw-light.yaml`: facts 20, raw top-k 10,
DeepSeek Flash query planner, and shared provider queue defaults.

Embedding request sizing is split deliberately:

- `SYMBIOTIC_MEMORY__EMBED__BATCH_SIZE` and `SYMBIOTIC_MEMORY__EMBED__BATCH_MAX_CHARS`
  (the kit's `embed.batch_size` / `embed.batch_max_chars`) pack embedding HTTP requests.
  Defaults are code-owned in `symbiotic-memory`.
- `MEMBENCH_EMBED_MAX_CHARS` is the per-input local text cap.

The batch char budget must not truncate individual inputs, and none of these settings are provider
concurrency caps. Provider concurrency is controlled by the shared model queue id.

Default native launches are paid, provider-backed, scored, and must run in Cargo release mode:

```text
distiller        llm
embedder         gemini
answerer         enabled
consolidation    enabled
query_planner    flash
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
  --manifest-path adapters/symbiotic-memory/Cargo.toml \
  --bin membench -- \
  --system symbiotic-memory \
  --benchmark long-mem-eval \
  --smoke
```

## Queued Scoring

Scoring defaults to the `official` LongMemEval judge prompt mode: the per-question-type paper
grader (see `JUDGE.md`). The selected mode is written to `scored.json` as `judge_prompt_mode`.
The older generic semantic grader remains available for A/B comparisons via
`MEMBENCH_JUDGE_PROMPT_MODE=semantic` (aliases: `semantic-shared-compact`, `legacy`, `generic`).

DeepSeek judge cache prewarm should remain opt-in and mainly for rejudge/score-heavy runs:

```bash
CARGO_TARGET_DIR=/tmp/symbiotic-mem-bench-target cargo run --release \
  --manifest-path adapters/symbiotic-memory/Cargo.toml --bin membench -- \
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
cargo run --manifest-path adapters/symbiotic-memory/Cargo.toml --bin membench -- \
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
cargo run --manifest-path adapters/symbiotic-memory/Cargo.toml --bin membench -- save-record \
  --run-root runs/{system}/{benchmark}/{limit}/{run_name}
```

Use `--record-name` only to give the tracked record a clearer public name. Use `--force` only when
intentionally replacing an existing record.

## Trials

Derive typed improvement-trial artifacts from existing run outputs:

```bash
cargo run --manifest-path adapters/symbiotic-memory/Cargo.toml --bin membench -- trials derive \
  --trial-run-root runs/{system}/{benchmark}/{limit}/{candidate_run} \
  --comparison-run-root runs/{system}/{benchmark}/{limit}/{previous_run} \
  --original-baseline-run-root runs/{system}/{benchmark}/{limit}/{baseline_run} \
  --change-title "{short title}" \
  --reasoning "{why this generic change is being tested}" \
  --changed-file "../symbiotic-memory/src/recall/prompt_policy.rs:120|answer prompt|Clarify evidence grouping" \
  --verification "cargo test --manifest-path ../symbiotic-memory/Cargo.toml prompt_ --features cli" \
  --risk "Focused trial stack; validate on a stratified 25-50Q stack before broad conclusions." \
  --decision "diagnostic_only"
```

`--stack-id` and `--change-id` are optional. Omit them for normal use: `membench` generates a
stable change id from the title plus compared run roots, and writes to
`runs/analysis/trial-{generated-change-id}/`. Provide explicit ids only when intentionally grouping
several trial rows into the same ledger.

Default output:

```text
runs/analysis/{stack_id}/trial-stack.json
runs/analysis/{stack_id}/trials.jsonl
runs/analysis/{stack_id}/trial-question-deltas.jsonl
```

Use `--output-dir` only for a deliberate alternate analysis folder. Use `--force` to replace existing
rows for the same trial run id after correcting metadata. The runner derives question deltas from
standard artifacts and stores debug bundle paths/hashes rather than copying raw prompt text.

Focused sub-25Q stacks are valid for one failure class or prompt-forensics loop. Use a stratified
25-50Q stack for broader diagnostic trial decisions, and a complete benchmark run for publishable
claims.

## Queue Timing

Summarize queue events:

```bash
cargo run --manifest-path adapters/symbiotic-memory/Cargo.toml --bin membench -- summarize-queue-events \
  --jsonl runs/{system}/{benchmark}/{limit}/{run_name}/provider-queue/model-queue-traces.jsonl
```

The queue summary groups by `queue_id` and `item_id`, deriving wait time, run time, total time,
attempt count, and final status.

## OpenRouter Qwen Embedding Tuning

Run a raw-embedding-only transport tuning arm with the existing release binary:

```bash
scripts/run-embedding-transport-tuning.sh openrouter-qwen3-8b-1024 h1-32x32
```

Compare the saved evidence runs:

```bash
scripts/report-embedding-transport-tuning.sh --profile openrouter-qwen3-8b-1024 --markdown
```

## DeepSeek Chat Transport Tuning

Run a distill-only DeepSeek transport tuning arm with the existing release binary:

```bash
scripts/run-chat-transport-tuning.sh deepseek-v4-flash-distill h2-64x32
```

Compare saved DeepSeek chat transport evidence runs:

```bash
scripts/report-chat-transport-tuning.sh --profile deepseek-v4-flash-distill --markdown
```

The chat transport shape is controlled by `SYMBIOTIC_MEMORY__TRANSPORT__CHAT_CLIENT_POOL_SIZE`,
`SYMBIOTIC_MEMORY__TRANSPORT__POOL_MAX_IDLE_PER_HOST`, and
`SYMBIOTIC_MEMORY__TRANSPORT__HTTP1_ONLY` (the kit's `[transport]` config section).
Those are below the provider queue: they do not replace the model catalog or
workflow `max_in_flight`.

Promote one local evidence run as a dashboard-safe meta record:

```bash
scripts/save-run-meta-record.sh runs/symbiotic-memory/long-mem-eval/10/<run-name>
```

The current decision records are:

- `docs/symbiotic-memory/openrouter-qwen-embedding-tuning.md`: OpenRouter Qwen raw embedding.
- `docs/symbiotic-memory/deepseek-chat-transport-tuning.md`: DeepSeek Flash distill chat.

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
