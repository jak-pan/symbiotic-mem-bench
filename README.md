# Symbiotic Memory Bench

Benchmark contracts and tooling for memory systems.

This repository owns benchmark orchestration, run metadata, trace schemas, score summaries, and
portable run records. Memory implementation behavior stays inside the system under test, such as
`symbiotic-memory`, `mem0`, or `HyMem`.

## Quick Start (no secrets required)

Everything in this section is local and network-free — no API keys. From this repository root:

```bash
CARGO_TARGET_DIR=/tmp/symbiotic-mem-bench-target cargo test
cargo run --bin membench-leaderboard -- export --records-root records
```

The `membench` CLI itself (`cargo run --bin membench -- explore`) needs the
`symbiotic-memory-adapter` feature, which builds against the pinned
`jak-pan/symbiotic-memory` revision — currently a **private** repository, so a clean clone
without access cannot build it (see `docs/oss-release-handoff.md`). With access:

```bash
cargo run --features symbiotic-memory-adapter --bin membench -- explore
```

`scripts/check-adapter-build.sh` is the gate for that path; `scripts/check-adapter-pins.sh`
checks (offline, no credentials) that every git dependency is pinned to an exact rev that
`Cargo.lock` resolves.

Export the static leaderboard document over the reproducible sample records (the CI canary
fixtures — synthetic, clearly labeled, never real results):

```bash
cargo run --bin membench-leaderboard -- export --records-root canary/records --deterministic
```

Build and open the dashboard over the tracked records (needs Node 22, still no keys):

```bash
cd dashboard && npm ci && npm run build && cd ..
cargo run --features server --bin membench-server   # http://localhost:8787
```

For paid provider-backed runs, create the local env file in this repository:

```bash
cp .env.example .env.test.local
```

Then edit `.env.test.local` with real keys. It is gitignored and is the only env file loaded by
default. Use `--env-file path/to/file` only for an intentional alternate environment. See
`docs/environment.md` for the full env contract.

Run a default Symbiotic Memory LongMemEval benchmark through the native adapter:

```bash
CARGO_TARGET_DIR=/tmp/symbiotic-mem-bench-target cargo run --release \
  --features symbiotic-memory-adapter \
  --bin membench -- \
  --system symbiotic-memory \
  --benchmark long-mem-eval \
  --limit 10
```

When `--dataset` is omitted for LongMemEval, `membench` downloads the cleaned S dataset from
`xiaowu0162/longmemeval-cleaned` into ignored local storage at
`runs/inputs/longmemeval-cleaned/longmemeval_s_cleaned.json` and reuses it on later runs.
Small LongMemEval runs use `--sample stratified` by default so quick runs cover multiple question
types. Use `--sample first` only when reproducing the dataset's original row order.

Default launches are paid, provider-backed, scored, and must run in Cargo release mode. The
owner-default stack is: `llm` distill (DeepSeek Flash), OpenRouter `qwen/qwen3-embedding-8b`
embeddings at 1024 dims (`--embedder openrouter` is the default; Gemini embeddings are NOT the
default), cross-encoder reranking ON by default with the free
`nvidia/llama-nemotron-rerank-vl-1b-v2:free` model (`MEMBENCH_RERANK=0` disables it), the
`zvec-hybrid` store, unified answering, DeepSeek Flash query planning, and judge scoring. Reweave
brief generation is opt-in (`MEMBENCH_CONSOLIDATOR=llm`), not a default. The native adapter owns
the run state, provider queues, response caches, hypotheses, verdicts, score summaries, and
normalized run report.

Run a no-network smoke test explicitly:

```bash
CARGO_TARGET_DIR=/tmp/symbiotic-mem-bench-target cargo run \
  --features symbiotic-memory-adapter \
  --bin membench -- \
  --system symbiotic-memory \
  --benchmark long-mem-eval \
  --smoke
```

The Symbiotic Memory adapter dependencies are pinned public git revisions in `Cargo.toml`, so the
core crate and the dashboard server build from a clean clone. Building the
`symbiotic-memory-adapter` feature currently requires overriding those pins to sibling checkouts
via `.cargo/config.toml` — see `docs/environment.md` ("Dependency Sources") for the exact block
and the upstream-publication blocker behind it.

These local `--smoke` adapter runs map internally to deterministic no-network providers and no
scorer. They are smoke tests, not benchmark records. By default they run under `runs/.tmp/` and
delete themselves after a successful validation, so they do not appear in the dashboard. Preserve one
only for forensics with an explicit `--run-root` or the hidden `--keep-smoke-run` escape hatch.

For Symbiotic Memory runs, `run-params.json` separates requested provider settings from providers
actually invoked by the adapter:

- `configured_models`: model/provider settings from the requested memory config.
- `runtime_models`: bindings used by this run. Paid runs record queued DeepSeek/Gemini bindings;
  native smoke runs record local deterministic providers such as `local:heuristic-v1`,
  `local:hash-embedding-v1`, and either `local:extractive-answer` or
  `local:disabled-chat-provider-with-extractive-fallback`.

Do not treat `configured_models` as proof that a provider was called. Provider calls are proven by
`artifacts/model-traces.jsonl` and `provider-queue/model-queue-traces.jsonl`.

## Dashboard

A Bloomberg-terminal-style web dashboard lives in `dashboard/`: a leaderboard for ranking memory
systems within a comparable cohort, plus a debugger/tuner for inspecting runs, questions, judge
verdicts, traces, and previewing new runs. It is a no-SSR Svelte SPA served by the Rust
`membench-server` binary over the same registry files the CLI reads.

```bash
# build the SPA once, then serve it + the API from one binary
cd dashboard && npm install && npm run build && cd ..
cargo run --features server --bin membench-server   # http://localhost:8787
```

Dashboard commands expect `node` and `npm` to resolve to the user `nvm` install from PATH, not
Homebrew Node. For live frontend development run `membench-server` and `npm run dev` (Vite on
:5173, proxies `/api`). See `dashboard/README.md`. The same normalized index is available headless
via `cargo run --bin membench -- explore --json`.

The dashboard command preview intentionally emits the short `membench` command from the benchmark
repo root. It omits default dataset, run-root, output, prompt, queue, and fresh-mode paths; those are
owned by the harness.

## Run Shape

Scratch runs live in the ignored local registry:

```text
runs/{system}/{benchmark}/{limit}/{run_name}/
```

Curated records use the same shape:

```text
records/{system}/{benchmark}/{limit}/{run_name}/
```

Every complete run folder contains:

- `run-params.json`: normalized run parameters.
- `benchmark-report.json`: normalized metrics, metadata, and artifact summaries.
- `artifacts/`: copied run artifacts such as hypotheses, verdicts, scored output, traces, and debug
  bundles.
- `artifact_manifest`: a field inside the JSON files that lists present and missing artifact
  classes.

Native runs can also contain:

- `raw/`: executor-native outputs written in-place while native runs execute.
- state folders such as `vaults/`, `workflow/`, and `provider-queue/`.

`workflow/` is the durable adapter workflow queue and can show row-level progress even for local
smoke runs. `provider-queue/` is only for queued model/provider calls; it is expected to be absent
or empty when the run used `--smoke`.

Local smoke-test state normally never reaches this layout because successful smoke runs are erased.
Only preserved smoke runs or real benchmark/provider runs should appear in the registry.

For LongMemEval native runs, `answer_output=true` means hypotheses are expected even when scoring is
off. `generative_answerer_enabled` says whether the memory engine used its generative answerer
policy. `score_output=false` means no judge verdicts or scored metrics should be present.

Imported comparison runs may be artifact-only. In that case `artifact_manifest.native_state_available`
is `false`, and `artifact_manifest.missing` lists unavailable traces or state exports.

New native runs default to a timestamp plus short id run name, such as:

```text
runs/symbiotic-memory/long-mem-eval/500/20260617-153012-a1b2c3d4/
```

Native runs re-ingest by default. `membench` passes `--fresh` for normal native benchmark runs so
the run root is reset and every question is ingested from source again. Reuse is opt-in with
`--resume` or `--answer-only`. For cheap answer-only comparisons, use `--source-vault-root
runs/inputs/vault-roots/.../vaults`; the rerun links immutable `memory.sqlite` and `archive/`
state, copies mutable manifests, and writes fresh answer/score artifacts in its own run root.

Relative `--registry-root`, `--run-root`, and `save-record --records-root` paths resolve from this
repo root, not from the caller's current working directory.

## Common Commands

List local runs:

```bash
cargo run --bin membench -- explore
```

Inspect one run:

```bash
cargo run --bin membench -- explore \
  --run-root runs/symbiotic-memory/long-mem-eval/500/baseline-clean
```

Import existing artifacts into the normalized registry:

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

Imports infer `{limit}` from `scored.json` and copy artifacts into `runs/.../artifacts/`. They do
not preserve noisy source paths in `run-params.json`.

Promote a scratch run to a tracked record:

```bash
cargo run --bin membench -- save-record \
  --run-root runs/symbiotic-memory/long-mem-eval/500/baseline-clean
```

The command is portable by default: it copies normalized metadata and known public artifacts,
rewrites artifact paths to the record, and omits executor-native vaults, queues, raw outputs, and
debug state. Absolute machine-local metadata paths are disclosed as `local://<basename>`. Use
`--include-native-state` only for an intentional local/private archive; that mode can be many GiB
and may contain raw prompts or debug data that must not be published.

Use `--force` only when intentionally replacing a record with a corrected version.

`docs/canonical-record-storage-task.md` tracks promotion of the first audited 500-question canonical
run and optional external retention of oversized native state.

Summarize queue timing from queue event JSONL:

```bash
cargo run --bin membench -- summarize-queue-events \
  --jsonl runs/symbiotic-memory/long-mem-eval/500/baseline-clean/provider-queue/model-queue-traces.jsonl
```

Run and report the first-class OpenRouter Qwen raw-embedding transport tuner:

```bash
scripts/run-embedding-transport-tuning.sh openrouter-qwen3-8b-1024 h1-32x32
scripts/report-embedding-transport-tuning.sh --profile openrouter-qwen3-8b-1024 --markdown
```

Run and report the equivalent DeepSeek chat transport tuner for distill-only runs:

```bash
scripts/run-chat-transport-tuning.sh deepseek-v4-flash-distill h2-64x32
scripts/report-chat-transport-tuning.sh --profile deepseek-v4-flash-distill --markdown
```

The current evidence-backed candidate for OpenRouter `qwen/qwen3-embedding-8b` raw embeddings is
documented in `docs/symbiotic-memory/openrouter-qwen-embedding-tuning.md`. The current DeepSeek
chat transport evidence is documented in
`docs/symbiotic-memory/deepseek-chat-transport-tuning.md`.

Dashboard-safe tuning records can be promoted without copying vaults, raw provider payloads, or
question-level artifacts:

```bash
scripts/save-run-meta-record.sh runs/symbiotic-memory/long-mem-eval/10/<run-name>
```

## Symbiotic Memory Runner

`membench` currently includes a first adapter for Symbiotic Memory's LongMemEval flow.

Current adapter status:

| Capability | Status | Notes |
| --- | --- | --- |
| Explicit `--smoke` run | wired | No network spend; maps internally to local deterministic providers and no scorer for adapter/run-shape smoke tests. |
| Fresh/resume/answer-only semantics | wired | Normal native runs re-ingest, `--resume` continues interrupted roots, and `--answer-only` regenerates answers from existing vault state. `--source-vault-root` links immutable vault data into a fresh run root for cheap isolated reruns. |
| Provider-backed LLM/Gemini ingestion | wired | Runs through `membench` native adapter and shared provider queues; benchmark subcommands stay out of `symem`. |
| Default paid Symbiotic Memory launch | wired | The CLI and dashboard default to `llm` distill + OpenRouter qwen3 embeddings + rerank + `score`. |
| Queued LongMemEval scoring | wired | Uses the same queued provider, retry, trace, and response-cache stack as memory answerer calls. |
| Explorer/import/save-record | wired | Reads normalized `benchmark-report.json` and `artifacts/`. |
| Provider queue/model trace export | wired | New native runs export `provider-queue/model-queue-traces.jsonl` as `artifacts/model-traces.jsonl`; dashboard live/detail also read provider queue traces directly for older runs. |

Queued scoring belongs in `membench`. The scorer uses the same durable provider queue, retry, trace,
and response-cache stack as memory answerer calls. The default judge worker fanout remains `400`, but
effective concurrency is capped by the shared model queue id, such as
`chat:deepseek:deepseek-v4-flash`.

Memory answer/distill/embed model defaults are owned by `../symbiotic-memory`. Benchmark profiles
should not repeat those bindings. Use env/model overrides only for explicit model-comparison or
tuning runs; the scorer/judge model remains benchmark-owned.

The reference clock passed to Symbiotic Memory recall defaults to the current RFC3339 datetime with
timezone for normal memory use. LongMemEval maps each row's `question_date` into an RFC3339
reference timestamp so temporal questions replay against the benchmark's pinned clock. Use
`MEMBENCH_REFERENCE_DATETIME` only when intentionally overriding that clock for a named experiment.

The default LongMemEval judge prompt mode is `official`: the per-question-type paper grader (see
`JUDGE.md`). The older generic semantic grader remains available as
`MEMBENCH_JUDGE_PROMPT_MODE=semantic` (aliases: `semantic-shared-compact`, `legacy`, `generic`) for
back-compat and A/B comparison. Each run records `judge_prompt_mode` in `scored.json` and the
dashboard surfaces it, because verdicts across judge modes are not comparable.

Judge cache prewarm is wired and opt-in: for DeepSeek rejudge or score-heavy runs,
`--prewarm-judge-cache 5 --prewarm-pause-secs 10` scores five temporary hypotheses into
`raw/judge-cache-prewarm/`, waits ten seconds, then runs the real scorer. Leave this disabled for
normal fully async ingest/answer runs unless scoring cost dominates.

Distill cache prewarm is not a separate default. Symbiotic Memory distill prompts already place their
stable instructions in a `cacheable_prefix` block before the variable source turns, and windowed
distill streams through the shared model queue. Use trace cache-hit/miss fields before adding any
blocking distill warmup.

## Async Adapter Contract

Benchmark adapters must preserve the system-under-test pipeline shape:

- do not duplicate memory ingestion, embedding, or recall logic inside `membench`;
- do not introduce a benchmark-only batch barrier between capture, Distillery, embedding, indexing,
  answer, and score;
- write artifacts, traces, queue events, and manifests incrementally as each stage completes;
- make fresh/resume/answer-only behavior explicit in `run-params.json`;
- record missing traces as missing instead of synthesizing them;
- keep provider concurrency governed by model queue ids, not by benchmark stage caps.

Useful modes and limits:

```text
--smoke  local no-network run-shape check
10       quick provider/debug slice
50       stratified debug slice
500      full LongMemEval run
```

The runner loads `.env.test.local` from this benchmark repository by default, sets
`MEMBENCH_PROVIDER_QUEUE_DIR` to `<run-root>/provider-queue` unless overridden, and only fills provider
queue/model defaults that the caller has not already set. It does not implicitly read sibling repo
env files.

Tracked templates:

```text
.env.example
.env.test.local.example
```

Both templates are safe to commit and contain placeholders only. Real `.env`, `.env.*`, and
`.env.test.local` files are ignored.

Use `--memory-config config/symbiotic-memory/longmemeval-raw-light.yaml` for the current
raw-light LongMemEval profile: fact top-k 20 and raw top-k 10,
DeepSeek Flash query planner, and foundation-owned provider/model queue defaults.

Embedding request sizing has two separate axes:

- `SYMBIOTIC_MEMORY__EMBED__BATCH_SIZE` and `SYMBIOTIC_MEMORY__EMBED__BATCH_MAX_CHARS` (the
  kit's `embed.batch_size` / `embed.batch_max_chars` config fields) control request packing only. Defaults
  are code-owned in `symbiotic-memory`. They are request-level throughput knobs: larger request char
  budgets reduce HTTP fanout, but each request can take longer and retries more work when it fails.
- `MEMBENCH_EMBED_MAX_CHARS` controls the local per-input text cap passed to the embedding provider. It
  is also code-owned and separate from request packing.

Do not use `SYMBIOTIC_MEMORY__EMBED__BATCH_MAX_CHARS` as an individual-input truncation cap, and do not use these
batch settings as substitute provider concurrency limits. Provider concurrency belongs to the model
queue id; workflow concurrency belongs to source-row fan-out.

Dashboard live monitor semantics:

- Provider Queue counts are latest state per queue item in the inspected trace window, not cumulative
  event counts. `running` should drop when each item writes a terminal `succeeded`, `failed`, or
  `dead` event.
- The per-queue rows show shared model queue ids, such as `chat:deepseek:deepseek-v4-flash` and
  `embedding:gemini:gemini-embedding-2`.
- Per-queue `rpm` is the number of provider requests started in the last trace minute of the
  inspected window. `maxrpm` is the highest 60-second start rate observed in that window. `peak` is
  observed peak running requests, not a configured provider limit.
- After a run completes, the Provider Queue panel switches to run-summary mode: `avg` is
  time-weighted average running requests, `avgrpm` is average request starts per minute across the
  inspected provider event span, and `peak`/`maxrpm` remain observed peaks.
- The Recent Activity panel interleaves memory-stage events and provider-queue events newest-first.
  Use it to confirm whether capture, raw embedding, distill, fact embedding, indexing, and answering
  are flowing independently or bunching at a join.
- Native completed runs keep a `LIVE` tab so the final provider queue and memory-stage snapshot can
  be inspected after the run leaves the in-flight list.
- The Memory Pipeline label `briefs` corresponds to the memory operation named `consolidate` in
  traces. It is the source-backed extractive brief pass, not a benchmark-only step.
- The Memory Pipeline label `prompt plan` corresponds to `query_plan`. It is the optional
  query-planner output from the memory engine, not a duplicate paid planner call from the benchmark.
  Trace metrics keep prompt/response hashes and the question-debug path; the raw planner
  system prompt, user prompt, response text, retrieval queries, and scored search responses live in
  that question-debug bundle. In the dashboard, open `QUESTIONS` and select a row to inspect those
  details without running another model call.
- Recall-native stages now include `answer embed`, `fact search`, `raw search`, `support`, and
  `answer ctx`. Those events are emitted by `RecallEngine`, not synthesized by the benchmark.

Default freshness rule:

```text
normal native run  -> fresh re-ingest
--resume           -> continue an interrupted run root
--answer-only      -> re-answer from an existing ingested run root
--source-vault-root runs/inputs/vault-roots/.../vaults
                   -> link immutable vault state for cheap isolated answer-only reruns
```

Answer-only native runs default to a `workflow_max_in_flight` of 500 so recall/answer comparisons
can fan out broadly. Full ingest/backfill runs use the configured memory workflow window, currently
50 in the raw-light profile, while provider queues still enforce the real model caps.

## Contract

Each benchmark adapter should report:

- `supported`: what the adapter can expose when configured, such as `ingest`, `retrieve`, `answer`,
  `provider_injection`, `embedding_injection`, `raw_context`, `retry_trace`, `token_usage`,
  `cache_usage`, `queue_events`, and `state_export`.
- `observed`: what this run actually captured, such as `model_calls`, `embedding_calls`,
  `retrieval_queries`, `retrieval_candidates`, `retrieval_scores`, `answer_prompt`,
  `answer_output`, `errors`, `retries`, `token_usage`, `cache_usage`, `timing`, `cost`, and
  `scoring_verdict`.

Do not use a trace-depth level. Use capability flags because partial instrumentation is normal when
wrapping external systems.

## Trace Files

Adapters can write memory operation traces as JSONL:

```text
memory-traces.jsonl
```

Important fields:

- `source_system`: system under test, such as `symbiotic-memory`, `mem0`, or `hymem`.
- `instrumentation`: `native_stage`, `wrapped_api`, `provider`, or `imported`.
- `operation`: normalized operation such as `capture`, `distill`, `write_archive`, `embed_raw`,
  `embed_facts`, `index`, `retrieve`, `answer`, `model_call`, `embedding_call`, or `vector_search`.
- `event`: `operation_started`, `operation_succeeded`, `operation_failed`, `branch_started`,
  `branch_joined`, or batch variants.
- `input_hash`, `output_hash`, `item_count`, `metrics`, and `error`: forensic fields.

Store raw content only in explicit local-debug modes owned by the adapter.

## Costs And Timings

Cost is derived from:

- exact model id after routing or fallback;
- input tokens;
- cached input tokens, when exposed by the provider;
- output tokens;
- pricing table version used by the run.

When a provider trace includes `cost_micro_usd`, membench uses the provider-reported value. When
the trace has token buckets but no explicit cost, membench estimates cost from the built-in pricing
catalog and marks the rollup as estimated. The current built-in catalog is
`official-pricing-2026-06-19`:

- DeepSeek API official pricing for `deepseek-v4-flash` and `deepseek-v4-pro`, including cache-hit,
  cache-miss, and output-token prices.
- Gemini API official pricing for `gemini-embedding-2` standard and batch text-input prices.

Runs without token usage for a model stay unpriced for that model. New Symbiotic Memory Gemini
embedding traces should include input tokens from Gemini `countTokens`; old traces may remain
unpriced because they recorded queue events before embedding token usage existed.

Queue traces should preserve timestamps for queued, running, succeeded, and failed events. The crate
can derive queue wait time, run time, total time, attempts, and final status by grouping events by
queue id and item id.

## Leaderboard

The publishable leaderboard is the `membench.leaderboard.v1` export over tracked `records/`
(see `docs/schemas.md`). Every ranked row carries a verification level; runs whose scores
cannot be independently reproduced from tracked artifacts are listed as unranked with the
reason. The dashboard bundles a snapshot at `dashboard/public/data/leaderboard.json` and,
when served statically without the API, renders it explicitly labeled as a static snapshot —
verified cohorts stay empty until a record passes the review gate in
`docs/longmemeval-methodology.md`, which also states the honest current result.

## License

Apache-2.0 (`LICENSE`). Contributions are welcome — see `CONTRIBUTING.md`; security reports
via `SECURITY.md`; release process in `RELEASING.md`.

## More Docs

- `AGENTS.md`: exact operating rules for coding agents.
- `docs/longmemeval-methodology.md`: scoring methodology, honest current result, and the
  leaderboard review gate.
- `docs/oss-release-handoff.md`: external decisions/blockers for taking the repo public.
- `docs/run-registry.md`: run layout and lifecycle reference.
- `docs/symbiotic-memory/openrouter-qwen-embedding-tuning.md`: OpenRouter Qwen raw-embedding
  transport tuning evidence and reproduction scripts.
- `docs/symbiotic-memory/deepseek-chat-transport-tuning.md`: DeepSeek Flash distill chat transport
  tuning evidence and reproduction scripts.
- `docs/schemas.md`: JSON field reference for run params, reports, artifacts, and traces.
- `docs/bench-explorer-design.md`: explorer, comparison, and viewer design (incl. the web dashboard).
- `dashboard/README.md`: dashboard develop/build/run instructions.
- `skills/membench/SKILL.md`: Codex skill for running and inspecting this benchmark harness.
