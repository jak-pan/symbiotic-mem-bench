# Tracing/Data-Artifact Model Design Sprint — Merged Sweep Synthesis

> **Status:** Evidence-backed synthesis merged from local audit plus three parallel sweeps: persisted status quo, dashboard/API consumer taxonomy, and observability best practices. This is a design sprint document, not an implementation patch.
>
> **User correction incorporated:** this is not an endpoint cleanup. We are designing an agnostic benchmark/debugging observability system for multi-prong/hybrid/agentic/memory systems. Raw traces are only one artifact family. Charts, distributions, heat maps, question response traces, gold/scored/verdict records, and graph/aggregation outputs are distinct data artifacts with distinct consumers.

## Goal

Design a durable tracing + data artifact model for `symbiotic-mem-bench` that can generalize beyond current memory benchmarks into an agnostic benchmark debug tool.

The system should support:

- Raw event/trace streams with common envelope fields and typed per-trace payloads.
- Provider traces with provider/model/token/cache/cost/timing fields.
- Memory/recall traces with memory-specific operation, retrieval, ranking, context, and store/index fields.
- Runner/workflow traces with orchestration, scheduling, retry, timeout, queue, artifact-generation, and question lifecycle fields.
- Question-centric joins: question id should map to gold answer, verdict, hypothesis, provenance, raw data, debug bundle, traces, retrieved evidence, and generated answer.
- Constructed artifacts: summaries, distributions, waterfalls, charts, heatmaps, graphs, bottlenecks, and per-question response traces should be explicitly materialized/cached artifacts, not confused with raw traces.
- Efficient APIs: fetch raw traces separately by type/page/search; fetch summaries/views separately; avoid one huge ambiguous `/traces` blob.

## Current local evidence

Run inspected:

```text
runs/symbiotic-memory/long-mem-eval/500/collapse-pplx
```

### Persisted files and duplicates

Observed by reading the actual run directory:

```text
artifacts/memory-traces.jsonl              size=12,883,644  lines=16,288  sha16=0578127fa9b8859f
traces/memory-events.jsonl                 size=12,883,644  lines=16,288  sha16=0578127fa9b8859f

artifacts/model-traces.jsonl               size=2,619,994   lines=7,506   sha16=a59836166b4358a7
provider-queue/model-queue-traces.jsonl    size=2,619,994   lines=7,506   sha16=a59836166b4358a7

workflow/longmemeval/queue.sqlite          size=614,400     sha16=59e69c5bb581222d
artifacts/step-analytics.json              size=12,862,392  sha16=00b26751be782468
benchmark-report.json                      size=12,463      sha16=03c62c02197ea99b
run-params.json                            size=7,588       sha16=9618fe92dc939d14
artifacts/gold-eval.json                   size=300,060     sha16=82fc6c5cb92efbc7
artifacts/verdicts.jsonl                   size=580,256     lines=500     sha16=b7cdf81e814c748e
artifacts/scored.json                      size=993         sha16=903f2c8a5c218d09
artifacts/provenance.jsonl                 size=1,475,106   lines=500     sha16=50260cbfb79d9da8
```

Evidence: memory trace and `traces/memory-events` are byte-identical; model/provider trace and provider queue trace are byte-identical. That suggests either historical migration residue or multiple naming conventions for the same stream. The design needs to decide canonical paths and compatibility policy.

### Observed raw memory trace shape

Every memory row had these 24 top-level keys in this run:

```text
schema_version, trace_id, parent_trace_id, source_system, instrumentation, run_id,
question_id, source_id, operation, stage, event, attempt, timestamp, started_at,
finished_at, duration_ms, input_hash, output_hash, item_count, model_trace_ids,
queue_item_ids, metrics, error_class, error
```

First row shape:

```json
{
  "schema_version": 1,
  "trace_id": "fe5d53384b2126caeb4b5993cce32bf143e154a37281e703e99253e016840733",
  "parent_trace_id": null,
  "source_system": "symbiotic-memory",
  "instrumentation": "native_stage",
  "run_id": "hypotheses",
  "question_id": "e47becba",
  "source_id": "e47becba",
  "operation": "adapter_call",
  "stage": "pre_capture_setup",
  "event": "operation_started",
  "attempt": 1,
  "timestamp": "2026-06-29T20:19:36.020974Z",
  "started_at": "2026-06-29T20:19:36.020975Z",
  "finished_at": null,
  "duration_ms": null,
  "input_hash": null,
  "output_hash": null,
  "item_count": null,
  "model_trace_ids": [],
  "queue_item_ids": [],
  "metrics": { "store_backend": "zvec-hybrid" },
  "error_class": null,
  "error": null
}
```

Memory trace operation counts:

```text
fact_search        4,144
raw_search         4,144
adapter_call       2,000
query_plan         1,000
embed_query        1,000
rerank             1,000
support_check      1,000
answer_context     1,000
answer             1,000
```

Memory trace event counts:

```text
operation_started    8,144
operation_succeeded  8,144
```

Memory metrics are sparse and operation-dependent. Top metric keys include:

```text
lane, query, top_k, best_score, result_count, fact_count, store_backend,
raw_turn_count, recall_profile, incremental_ingest_completed, planner_mode,
query_count, total_query_chars, candidate_type, candidates_submitted,
overall_top_k, fact_top_k, raw_turn_top_k, context_item_count, answerer_enabled,
create_vault_dir_ms, embedding_dimensions_ms, existing_fact_count,
existing_turn_count, load_existing_ms, manifest_ms, source_hash_ms,
store_open_ms, store_sqlite_open_ms, zvec_cache_ms, ensure_recall_index_ms,
retrieval_query_count, dense_query_count, sparse_term_count, rerank_ms,
support, context_chars, answer_chars, finish_reason
```

Implication: a single flat `TraceEventRow` is too lossy for raw memory traces. Better model: common trace envelope + typed payload variant, e.g. `MemoryTracePayload` with a structured common subset and extensible metrics/attrs.

### Observed provider trace shape

Provider/model queue trace rows have these common keys in the run:

```text
queue_id, item_id, operation, status, attempt, timestamp, request_units,
input_units, input_hash
```

Additional keys on subsets:

```text
queue_wait_ms, throttle_wait_ms, run_ms, usage, error
```

Operation counts:

```text
chat       4,506
embedding  1,500
rerank     1,500
```

Status counts:

```text
running    2,503
queued     2,500
succeeded  2,500
failed         3
```

Queue IDs:

```text
chat:deepseek:deepseek-v4-flash                       4,506
embedding:openrouter:perplexity/pplx-embed-v1-4b      1,500
rerank:openrouter:cohere/rerank-4-fast                1,500
```

Implication: provider traces deserve a `ProviderTracePayload` containing provider/model/operator, operation, request units, input/output tokens, cache/cost/timing/error fields, queue identifiers, and links to question/run/source ids when available.

### Current endpoint flow evidence

Routes in `src/bin/membench-server.rs`:

```text
/run/traces     -> run_traces       line ~1810
/run/traces.pb  -> run_traces_pb    line ~3368
```

Current JSON flow in `run_traces`:

```text
read artifacts/memory-traces.jsonl, capped by TRACE_ROW_CAP=4000
summarize_memory_stage_timing(memory_rows)
cost::rollup_model_traces(root)
read provider-queue/model-queue-traces.jsonl
summarize_queue_timing(queue_events)
summarize_trace_waterfall(memory_rows, queue_events)
summarize_dependency_waterfall(memory_rows)
summarize_trace_events(memory_rows, queue_events, queue_timing_rows)
read_workflow_queue(root)
return a mixed payload
```

Current Protobuf flow does a parallel derivation path and then converts the synthetic JSON value to protobuf.

Important current-session drift: Protobuf schema has been changed to make `MemoryTraces` stats-only, while JSON `/run/traces` still includes raw `memory_traces.rows`. This was a tactical performance change and should not be treated as the final design. The design sprint should decide the intended semantics and then make JSON/PB match or deprecate JSON.

### Current UI consumer evidence

`dashboard/src/sections/Traces.svelte` consumes one big `api.traces(id)` payload and renders:

```text
dependency_waterfall     -> Dependency Waterfall panel
memory_stage_timing      -> Memory Work Timing table
trace_events.rows        -> Unified Trace Log via TraceLog.svelte
bottlenecks              -> derived client-side from memory_stage_timing + queue_timing
trace_waterfall          -> Trace Waterfall panel
queue_timing             -> Provider queue summary/grouping
workflow_queue           -> Workflow Queue panel from SQLite projection
memory_traces.total      -> only presence/stats
```

`dashboard/src/components/TraceLog.svelte` filters/searches `TraceEventRow[]` by:

```text
timestamp, kind, operation, lane, event, status, source, error
```

Implication: current UI raw trace log is a normalized projection, not the full raw trace stream. That may be correct for display, but raw traces need separate type-aware drill-down endpoints.

### Current code hotspots / responsibilities

Source files with relevant responsibilities:

```text
src/bin/membench-server.rs
  /run/traces JSON/PB endpoints and all current request-time projections.

src/live.rs
  live detail reads provider queue traces and memory traces with multiple fallback paths.

src/artifacts.rs
  artifact kind -> file mapping for artifacts/ directory only.

src/step_analytics.rs
  derives persisted step_analytics from artifacts/memory-traces.jsonl and artifacts/model-traces.jsonl.

src/cost.rs
  rolls up cost/model usage from artifacts/model-traces.jsonl, provider-queue/model-queue-traces.jsonl, raw/model-traces.jsonl, model-traces.jsonl.

src/lib.rs
  BenchQueueEvent, memory trace JSONL read/write helpers, queue timing summary.

src/symbiotic_memory_adapter.rs
  records native memory adapter stages; writes question-debug bundles and score artifacts.

proto/membench/dashboard/v1/debugger.proto
  current dashboard protobuf DTOs.
```

## Corrected design framing

The primary data model should not be “one trace row for everything”. Better framing:

### 1. Raw event/trace streams

Raw trace rows need a common envelope and typed payloads:

```proto
message TraceRecord {
  string schema = 1;            // e.g. membench.trace.v1
  string run_id = 2;
  string event_id = 3;
  optional string parent_event_id = 4;
  string timestamp = 5;
  TraceType type = 6;           // MEMORY / PROVIDER / RUNNER / RECALL / QUESTION / etc.
  optional string question_id = 7;
  optional string source_id = 8;
  optional string operation = 9;
  optional string event = 10;   // started/succeeded/failed/queued/running/...
  optional string status = 11;
  repeated Link links = 12;     // provider request id, memory trace ids, artifact ids, etc.
  oneof payload {
    MemoryTracePayload memory = 20;
    ProviderTracePayload provider = 21;
    RunnerTracePayload runner = 22;
    RecallTracePayload recall = 23;
    QuestionTracePayload question = 24;
  }
}
```

Common denominator is envelope-level, not flattening every type into one row.

### 2. Type-specific raw trace payloads

Examples:

```text
ProviderTracePayload:
  provider/operator/model/operation
  queue_id/item_id/request_id/attempt/status
  token usage/cache usage/request units
  cost micro USD/pricing basis
  queue wait/throttle wait/run/total timing
  request/response hashes, error, retry metadata

MemoryTracePayload:
  memory system/store/index/backend
  stage/operation/instrumentation
  source/question identifiers
  retrieval query counts/query hashes/lane/top_k
  result counts/best score/support/context chars
  store/open/zvec/cache/manifest/index timings
  model_trace_ids/queue_item_ids links

RunnerTracePayload:
  workflow id/task id/question id
  queue state, claim/start/complete/retry/timeout/cancel events
  worker id, attempt, concurrency slot, scheduling delay
  artifact generation milestones and errors

RecallTracePayload:
  query plan, semantic/lexical queries, raw turn/fact candidates
  rerank features, candidate ids, scores, thresholds
  final evidence/context assembly
```

### 3. Constructed artifacts / materialized views

These are not raw traces:

```text
waterfalls
distribution heatmaps
step analytics
cost rollups
queue timing summaries
memory stage timing
bottleneck overviews
question response traces
retrieval coverage/gold coverage graphs
comparison deltas
```

They should be explicit artifacts with schemas and source references:

```proto
message ArtifactHeader {
  string artifact_id = 1;
  string schema = 2;
  string run_id = 3;
  string generated_at = 4;
  repeated SourceRef sources = 5;  // raw trace streams + hashes/ranges
  bool complete = 6;
}
```

Some should be constructed while the bench runs (cheap counters, online histograms, queue summaries). Others after run completion (waterfalls, graphs, coverage summaries, comparison views).

### 4. Question-centric model

Questions should be a first-class join axis, not incidental in rows.

Need stable question endpoint/data family:

```text
/api/run/questions.pb?id=...
/api/run/question.pb?id=...&question_id=...
/api/run/question/traces.pb?id=...&question_id=...&type=memory|provider|runner|recall
/api/run/question/artifacts.pb?id=...&question_id=...
```

A question record should link:

```text
question id
question text/type/source session ids
gold answer / gold evidence ids
hypothesis / answer / model output
judge verdict / prompts / raw judge output
provenance / router picks / memory config
question-debug bundle
evidence/candidate ids
raw trace ids / provider request ids / runner task ids
```

## API direction to evaluate after subagent reports

Potential target API families:

```text
/run/{id}/trace-streams.pb
  inventory available raw trace streams and schemas

/run/{id}/traces/events.pb?type=provider|memory|runner|recall&q=&question_id=&offset=&limit=
  typed raw trace records, separately fetchable, pageable/searchable

/run/{id}/trace-views/summary.pb
  summary counters, source stats, top-level health

/run/{id}/trace-views/waterfall.pb?scope=run|question&question_id=
  materialized or cached waterfall view, explicitly not raw trace

/run/{id}/trace-views/distributions.pb
  histograms/heatmaps, materialized data artifact

/run/{id}/questions.pb
  question index/list rows

/run/{id}/questions/{question_id}.pb
  full question-centric record with links

/run/{id}/questions/{question_id}/debug.pb
  question response trace/debug bundle

/run/{id}/artifacts.pb
  artifact inventory with ids/schemas/source refs
```

## Open design questions

1. Canonical storage paths:
   - Keep `artifacts/memory-traces.jsonl` / `artifacts/model-traces.jsonl`?
   - Move to `traces/memory.jsonl`, `traces/provider.jsonl`, `traces/runner.jsonl`?
   - Or keep artifacts as the publishing layer and traces/ as live-run layer?

2. Raw stream representation:
   - JSONL with schema version per row?
   - Protobuf frames?
   - SQLite/event table for searchable trace data?
   - Hybrid: JSONL append-only raw, SQLite/materialized index for dashboard?

3. Payload extensibility:
   - Protobuf `oneof` typed payloads plus `Struct`/map for extension attrs?
   - Or separate message definitions per trace stream?
   - How to avoid inefficient map-of-oneof for hot paths while preserving arbitrary attrs?

4. When to materialize derived artifacts:
   - During run for online counters/histograms?
   - After run for expensive graph/waterfall/coverage artifacts?
   - Lazy with source hash cache?

5. Question-centric joins:
   - What is the canonical question id?
   - How do provider request ids map back to question ids for all operations?
   - How do memory trace ids/provenance/debug bundles link to raw candidates/evidence?

6. Backcompat policy:
   - How much old-run compatibility matters?
   - User has said non-compliant old traces can be dropped. Need define “non-compliant” and validation output.

## Merged multi-agent conclusions

### 1. There are two canonicalities today

The status-quo sweep found two overlapping models:

```text
public/completed canonical bundle: artifacts/*
native/live execution originals: raw/*, traces/*, provider-queue/*, workflow/*, vaults/*
```

The duplicated files are not hypothetical:

```text
artifacts/memory-traces.jsonl            == traces/memory-events.jsonl
artifacts/model-traces.jsonl             == provider-queue/model-queue-traces.jsonl
artifacts/hypotheses.jsonl               == raw/hypotheses.jsonl
artifacts/verdicts.jsonl                 == raw/verdicts.jsonl
artifacts/partial-verdicts.jsonl         == raw/partial-verdicts.jsonl
artifacts/provenance.jsonl               == raw/provenance.jsonl
artifacts/scored.json                    == raw/scored.json
artifacts/score-summary.json             == raw/score-summary.json
```

`artifacts/` is the publication/import/completed-run layer. Native paths are the live execution layer. The design should make this explicit instead of letting every reader maintain its own fallback chain.

### 2. `artifacts/model-traces.jsonl` is currently provider queue lifecycle data

Despite the name, real `artifacts/model-traces.jsonl` rows are provider queue events:

```text
queue_id, item_id, operation, status, attempt, timestamp,
request_units, input_units, input_hash, queue_wait_ms, throttle_wait_ms,
run_ms, usage, error
```

This is a valid raw provider trace stream, but the name `model-traces` hides the fact that it includes queued/running/succeeded/failed lifecycle events and not just terminal model calls. Cost/live code already compensates by filtering terminal events in places.

### 3. Question debug is duplicated and path-addressed

There are 500 current debug files and 500 snapshot debug files:

```text
vaults/{qid}/debug/question-debug.json
vaults/{qid}/debug/hypotheses/hypotheses/question-debug.json
```

Sampled snapshots are byte-identical to current debug files. The dashboard logically selects by `question_id`, but the API fetches by run-relative `debug_artifact` path. That is an implementation detail leaking into the product model.

### 4. Workflow/runner state is real but not a first-class trace stream

The run has durable SQLite workflow state:

```text
workflow/longmemeval/queue.sqlite
queue_items count: 500
queue_events count: 1502
```

But current unified trace events only contain memory/provider projections. Runner/workflow is exposed as a separate `workflow_queue` dashboard projection, not as a typed raw/event stream.

### 5. Dashboard consumers are multiple data products, not one trace endpoint

The dashboard/API sweep classified the current UI as these families:

```text
Registry/run metadata:
  health, runs, pending, leaderboard, run detail

Question-centric artifacts:
  questions table, question drawer, question-debug bundle, answerer/retrieval context

Gold/scored/verdict data:
  verdicts, scored, score summary, gold-eval, compare deltas

Raw trace streams:
  memory traces, provider/model queue traces, runner/workflow events

Derived trace views:
  memory_stage_timing, queue_timing, trace_events projection,
  trace_waterfall, dependency_waterfall, bottleneck overview

Live/runner data:
  pending, live progress, workflow queue SQLite projection, runner schema/plan

Charts/distributions/heatmaps:
  leaderboard matrix/radar, gold rank heatmaps, bottleneck bars,
  timing percentiles, waterfall views
```

Therefore `/api/run/traces` is currently an overloaded bundle: raw-ish stats + derived trace views + workflow SQLite projection + model/cost rollup. That is useful for fast iteration but wrong as the durable abstraction.

### 6. Use OpenTelemetry concepts as a map, not as a full dependency

Best-practice sweep recommendation:

```text
Resource  -> run/system/config/instrumentation identity
Trace     -> question attempt or run-level setup/finalize causal unit
Span      -> work with duration: capture, query_plan, provider call, judge, artifact build
Event     -> append-only lifecycle/fact record: queued, started, progress, completed, failed
Metric    -> derived counters/gauges/histograms, not source-of-truth
Log       -> human/debug text correlated with trace/span, not analytic truth
Link      -> async/fanout/fanin relations; crucial for queues and memory recall
```

Do not blindly implement OTel or force everything into a tree. Multi-agent/memory systems need links/DAGs.

## Proposed target architecture

### Layer A — raw append-only facts

Raw data should be append-only and replayable. It should use a common envelope plus typed payloads, not one flat row and not opaque `timestamp/type/struct` only.

```proto
message TraceRecord {
  string schema = 1;              // membench.trace_record.v1
  string event_id = 2;
  string run_id = 3;
  string timestamp = 4;
  optional string observed_timestamp = 5;

  optional string trace_id = 6;    // causal trace, often question attempt
  optional string span_id = 7;     // duration/work unit
  optional string parent_span_id = 8;
  repeated TraceLink links = 9;    // async/queue/memory/artifact relations

  TraceType type = 10;             // MEMORY / PROVIDER / RUNNER / RECALL / QUESTION / JUDGE / ARTIFACT / SYSTEM
  optional string question_id = 11;
  optional string source_id = 12;
  optional string operation = 13;
  optional string phase = 14;      // queued/started/progress/completed/failed
  optional string status = 15;     // ok/error/cancelled/retrying/running
  uint32 attempt = 16;
  optional string component = 17;
  optional string role_binding = 18;

  repeated ArtifactRef artifact_refs = 19;
  optional string input_hash = 20;
  optional string output_hash = 21;
  optional string error_class = 22;
  optional string error = 23;

  oneof payload {
    MemoryTracePayload memory = 40;
    ProviderTracePayload provider = 41;
    RunnerTracePayload runner = 42;
    RecallTracePayload recall = 43;
    QuestionTracePayload question = 44;
    JudgeTracePayload judge = 45;
    ArtifactTracePayload artifact = 46;
  }

  // Escape hatch for experimental/vendor fields. Query-critical fields must not live here.
  google.protobuf.Struct extensions = 100;
}
```

This preserves the user’s “common denominator fields” idea while keeping type-specific structs first-class.

### Layer B — typed payload families

Provider payload:

```text
provider/operator/model/operation
queue_id/item_id/provider_call_id/request_id
request_units/input_units/output_units
input_tokens/output_tokens/cached_input_tokens/cache_miss_input_tokens
cost_micro_usd/pricing_source/estimated_vs_reported
queue_wait_ms/throttle_wait_ms/run_ms/total_ms
finish_reason/rate_limit/retry/error
```

Memory payload:

```text
memory_system/store/index/backend
stage/operation/instrumentation
source/question ids
retrieval query counts/query hashes/lane/top_k
result counts/best score/support/context chars
store/open/zvec/cache/manifest/index timings
model_trace_ids/queue_item_ids links
```

Recall/retrieval payload:

```text
query_plan id/hash
semantic/lexical/raw-turn query counts
retrieval profile/route
candidate ids/scores/ranks
rerank candidate ids/scores/ranks
selected evidence ids
gold evidence coverage hooks
support-check decision
```

Runner/workflow payload:

```text
workflow_id/task_id/question_id
queue item lifecycle: enqueued/claimed/started/completed/failed/retried/timed_out/cancelled
worker_id/lease_owner/concurrency slot
attempt/max_attempts/run_after/lease_until
artifact generation lifecycle
```

Question/judge payloads:

```text
question_type/source sessions/gold answer refs
answer/hypothesis hashes or artifact refs
judge model/prompt mode/verdict/abstention/rubric version
question-debug bundle refs
```

### Layer C — materialized data artifacts

The following are not raw traces and should be explicit materialized artifacts/views with schema, source refs, builder version, source hashes, and generation time:

```text
trace waterfalls
dependency waterfalls
memory stage timing distributions
provider queue timing distributions
cost/model rollups
bottleneck summaries
gold coverage summaries and rank heatmaps
leaderboard/cohort matrices
comparison deltas
question browser joined rows
question response traces/debug views
search indexes
```

Each materialized artifact should carry:

```proto
message ArtifactHeader {
  string artifact_id = 1;
  string schema = 2;
  string run_id = 3;
  string generated_at = 4;
  string builder = 5;
  repeated SourceRef sources = 6;  // file path/artifact id/hash/range/schema
  bool complete = 7;
}
```

### Layer D — question-centric join model

`question_id` is the dominant join key. Build a first-class question run record instead of requiring consumers to open raw debug files.

```text
QuestionRunRecord:
  run_id
  question_id
  question_type/category
  question text/source refs
  gold answer/gold evidence refs
  hypothesis/answer/model output refs
  verdict/judge details
  abstention/error
  router pick/final pick/profile
  provider cost/tokens/latency summary for that question
  memory/recall/evidence summary
  trace refs: memory/provider/runner/recall event ids or spans
  artifact refs: question-debug, raw prompts/responses, retrieved candidates
```

This should power the question list, drawer, per-question trace drilldown, gold coverage, and compare views.

## Proposed API partition

Keep data classes separate:

```text
Registry/run metadata:
  GET /api/runs(.pb)
  GET /api/run(.pb)?id=...
  GET /api/leaderboard(.pb)
  GET /api/compare(.pb)?base=...&cand=...

Question-centric:
  GET /api/run/questions(.pb)?id=...&q=&label=&type=&offset=&limit=
  GET /api/run/question(.pb)?id=...&question_id=...
  GET /api/run/question/debug(.pb)?id=...&question_id=...
  GET /api/run/question/traces(.pb)?id=...&question_id=...&type=memory|provider|runner|recall

Raw typed observability:
  GET /api/run/events(.pb)?id=...&type=memory|provider|runner|recall|judge|artifact&q=&question_id=&cursor=&limit=
  GET /api/run/spans(.pb)?id=...&type=&question_id=&cursor=&limit=
  GET /api/run/trace-streams(.pb)?id=...   # inventory schemas/counts/source refs

Materialized analytics/views:
  GET /api/run/trace-views/summary(.pb)?id=...
  GET /api/run/trace-views/waterfall(.pb)?id=...&scope=run|question&question_id=
  GET /api/run/analytics/stages(.pb)?id=...
  GET /api/run/analytics/cost(.pb)?id=...
  GET /api/run/analytics/retrieval(.pb)?id=...
  GET /api/run/analytics/gold-coverage(.pb)?id=...

Artifacts:
  GET /api/run/artifacts(.pb)?id=...
  GET /api/run/artifact(.pb)?id=...&artifact_id=...&cursor=&limit=&mode=raw|redacted|summary

Live:
  GET /api/run/live(.pb)?id=...
  GET /api/run/live/stream?id=...       # SSE eventually

Runner/config:
  GET /api/runner/schema(.pb)
  POST /api/runner/plan(.pb)
```

The existing endpoints can remain as compatibility wrappers while the new typed families land.

## Materialization lifecycle

### During run

Compute cheap live projections:

```text
run status
question pending/running/done/failed/scored
open spans
current queue pressure
active provider calls
recent events/errors
rough per-stage counts/durations
token/cost running totals when directly available
artifact presence/heartbeat
```

These support `/live` and should not be treated as final analytics.

### After run

Compute reproducible artifacts:

```text
question_run_summary.v1
trace_summary.v1
trace_waterfall.v1
dependency_waterfall.v1
provider_queue_timing.v1
memory_stage_timing.v1
cost_rollup.v1
gold_coverage.v1
search_index.v1
artifact_manifest.v2 with hashes/sizes/line counts/source refs
```

These should be invalidated/rebuilt when source hashes change. Existing `step-analytics.json` is a partial precedent but needs clearer schema/use boundaries.

## Compatibility and cleanup policy

1. **Do not delete old-run support blindly in code paths used for registry/import.** Instead centralize resolution.
2. Add a `TraceSources` resolver:

```rust
struct TraceSources {
    memory: Option<PathBuf>,
    provider: Option<PathBuf>,
    runner: Option<PathBuf>,
    artifacts_bundle: Option<PathBuf>,
}
```

3. Give each source a role:

```text
native_source     traces/*, provider-queue/*, workflow/*
published_copy    artifacts/*
legacy_raw        raw/*
```

4. For new runs, write one canonical native stream and one published artifact if needed. Avoid byte-identical duplicates unless publication/export requires them.
5. For dashboard event normalization, drop non-compliant rows from typed event APIs, but count/report them:

```text
dropped_rows
drop_reasons
source_path
schema_version
```

6. Keep raw artifact access for forensic debugging.

## Implementation roadmap

### Phase 0 — freeze the design target before more schema churn

Deliverables:

```text
ADR: tracing/data artifact taxonomy
protobuf sketch for TraceRecord + payloads
API namespace sketch
materialization lifecycle
compatibility policy
```

No runtime code changes except docs/tests/fixtures.

### Phase 1 — source resolver and inventory endpoint

Files likely involved:

```text
src/trace_sources.rs              # new central resolver
src/artifacts.rs
src/live.rs
src/cost.rs
src/step_analytics.rs
src/bin/membench-server.rs
proto/membench/dashboard/v1/debugger.proto
```

Add tests with fixture run directories covering:

```text
native traces/* + provider-queue/*
artifacts-only imported run
legacy raw/* paths
missing/non-compliant rows
```

### Phase 2 — question-centric records

Build `QuestionRunRecord` materialization from:

```text
hypotheses.jsonl
verdicts/partial-verdicts.jsonl
provenance.jsonl
question-debug.json
gold-eval.json
trace links
provider/memory summaries
```

Add endpoints by `question_id`, not `debug_artifact` path.

### Phase 3 — raw typed events API

Normalize existing memory/provider/workflow rows into `TraceRecord` envelopes with typed payloads. Add:

```text
/api/run/events.pb
/api/run/spans.pb
/api/run/trace-streams.pb
```

Support type filters, question filters, cursor pagination, and search over envelope/common fields first.

### Phase 4 — materialized analytics/views

Move request-time derivations behind materialized/cacheable artifacts:

```text
trace waterfall
dependency waterfall
memory stage timing
queue timing
cost rollup
gold coverage/search indexes
```

Dashboard consumes these as views, not as raw traces.

### Phase 5 — dashboard refactor

Refactor dashboard panels to consume the correct data products:

```text
Trace Log -> /events or /spans
Waterfalls -> /trace-views/waterfall
Question drawer -> /question by id
Gold coverage -> gold coverage artifact/view
Bottlenecks/charts -> analytics/views
Raw artifacts -> artifact browser only
```

## Immediate decision points

Before implementing, decide:

1. Canonical native trace paths for new runs:
   - `traces/memory.jsonl`, `traces/provider.jsonl`, `traces/runner.jsonl`?
   - or keep existing names and only centralize resolver?

2. Raw storage format:
   - JSONL envelope rows now, Protobuf frames later?
   - SQLite/index sidecar for search?

3. Event model granularity:
   - lifecycle events only, or also materialized spans?
   - likely both: append-only events + derived spans.

4. Public artifact strategy:
   - keep publishing `artifacts/*` copies?
   - or make `artifact_manifest.v2` point to native streams?

5. Backcompat strictness:
   - user is willing to drop non-compliant old rows from typed dashboards;
   - still need registry/import behavior for artifact-only historical runs.

