---
name: membench
description: Use when Codex needs to run, inspect, import, validate, compare, derive Trials, or prepare publication artifacts for memory-system benchmarks with the symbiotic-mem-bench repository; triggers include membench, memory benchmark, LongMemEval, benchmark runs, run registry, benchmark-report.json, run-params.json, artifact_manifest, score artifacts, provider queue traces, trial-stack.json, trials.jsonl, trial-question-deltas.jsonl, or Symbiotic Memory benchmark workflows.
---

# Membench

Shared collaboration, Git, verification, and security rules: House Rules (`~/p/house-rules/AGENTS.md`).

## Read First

- [AGENTS.md](../../AGENTS.md) owns repository boundaries, validation commands, and settled
  campaign decisions.
- [Command reference](references/membench-commands.md) owns runnable workflow examples.
- [Run registry](../../docs/run-registry.md) owns paths and artifact completeness;
  [schemas](../../docs/schemas.md) describes report, trace, and debug fields.
- [Environment](../../docs/environment.md) describes env-file loading, engine configuration,
  and model/provider ownership.

## Choose the Workflow

Work from the repository root. The public root builds the core/server/leaderboard without
private dependencies. Native `membench` commands use
`cargo run --manifest-path adapters/symbiotic-memory/Cargo.toml --bin membench -- ...` and
require repository access to the pinned Memory dependency. Use release for paid runs.

- Inspect existing runs with `explore`; import frozen artifacts with `--import-report`.
- Native execution is paid/scored by default. Only explicit `--smoke` selects local providers;
  smoke does not prove any configured cloud model was invoked.
- State reuse is explicit: `--resume` or `--answer-only`. For answer-only comparisons,
  `--source-vault-root` must name an existing compatible zvec vault root. The runner copies
  `*.zvec` collections and manifests and links the immutable archive; new answers/debug stay
  in the new run root. Old SQLite/zvec-hybrid vaults require fresh ingestion.
- Derive trial ledgers with `trials derive`, rather than writing scores or deltas manually.
  `TRIAL` rows are diagnostic experiments; publication requires a full run and the
  [ranking review gate](../../docs/longmemeval-methodology.md).
- Promote with `save-record`, which defaults to portable artifacts without native state.
  Use [AGENTS.md publication checks](../../AGENTS.md#publication-hygiene) and
  [RELEASING.md](../../RELEASING.md) for release preparation.
- Local validation follows [AGENTS.md](../../AGENTS.md#validate), in CI's debug profile.
  CI owns the full core/server test suites.

## Diagnose a Run

Compare `run-params.json`'s `configured_models` (requested) and `runtime_models` (invoked).
Provider calls are evidenced by `artifacts/model-traces.jsonl` or the live/fallback
`provider-queue/model-queue-traces.jsonl`, not by config names.

The memory engine's query-planner result supplies `query_plan` trace hashes/pointers.
Per-question `vaults/{question}/debug/.../question-debug.json` stores
`recall.query_planner_call.{system_prompt,user_prompt,response_text}`, retrieval queries,
and initial/fallback profile diagnostics. Do not invoke an extra planner for display.
The dashboard reads question debug lazily. Its `answer embed` label maps to `embed_query`.

Embedding request packing (`SYMBIOTIC_MEMORY__EMBED__BATCH_SIZE` / `BATCH_MAX_CHARS`) is
separate from per-input truncation (`MEMBENCH_EMBED_MAX_CHARS`) and model-queue concurrency.
LongMemEval's reference clock comes from each row's `question_date`; override with
`MEMBENCH_REFERENCE_DATETIME` only for a named experiment.

## Cost Interpretation

Use the dashboard/API `cost` rollup, implemented in [src/cost.rs](../../src/cost.rs).
Provider-reported `cost_micro_usd` wins; token-priced estimates carry `cost_estimated: true`.
Missing token usage stays unpriced, not zero-cost. The static table version is defined by
`PRICING_TABLE_VERSION`; the OpenRouter catalog is
[config/pricing/openrouter-pricing.json](../../config/pricing/openrouter-pricing.json),
refreshed by [scripts/refresh-pricing.sh](../../scripts/refresh-pricing.sh).
See [DeepSeek cost accounting](../../docs/deepseek-cost-accounting.md) for cache buckets.

Inspect one run (replace `{run_name}`):

```bash
curl -s 'http://127.0.0.1:8787/api/run?id=runs%2Fsymbiotic-memory%2Flong-mem-eval%2F10%2F{run_name}' \
  | jq '{cost:.cost.cost_micro_usd, estimated:.cost.cost_estimated, pricing:.cost.pricing_table_version, models:[.cost.models[] | {model,calls,cost_micro_usd,cost_estimated,input_tokens,cached_input_tokens,output_tokens}]}'
```

For unexpected spend, compare configured/runtime bindings, queue ids and successful-call
counts, and per-model input/cache/output token buckets. Older embedding traces may lack
usage and cannot retrospectively prove cost.
