---
name: membench
description: Use when Codex needs to run, inspect, import, validate, compare, or prepare publication artifacts for memory-system benchmarks with the symbiotic-mem-bench repository; triggers include membench, memory benchmark, LongMemEval, benchmark runs, run registry, benchmark-report.json, run-params.json, artifact_manifest, score artifacts, provider queue traces, or Symbiotic Memory benchmark workflows.
---

# Membench

## Core Workflow

Work from the `symbiotic-mem-bench` repository root when possible. If called from a sibling repo,
use `--manifest-path ../symbiotic-mem-bench/Cargo.toml`.

Use `membench` for benchmark orchestration and inspection. Do not recreate old Python scoring
scripts, manually enter scores, or infer missing traces. Missing artifacts must stay missing and be
declared through `artifact_manifest`.

## First Checks

1. Read `AGENTS.md` in the benchmark repo.
2. Read `docs/run-registry.md` for layout questions.
3. Read `docs/schemas.md` for JSON fields, traces, or artifact completeness questions.
4. Use `CARGO_TARGET_DIR=/tmp/symbiotic-mem-bench-target` for validation so the repo stays clean.

## Common Tasks

List known scratch runs:

```bash
cargo run --bin membench -- explore
```

Inspect one run:

```bash
cargo run --bin membench -- explore \
  --run-root runs/{system}/{benchmark}/{limit}/{run_name}
```

Run the standard validation gate:

```bash
CARGO_TARGET_DIR=/tmp/symbiotic-mem-bench-target cargo fmt -- --check
CARGO_TARGET_DIR=/tmp/symbiotic-mem-bench-target cargo test
CARGO_TARGET_DIR=/tmp/symbiotic-mem-bench-target cargo clippy --all-targets -- -D warnings
```

For detailed commands and run-shape rules, read `references/membench-commands.md`.

## Environment

Paid provider-backed runs load `.env.test.local` from the benchmark repository by default. Create it
from a tracked template:

```bash
cp .env.example .env.test.local
```

Never commit real `.env` files. Use `--env-file path/to/file` only when intentionally testing an
alternate environment.

## Run Integrity Rules

- Scratch runs live under `runs/{system}/{benchmark}/{limit}/{run_name}/` and are ignored by git.
- Tracked records live under `records/{system}/{benchmark}/{limit}/{run_name}/`.
- Standard LongMemEval runs may omit `--dataset`; `membench` auto-downloads the cleaned S dataset to
  ignored local storage under `runs/inputs/longmemeval-cleaned/` when missing.
- `membench` does not auto-download `symbiotic-memory`; the current adapter uses the sibling local
  crate dependency `../symbiotic-memory`.
- LongMemEval small runs default to `--sample stratified`, not first-N row order. Use `--sample first`
  only for exact row-order reproductions.
- Native runs re-ingest by default; state reuse must be explicit with `--resume` or `--answer-only`.
  For cheap isolated answer-only comparisons, pass `--source-vault-root
  runs/inputs/vault-roots/.../vaults`; membench links immutable `memory.sqlite` and `archive/`
  data into the new run root and writes fresh hypotheses, debug, traces, and scores there.
- Default Symbiotic Memory launches are paid, provider-backed, and scored: `llm` distill, `gemini`
  embeddings, answerer/routed/consolidated retrieval enabled, and `score=true`.
- Paid/provider-backed benchmark runs must use Cargo release mode (`cargo run --release ...`).
  Dev-mode runs are acceptable only for smoke/debug/inspection commands.
- Symbiotic Memory adapter status today: explicit local `--smoke` runs and the default paid
  LLM/Gemini ingestion/scoring path are wired in this repo. Compact normalized
  `artifacts/model-traces.jsonl` export is still pending; provider queue logs are present under
  `provider-queue/model-queue-traces.jsonl`.
- Local `--smoke` adapter runs map internally to local deterministic providers and no scorer.
  Without an explicit `--run-root` or hidden `--keep-smoke-run`, they execute outside the
  registry and delete themselves after success; they should not appear in dashboard run lists.
- Check `run-params.json` before interpreting a run: `configured_models` records requested YAML
  settings, while `runtime_models` records what the adapter actually invoked. Current local smoke
  runs do not prove DeepSeek/Gemini use even when the config names those providers.
- `workflow/longmemeval/queue.sqlite` is durable row workflow state. Provider/model queue logs live
  separately under `provider-queue/model-queue-traces.jsonl` and are present only for actual queued
  model calls.
- LongMemEval scoring should default to `judge_prompt_mode=semantic-shared-compact` once the scorer
  is used; use `SYMEM_JUDGE_PROMPT_MODE=official` only for original-prompt audit comparisons.
- Judge cache prewarm is wired but must remain opt-in: use `--prewarm-judge-cache 5
  --prewarm-pause-secs 10` for DeepSeek rejudge/score-heavy runs, not as a default ingest/answer
  behavior.
- Imported runs can be artifact-only; `artifact_manifest.native_state_available` must be `false`.
- Never commit `runs/`, `.debug-session/`, `target/`, provider queues with secrets, local datasets,
  raw prompts, or raw secret-bearing traces.
- Preserve repo-relative paths in reports and records.
- Native adapters must preserve async pipeline behavior: no benchmark-owned ingestion/recall
  duplication, no artificial batch barrier, stage outputs and traces written incrementally, and
  provider caps controlled by model queue id.
- Symbiotic Memory model/provider defaults belong in `../symbiotic-memory` code/config. Benchmark
  profiles should not repeat answer/distill/embed model defaults; add role/model env overrides only
  for an explicitly named tuning arm or provider comparison.
- Native adapters write executor-owned outputs directly under `raw/`; do not add root-level
  temporary outputs that need a cleanup pass.
- For LongMemEval native runs, hypotheses and memory `answer` traces are controlled by
  `answer_output`; the legacy `answerer` flag means generative-answerer policy, not whether answers
  exist. Scored/judged output is controlled by `score_output`/`score`.
- Dashboard commands should use `node` and `npm` from the user `nvm` install already on PATH; do
  not install or rely on Homebrew Node.
- Dashboard command previews should target the short native `membench` command and omit harness-owned
  defaults such as dataset, run root, raw output, prompt dir, provider queue dir, and fresh mode.

## Cost And Pricing Interpretation

Use the dashboard/API cost fields for run-level cost. Do not manually multiply screenshots or guess
from provider dashboards unless debugging a provider-side discrepancy.

Cost source rules:

- If a trace row has `cost_micro_usd`, that provider-reported value wins.
- If a trace row has token usage but no explicit cost, membench estimates cost from its built-in
  pricing catalog and marks the rollup with `cost_estimated: true`.
- If a model has no token usage, that model stays unpriced. Do not treat missing usage as zero-cost.
- New Symbiotic Memory Gemini embedding traces should include `usage.prompt_tokens` from Gemini
  `countTokens`, so embedding cost should be estimated. Old native Gemini embedding traces may show
  thousands of embedding calls with no cost because those queue rows did not record embedding
  input-token usage.

Current built-in catalog:

- `official-pricing-2026-06-19`
- DeepSeek official API pricing for `deepseek-v4-flash` and `deepseek-v4-pro`, including cache-hit
  input, cache-miss input, and output tokens.
- Gemini official API pricing for `gemini-embedding-2` standard and batch text-input prices.

Useful inspection command:

```bash
curl -s 'http://127.0.0.1:8787/api/run?id=runs%2Fsymbiotic-memory%2Flong-mem-eval%2F10%2F{run_name}' \
  | jq '{cost:.cost.cost_micro_usd, estimated:.cost.cost_estimated, pricing:.cost.pricing_table_version, models:[.cost.models[] | {model,calls,cost_micro_usd,cost_estimated,input_tokens,cached_input_tokens,output_tokens}]}'
```

When investigating unexpected spend, always compare:

- `run-params.json` `configured_models` and `runtime_models`;
- `provider-queue/model-queue-traces.jsonl` queue ids and successful-call counts;
- dashboard/API `cost.models[]`, especially `cached_input_tokens` versus `input_tokens`.

## Publication Pass

Before calling a run or repo publish-ready:

```bash
CARGO_TARGET_DIR=/tmp/symbiotic-mem-bench-target cargo fmt -- --check
CARGO_TARGET_DIR=/tmp/symbiotic-mem-bench-target cargo test
CARGO_TARGET_DIR=/tmp/symbiotic-mem-bench-target cargo clippy --all-targets -- -D warnings
cargo run --bin membench -- explore
git status --short --ignored
```

Then run the publication stale-reference scan from `AGENTS.md`.
