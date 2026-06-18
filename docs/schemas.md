# Schemas

This document describes the stable JSON files written into each benchmark run folder.

## Run Params

`run-params.json` records how a run was created or imported.

Common fields:

| Field | Meaning |
|---|---|
| `schema` | Schema id, currently `membench.run_params.v1`. |
| `system` | Memory system under test, such as `symbiotic-memory`. |
| `benchmark` | Benchmark id, such as `long-mem-eval`. |
| `run_kind` | `native` for runs executed by an adapter, `imported-artifact` for copied comparison artifacts. |
| `run_name` | Folder-safe human name for the run. |
| `run_root` | Repo-relative run folder path when the folder is inside this repository. |
| `limit` | Intended or scored question count. |

Native Symbiotic Memory runs also record adapter settings such as `dataset`, `distiller`,
`embedder`, `store`, `answer_output`, `generative_answerer_enabled`, `routed`, `query_planner`,
`score_output`, `scorer`, and `judge_workers`. `answer_output=true` means the benchmark run writes
hypotheses and emits the memory `answer` stage; `generative_answerer_enabled` controls whether the
memory engine uses its generative answerer policy. The older `answerer` field is retained as a
compatibility alias for `generative_answerer_enabled`. `score_output=true` means judge/scored
artifacts are expected. When enabled, `prewarm_judge_cache` and `prewarm_pause_secs` describe the
optional same-scorer cache warmup performed immediately before the real score.

Provider fields are split intentionally:

| Field | Meaning |
|---|---|
| `configured_models` | Model/provider settings requested through the memory config or CLI. This is intent, not proof of provider use. |
| `runtime_models` | Providers actually invoked by this adapter run. Current local `--smoke` runs use local deterministic providers and extractive or disabled-chat answer fallback. |
| `runtime_provider_note` | Human-readable warning explaining the configured/runtime distinction. |
| `provider_queue_available` | `true` only when provider queue traces are expected from actual model calls. |
| `workflow_queue_available` | `true` when the native workflow queue is expected to exist. |
| `ephemeral_smoke_run` | `true` for local no-network smoke tests that are deleted after successful validation. These should not appear in normal dashboards or records. |

Imported runs include `imported_artifacts` booleans and `artifact_manifest` so artifact-only
comparison records are self-describing.

## Benchmark Report

`benchmark-report.json` is the primary index read by `membench explore`.

Top-level fields:

| Field | Meaning |
|---|---|
| `schema` | Schema id, currently `membench.report.v1`. |
| `system` | Memory system under test. |
| `benchmark` | Benchmark id. |
| `run_kind` | `native` or `imported-artifact`. |
| `run_name` | Run name. |
| `run_params` | Embedded copy of the relevant run params. |
| `metrics` | Normalized score fields. |
| `artifact_manifest` | Artifact completeness summary. |
| `artifacts` | Per-artifact hashes, sizes, line counts, and paths. |
| `created_at` | RFC3339 timestamp written when the report was finalized. |
| `cohort` | Comparability identity (see below). |
| `models` | Resolved role models (`answer`, `distill`, `embed`, `judge`) when derivable. |
| `config_signature` | Hash of the comparable configuration knobs (groups "same params" runs). |

`metrics.accuracy` uses:

```json
{
  "correct": 471,
  "total": 500,
  "value": 0.942
}
```

When available, reports also include `task_averaged_accuracy` and `abstention_accuracy`.
Native runs with model traces also carry `metrics.cost_micro_usd`, `metrics.latency_ms_p50`,
and `metrics.latency_ms_p95`, derived from `model-traces.jsonl`.

## Cohort Identity

`cohort` lets tooling decide which runs are fairly comparable on a leaderboard. Two runs belong to
the same cohort when they share a benchmark, size, question set, judge model, and judge prompt mode.

```json
{
  "dataset_fingerprint": "d5b13b130fdf…",
  "judge_model": "deepseek-v4-flash",
  "judge_prompt_mode": "semantic-shared-compact"
}
```

| Field | Meaning |
|---|---|
| `dataset_fingerprint` | SHA-256 over the sorted question-id set the run covered. Equal fingerprints mean "the same questions" — verifiable rather than assumed. |
| `judge_model` | The judge model from `scored.json`. |
| `judge_prompt_mode` | The judge prompt/rubric mode from `scored.json`, such as `semantic-shared-compact` or `official`. |

These fields are additive to `membench.report.v1`; readers that do not understand them ignore them,
and runs written before the upgrade simply omit them (the explorer derives them on the fly from
artifacts).

## Machine-readable Index

```bash
cargo run --bin membench -- explore --json
```

Emits the normalized run index (one `RunSummary` per run) as JSON — the same shape the dashboard
backend serves at `GET /api/runs`. Without `--run-root` it scans both `runs/` and `records/`.

## Artifact Manifest

`artifact_manifest` answers two questions: what did this run capture, and what is missing?

```json
{
  "available": ["hypotheses", "scored", "verdicts"],
  "missing": ["provenance", "memory_traces", "model_traces", "score_summary"],
  "native_state_available": false,
  "native_state_note": "Imported artifact runs preserve copied benchmark artifacts only; native state folders such as raw, vaults, workflow, and provider-queue may be absent."
}
```

Common artifact kinds:

| Kind | File |
|---|---|
| `hypotheses` | `artifacts/hypotheses.jsonl` |
| `scored` | `artifacts/scored.json` |
| `verdicts` | `artifacts/verdicts.jsonl` |
| `partial_verdicts` | `artifacts/partial-verdicts.jsonl` |
| `provenance` | `artifacts/provenance.jsonl` |
| `memory_traces` | `artifacts/memory-traces.jsonl` |
| `model_traces` | `artifacts/model-traces.jsonl` |
| `score_summary` | `artifacts/score-summary.json` |

Do not synthesize traces. If a wrapped external system cannot expose internal retrieval details, keep
the artifact missing and let the manifest say so.

## Artifact Summaries

Each entry under `artifacts` has:

| Field | Meaning |
|---|---|
| `kind` | Artifact kind from the manifest vocabulary. |
| `path` | Repo-relative path when possible. |
| `bytes` | File size in bytes. |
| `non_empty_lines` | Count of non-empty text lines. |
| `sha256` | Content hash. |

## Memory Trace JSONL

Memory traces are optional JSONL records for operation-level debugging.

Important fields:

| Field | Meaning |
|---|---|
| `schema_version` | Numeric schema version. |
| `trace_id` | Stable event id. |
| `parent_trace_id` | Parent event id, when nested. |
| `source_system` | System under test. |
| `instrumentation` | `native_stage`, `wrapped_api`, `provider`, or `imported`. |
| `run_id` | Run identifier. |
| `question_id` | Benchmark question id, when applicable. |
| `operation` | Normalized stage such as `capture`, `distill`, `retrieve`, `answer`, or `score`. |
| `event` | Lifecycle event such as `operation_started`, `operation_succeeded`, or `operation_failed`. |
| `attempt` | Attempt number for retries. |
| `timestamp` | Event timestamp. |
| `input_hash` / `output_hash` | Hashes for forensic correlation without storing raw content. |
| `model_trace_ids` / `queue_item_ids` | Links to provider queue or model traces. |
| `metrics` | Adapter-specific structured measurements. |
| `error_class` / `error` | Failure data when the event failed. |

Raw prompts, raw documents, and secrets should stay out of tracked records unless a future
local-only debug mode marks them explicitly.

## Async Stage Expectations

Native adapters should emit enough trace and state to show the asynchronous flow:

| Operation | Meaning |
|---|---|
| `capture` | source material accepted and normalized |
| `write_receipt` | durable raw source receipt written |
| `distill` | source-backed memory facts extracted |
| `write_archive` | Archive Markdown or equivalent durable memory truth written |
| `embed_raw` | raw source unit embeddings produced and persisted |
| `embed_facts` | fact/search text embeddings produced and persisted |
| `index` | derived recall index updated |
| `retrieve` | retrieval query and candidates produced |
| `answer` | final answer or explicit unavailable result produced |
| `score` | benchmark judgment produced |

Expected event progression is `operation_started` followed by `operation_succeeded` or
`operation_failed`, with retry attempts represented as new events using the same logical input hash
or queue item id. Branches such as `embed_raw` and `distill -> write_archive -> embed_facts` may
overlap. Tools must not infer failure from missing later stages alone; use durable state and trace
events.

## Queue Events

Provider/model queue event JSONL can be summarized with:

```bash
cargo run --bin membench -- summarize-queue-events \
  --jsonl runs/{system}/{benchmark}/{limit}/{run_name}/provider-queue/model-queue-traces.jsonl
```

Required queue fields are `queue_id`, `item_id`, `operation` or `kind`, `status`, `attempt`, and
`timestamp`. Optional fields include `model`, `input_hash`, `usage`, `cost_micro_usd`, and `error`.

Native Symbiotic Memory runs may also have a workflow queue at
`workflow/longmemeval/queue.sqlite`. This queue records durable row-level work such as pending,
running, retry, and succeeded states. It does not imply that a model/provider call happened; model
calls require `model-traces.jsonl` or `provider-queue/model-queue-traces.jsonl`.
