# Benchmark Explorer Design

## Goal

Build a benchmark explorer that makes memory-system runs inspectable without opening raw JSON files.
The explorer should work for native Symbiotic Memory runs, imported artifacts, and later adapters such
as mem0 or HyMem.

## Data Model

Every local run lives under the bench repo's ignored registry root:

```text
runs/{system}/{benchmark}/{limit}/{run_name}/
```

See `docs/run-registry.md` for the canonical run and record shape.

Relative registry paths are resolved from the `symbiotic-mem-bench` repo root, not from the caller's
current working directory.

Each run folder contains:

- `run-params.json`: exact parameters used to create or import the run.
- `benchmark-report.json`: normalized metrics, run metadata, and artifact summaries.
- `artifacts/`: copied source artifacts such as hypotheses, scored output, provenance, debug traces,
  queue events, and question-debug bundles.
- `artifact_manifest`: present in report JSON, and also in imported run params, to distinguish
  native runs from artifact-only imports and list missing artifact classes.

The registry is append-friendly. New runs add folders; they do not mutate baselines.

## Known Runs

The explorer should support named run sets:

- `baseline`: accepted comparison points for a benchmark.
- `latest`: most recent complete run per system and benchmark.
- `candidate`: current experimental run.
- `imported`: converted external or frozen artifacts.

Known runs can be represented by a small registry index file later:

```json
{
  "schema": "membench.registry.v1",
  "baselines": [
    {
      "system": "symbiotic-memory",
      "benchmark": "long-mem-eval",
      "limit": 500,
      "run_name": "baseline-clean",
      "run_root": "records/symbiotic-memory/long-mem-eval/500/baseline-clean"
    }
  ]
}
```

The first implementation can infer known runs by scanning `benchmark-report.json` files. The index is
only needed when we want curated labels such as "official baseline" or "best clean run".

## Views

### Run List

Show every known run with:

- system
- benchmark
- run name
- run kind
- accuracy and task-averaged accuracy
- timestamp when available
- artifact completeness
- root path

### Run Detail

Show:

- run parameters
- model/provider settings
- queue settings
- total score
- category scores when present
- artifacts and hashes
- warnings such as missing score, missing provenance, or missing question debug files

### Baseline Comparison

Compare a candidate run against one or more baselines:

- overall delta
- category deltas
- changed verdicts
- newly correct answers
- newly wrong answers
- unchanged wrong answers
- abstention changes

### Question Browser

For each question:

- question id
- question text
- gold answer
- candidate answer
- baseline answer
- verdict and judge rationale when present
- retrieved facts and raw turns
- answerer prompt and response
- model usage, cache hits, retries, and timing

Rows should be filterable by:

- correct / wrong / changed
- category
- route
- model
- high token use
- retries or errors
- missing evidence

## UI Options

### Terminal Explorer

Good first version. It should support:

- listing known runs
- showing one run
- comparing two runs
- filtering wrong or changed answers
- opening the question-debug path for a row

### TUI

Good for local debugging once the terminal explorer becomes crowded:

- left pane: runs
- middle pane: questions
- right pane: answer/evidence/debug detail

### Web Dashboard (implemented)

A Bloomberg-terminal-style web dashboard lives in `dashboard/`. It is a no-SSR Svelte 5 + Vite SPA
served by the Rust `membench-server` binary (`cargo run --features server --bin membench-server`),
which reads the same `benchmark-report.json`, `run-params.json`, and artifact files as the CLI — no
second data format. Two screens:

- **Leaderboard** — cohort picker (benchmark + size), ranked field, per-category matrix, and a
  head-to-head compare rail (radar + per-category deltas). Flags cohorts that are not strictly
  comparable (mixed `dataset_fingerprint` or `judge_model`).
- **Debugger / Tuner / Runner** — run tree, run overview (params, cohort/models, artifact manifest,
  cost/timing rollup), a filterable question browser, baseline comparison, model/memory/queue
  traces, and a parameter tuner that previews the exact `symem` command (live execution is the next
  phase).

Backend endpoints (all under `/api`): `runs`, `leaderboard`, `run`, `run/questions`,
`run/artifact`, `run/traces`, `compare`, `runner/schema`, `runner/plan`. The pure aggregation logic
(`src/registry.rs`, `cohort.rs`, `compare.rs`, `cost.rs`, `leaderboard.rs`, `runner.rs`) is shared
with the CLI, so `membench explore --json` returns the same index the dashboard serves.

See `dashboard/README.md` for develop/build instructions.

## Non-Goals

- Do not hard-code benchmark names into viewer logic beyond display defaults.
- Do not require manual score entry when a scored artifact exists.
- Do not hide missing artifacts; incomplete runs must be visibly incomplete.
- Do not treat imported artifacts as a separate fake system. They keep their real `system` and
  `benchmark` and use `run_kind = "imported-artifact"`.

## First Build Sequence

1. Stabilize registry writes for native and imported runs.
2. Add terminal explorer list/detail views.
3. Add compare view over normalized scored outputs.
4. Add question-debug bundle ingestion and wrong-answer browsing.
5. Build a small TypeScript local web explorer over the same files.
