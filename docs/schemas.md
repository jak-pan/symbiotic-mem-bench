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
`embedder`, `store`, `answer_output`, `generative_answerer_enabled`, `query_planner`,
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

## Native Trace Stage Labels

Native Symbiotic Memory traces normalize a few adapter and memory operations before rendering them
in Live and Traces:

| Rendered stage | Trace source | Meaning |
|---|---|---|
| `setup` | `operation=adapter_call`, `stage=pre_capture_setup` | Vault directory creation, source hashing, manifest read/write, embedder dimension lookup, zvec cache validation, store open, and existing-state load before `capture` starts. |
| `capture` | `operation=capture` | First memory-pipeline stage emitted by the native ingest pipeline. |
| `briefs` | `operation=embed_facts`, `metrics.kind=brief` normalized to `consolidate` | Source-backed extractive brief pass. |
| `recall setup` | `operation=adapter_call`, `stage=pre_recall_setup` | Post-ingest count loading and recall-index readiness before query planning, search, support, and answer stages. |
| `prompt plan` | `operation=query_plan` | Query planner trace emitted by the memory engine recall debug path. |

Setup stages are intentionally separate from ingest timing. If loading 50 vaults is slow before any
capture bar moves, inspect `setup` p80/p98 and its numeric metrics such as `store_open_ms`,
`zvec_cache_ms`, `load_existing_ms`, and `manifest_ms`.

Legacy `adapter_call` rows without one of the typed setup stage names are ignored by dashboard
summaries and unified trace logs. Raw trace artifacts keep those rows for archeology, but old runs
should not render a misleading generic adapter stage.

## Machine-readable Index

```bash
cargo run --bin membench -- explore --json
```

Emits the normalized run index (one `RunSummary` per run) as JSON — the same shape the dashboard
backend serves at `GET /api/runs`. Without `--run-root` it scans both `runs/` and `records/`.

## Trials

Trials are typed analysis artifacts for improving a memory system from observed failures. Use this
term instead of "fine-tuning": no model weights are trained, and no question-specific answers are
patched. A trial tests a generic prompt, retrieval, storage, or scoring change against a declared
failure stack and records both wins and regressions.

Trial stack size carries scope:

| Size | Classification | Use |
|---|---|---|
| `<25Q` | `focused_trial` | Tight failure-class forensics or prompt iteration. Valid, but intentionally narrow. |
| `25-50Q` | `diagnostic_trial` | Normal trial band for broader diagnostic decisions; should be stratified by failure bucket and question type. |
| `51-499Q` | `broad_diagnostic` | Confirmation sweep before full benchmark scale. |
| `500Q+` | `benchmark_scale` | Candidate benchmark claim only with complete artifacts, provenance, and no-cheating review. |

Trial analysis folders live under:

```text
runs/analysis/{stack_id}/
```

The typed files are:

| File | Schema | Meaning |
|---|---|---|
| `trial-stack.json` | `membench.trial_stack.v1` | The baseline failure stack, comparison runs, subset, terminology, and rules. |
| `trials.jsonl` | `membench.trial.v1` | One row per improvement run, including reasoning, changed files, tests, aggregate score, improvements, regressions, risks, and decision. |
| `trial-question-deltas.jsonl` | `membench.trial_question_delta.v1` | One row per question per trial run, comparing current result to the declared comparison run and original baseline stack. |
| `LEDGER.md` | Markdown render | Human-readable summary. The JSON/JSONL files are the structured source of truth. |

`trial-stack.json` common fields:

| Field | Meaning |
|---|---|
| `schema` | Schema id, currently `membench.trial_stack.v1`. |
| `stack_id` | Folder-safe id for the analyzed failure stack. |
| `terminology` | Preferred name and avoided names with rationale. |
| `system` / `benchmark` | System and benchmark under analysis. |
| `baseline_runs` | Full-run references used to define the original stack. |
| `failure_buckets` / `by_type` | Counts for baseline overlap and question-type distribution. |
| `sample_policy` | Question count, classification, recommended 25-50Q diagnostic range, and whether the stack is focused. |
| `tuning_subset` | Optional smaller question set used for diagnostic trials. |
| `rules` | Anti-cheating and comparability rules for this stack. |

`trials.jsonl` rows use:

| Field | Meaning |
|---|---|
| `schema` | Schema id, currently `membench.trial.v1`. |
| `stack_id` | Parent trial stack id. |
| `run_id` / `run_path` | Benchmark run tested by this trial. |
| `change_id` / `change_title` | Stable change identity. Multiple model settings may test the same change. |
| `answerer` | Model and reasoning/thinking setting when relevant. |
| `compared_to_run_id` | Immediate comparison run for wins/regressions. |
| `reasoning` | Why this change was made and what failure class it targets. |
| `changed_files` | Structured list of paths, areas, and summaries. |
| `verification` | Tests and commands run before or with the trial. |
| `sample_policy` | Sample-size classification for this trial row. |
| `aggregate` | Score metrics for the trial run. |
| `outcomes.improvements` | Questions fixed versus `compared_to_run_id`. |
| `outcomes.regressions` | Questions broken versus `compared_to_run_id`. |
| `outcomes.unchanged_wrong` / `unchanged_correct` | Stable failures/successes. |
| `risks` | Known ways the change could overgeneralize or mislead. |
| `decision` | What to do with the trial result. |

`trial-question-deltas.jsonl` rows use:

| Field | Meaning |
|---|---|
| `schema` | Schema id, currently `membench.trial_question_delta.v1`. |
| `run_id` / `change_id` | Trial run and change identity. |
| `question_id`, `question_type`, `question`, `gold_answer` | Question metadata for forensic inspection. |
| `comparison_run_id` / `comparison` | Immediate comparison answer and label. |
| `original_baseline_run_id` / `original_baseline` | Original full-stack baseline answer and label for this model path. |
| `current` | Trial answer, judge label, and raw judge output when available. |
| `outcome` | `improved_vs_comparison`, `regressed_vs_comparison`, `unchanged_correct_vs_comparison`, or `unchanged_wrong_vs_comparison`. |
| `original_outcome` | Fixed/regressed/still-correct/still-wrong relative to the original baseline stack. |
| `notes` | Optional human notes; keep empty instead of inventing explanations. |

Trial files are intended to be appendable. When adding a new run, append one
`membench.trial.v1` row and one `membench.trial_question_delta.v1` row for each question being
compared. Do not overwrite prior rows unless correcting malformed metadata; write corrections as
new rows when the answer content or scoring changed.

Generate or update these files from existing run artifacts with:

```bash
cargo run --bin membench -- trials derive \
  --trial-run-root runs/{system}/{benchmark}/{limit}/{candidate_run} \
  --comparison-run-root runs/{system}/{benchmark}/{limit}/{previous_run} \
  --original-baseline-run-root runs/{system}/{benchmark}/{limit}/{baseline_run} \
  --change-title "{short title}" \
  --reasoning "{why this generic change is being tested}" \
  --changed-file "../symbiotic-memory/src/recall/prompt_policy.rs:120|answer prompt|Clarify evidence grouping" \
  --verification "cargo test --manifest-path ../symbiotic-memory/Cargo.toml prompt_ --features cli" \
  --decision "diagnostic_only"
```

The command reads standard run artifacts (`scored.json`, verdicts, hypotheses, provenance, memory
traces, model traces, and question-debug bundles when present) and computes the per-question deltas.
`--stack-id` and `--change-id` are generated by default from the change title and compared run roots;
pass explicit ids only when intentionally grouping several trial rows into one ledger.
Question-debug content is referenced by path and hash; raw prompt bodies are not copied into the trial
ledger.

Dashboard registry rows are flagged as `TRIAL` when a `membench.trial.v1` row references that run via
`run_path`. The badge means "diagnostic improvement trial", not a promoted benchmark claim. The run
still keeps its ordinary `benchmark-report.json`; the trial context is an overlay from
`runs/analysis/{stack_id}/trials.jsonl`.

## Artifact Manifest

`artifact_manifest` answers two questions: what did this run capture, and what is missing?

```json
{
  "available": ["hypotheses", "scored", "verdicts"],
  "missing": ["provenance", "memory_traces", "model_traces", "step_analytics", "score_summary"],
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
| `step_analytics` | `artifacts/step-analytics.json` |
| `score_summary` | `artifacts/score-summary.json` |

For native Symbiotic Memory runs, `model_traces` may use the provider queue event schema copied from
`provider-queue/model-queue-traces.jsonl`. Readers must accept both the older nested
`model/usage/timing/outcome` trace shape and the queue-native
`queue_id/item_id/operation/status/attempt/usage` shape.

Native answer-only reruns may include `run_params.source_vault_root`. When present, the run's
`vaults/` tree is an isolated view over an existing ingested substrate: heavy immutable files such
as `memory.sqlite` and `archive/` may be filesystem links, while mutable files such as
`manifest.json`, `answer.json`, and `debug/` belong to the rerun.

Per-question debug bundles under
`vaults/{question_id}/debug/hypotheses/{run_id}/question-debug.json` may include raw prompts and
model responses for local diagnosis. For the query planner, inspect
`recall.query_planner_call.system_prompt`, `recall.query_planner_call.user_prompt`, and
`recall.query_planner_call.response_text`. For the search response, inspect
`recall.retrieval_queries`, `recall.query_plan`, `recall.initial_profile`, and optional
`recall.fallback_profile`; the profile entries carry scores plus fact/raw-turn evidence. Portable
memory traces should point to that bundle and record prompt/response hashes instead of duplicating
raw prompt text.

`step_analytics` is a derived JSON rollup over `memory_traces`, `model_traces`, and local
per-question debug bundles when present. It stores per-operation and per-question timing summaries,
numeric metric summaries (`p50`, `p80`, `p95`, `p98`, `max`), model queue/item timing summaries,
context/answer sizes, retrieval-query counts, and hashes of query plans/retrieval query lists. Use
numeric metric summaries for stage sub-timings such as provider embed time, local store upsert time,
and post-provider work. It must not duplicate raw prompts, raw retrieval query text, raw source
documents, or secrets. Backfill or refresh it for an existing run with:

```bash
cargo run --bin membench -- analytics --run-root runs/{system}/{benchmark}/{limit}/{run_name}
```

The live monitor derives model-queue diagnostics from provider queue events:

| Field | Meaning |
|---|---|
| `running` / `queued` | Current latest state per queue item inside the inspected window. |
| `observed_peak_running` | Highest simultaneously running item count observed in the inspected window. |
| `starts_last_minute` | Requests whose `running` event started in the last trace minute of the inspected window. |
| `peak_starts_per_minute` | Highest 60-second count of `running` events observed in the inspected window. |
| `avg_running` | Time-weighted average running item count over the inspected provider event span. |
| `avg_queued` | Time-weighted average queued item count over the inspected provider event span. |
| `avg_starts_per_minute` | Average request-start rate over the inspected provider event span. |
| `observed_duration_secs` | Seconds between first and last inspected event for this queue. |
| `last_event_at` | Latest provider event timestamp for the queue. |

Active runs may be summarized from a bounded tail. Completed native runs read the full provider trace
when it is under the live safety cap, otherwise they also fall back to a bounded tail. Full-run cost,
token, and latency summaries come from the artifact rollups and should not be inferred from the live
tail alone.

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
| `event` | Lifecycle event such as `operation_started`, `operation_succeeded`, `operation_failed`, `branch_started`, or `branch_joined`. |
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
| `consolidate` | source-backed extractive briefs written for smaller grounded recall context |
| `query_plan` | optional prompt-planning/query-planning result used to build retrieval probes; trace metrics include prompt/response hashes and point to the question debug bundle |
| `retrieve` | retrieval query and candidates produced |
| `answer` | final answer or explicit unavailable result produced |
| `score` | benchmark judgment produced |

Expected event progression is `operation_started` followed by `operation_succeeded` or
`operation_failed`, with retry attempts represented as new events using the same logical input hash
or queue item id. Branches such as `embed_raw` and `distill -> write_archive -> embed_facts` may
overlap and may emit `branch_started` / `branch_joined` instead of operation events. Tools must not
infer failure from missing later stages alone; use durable state and trace events.

Embedding batch progress is request-oriented. `embed_raw` and `embed_facts` success events may carry
batch item counts and total item counts; these represent persisted embedding batches, not source-row
completion. Batch sizing defaults are code-owned. The separate per-input local cap is
`MEMBENCH_EMBED_MAX_CHARS`. Dashboards should display both concepts without implying that the batch text
budget is the per-item model window.

## Queue Events

Provider/model queue event JSONL can be summarized with:

```bash
cargo run --bin membench -- summarize-queue-events \
  --jsonl runs/{system}/{benchmark}/{limit}/{run_name}/provider-queue/model-queue-traces.jsonl
```

Required queue fields are `queue_id`, `item_id`, `operation` or `kind`, `status`, `attempt`, and
`timestamp`. Optional fields include `model`, `input_hash`, `usage`, `cost_micro_usd`, and `error`.
If `cost_micro_usd` is missing but `usage` has token buckets, the cost rollup may estimate cost from
the built-in pricing catalog. Estimated rollups set `cost_estimated: true`,
`pricing_table_version`, and `pricing_sources`. If token usage is missing, the model remains
unpriced instead of being treated as zero-cost.

Native Symbiotic Memory runs may also have a workflow queue at
`workflow/longmemeval/queue.sqlite`. This queue records durable row-level work such as pending,
running, retry, and succeeded states. It does not imply that a model/provider call happened; model
calls require `model-traces.jsonl` or `provider-queue/model-queue-traces.jsonl`.
