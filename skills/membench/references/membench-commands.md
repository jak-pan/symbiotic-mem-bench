# Membench Commands

Shared collaboration, Git, verification, and security rules: House Rules (`~/p/house-rules/AGENTS.md`).

## Repository Root

Run the examples from this repository root. Native adapter commands require read access to
the pinned Memory Git dependency; the public root package is credential-free. Replace
`<...>`, `{...}`, and `path/to/...` placeholders with real local inputs. Example run roots
are illustrative and are not included in a clean checkout.

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

Answer-only native reruns default to `workflow_max_in_flight=64`. Fresh ingest runs use the memory
config's workflow window. Override either with `MEMBENCH_WORKFLOW_MAX_IN_FLIGHT`; provider
concurrency remains controlled by model queue id.

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
Flash query planner policy, and queue timeout/retry settings. Model bindings are resolved
from the pinned Memory/foundation configuration, not this profile.

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
embedder         openrouter
answerer         enabled
consolidation    opt-in
query_planner    flash
store            zvec
score            enabled
memory_config    config/symbiotic-memory/longmemeval-raw-light.yaml
```

Native `--smoke`, paid ingestion/scoring, and `artifacts/model-traces.jsonl` export are wired.
Gemini embeddings require explicit `--embedder gemini`. Brief generation needs both
`--consolidate-briefs` and `MEMBENCH_CONSOLIDATOR=llm`. The persistent store is `zvec`;
`sqlite` and `zvec-hybrid` are rejected (old vaults need fresh ingestion). Rerank is on by
harness default; pin `MEMBENCH_RERANK_MODEL` for comparable model-specific trials.

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

## Oracle Reader Comparison

For an answerer/prompt experiment with gold-session evidence, use the native CLI. Replace
`<source-vault-root>`, `<prompt-dir>`, and `<run-name>` with local inputs; the source vaults
must use the current zvec format. A clean checkout contains neither vaults nor custom prompts.
This is a paid diagnostic arm, not a promotable benchmark claim. Gold evidence replaces the
answer context; recall still runs for diagnostics.

```bash
MEMBENCH_ANSWER_OPERATOR=openrouter MEMBENCH_ANSWER_MODEL=<operator/model> \
MEMBENCH_ANSWER_THINKING=on \
CARGO_TARGET_DIR=/tmp/symbiotic-mem-bench-target cargo run --release \
  --manifest-path adapters/symbiotic-memory/Cargo.toml --bin membench -- \
  --system symbiotic-memory --benchmark long-mem-eval --limit 500 \
  --answer-only --oracle-gold --source-vault-root <source-vault-root> \
  --prompt-dir <prompt-dir> --run-name <run-name>
```

Read the stored result with `scripts/score-run.sh <run-name>`; that helper makes no provider calls.

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

Prewarm scores the selected prefix under `raw/judge-cache-prewarm/` through the queued
scorer, waits, then performs the real score. Inspect that subdirectory when accounting for
warmup calls. It is disabled by default so ingest/answer pipelines remain async.

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
  --changed-file "src/runner.rs|runner|Describe the benchmark policy change" \
  --verification "cargo fmt -- --check" \
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

## Historical Transport Experiments

Decisions from the original transport studies live in
[OpenRouter Qwen embedding tuning](../../../docs/symbiotic-memory/openrouter-qwen-embedding-tuning.md)
and [DeepSeek chat transport tuning](../../../docs/symbiotic-memory/deepseek-chat-transport-tuning.md).
Their launch helpers still select retired storage backends; they are not current launch examples.

## Required Run Shape

See [docs/run-registry.md](../../../docs/run-registry.md) for the native/imported layout and
[AGENTS.md](../../../AGENTS.md#async-adapter-contract) for incremental-output invariants.

## Validation

Use the local checks in [AGENTS.md](../../../AGENTS.md#validate), with the external target
cache and targeted tests in CI's debug profile. CI owns the full core/server test suites.
