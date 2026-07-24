# membench — Definitive Ground-Truth Status Quo

*A merged code-inventory of the membench benchmark tool: backend server, Svelte dashboard, on-disk run storage, the raw/derived data model, and the in-flight JSON→protobuf migration. Every claim is anchored to `file:line` or on-disk evidence.*

---

## 1. System at a Glance

membench is a LongMemEval benchmark harness plus a debugger dashboard. Three tiers:

**Backend — `membench-server` (Rust / axum).** All routes are registered in one block at `src/bin/membench-server.rs:266-294`, nested under `/api` (`:297`), with permissive CORS (`:298`). Beyond `/api/*`, a static `ServeDir` fallback serves the built Svelte SPA from `dashboard/dist` (`:301-303`). The server reads per-run files off disk, most derivations happening at request time; a small set is cached (registry snapshot, live-detail).

**Dashboard — single-page Svelte 5 (runes) app.** An amber-on-black "memory system terminal" with **no library router** — routing is a hand-rolled hash-based rune singleton (`dashboard/src/lib/router.svelte.ts`). Transport is **protobuf-first with JSON fallback** (`dashboard/src/lib/api.ts:365-457`): every endpoint tries its `*.pb` twin with `accept: application/x-protobuf`, then falls back to the JSON route on failure. The app internally consumes only the hand-written snake_case DTOs in `types.ts` regardless of wire format.

**Storage — a 4-level run registry on disk.** Runs live under `runs/{system}/{benchmark}/{limit}/{run_name}/` (`records/README.md:7-13`), i.e. `runs/symbiotic-memory/long-mem-eval/{N}/{run_name}/`. A git-tracked, artifact-only sibling registry lives at `records/{system}/{benchmark}/{limit}/{name}/`. Each run root has three storage layers: a **native live-execution layer** (`traces/`, `provider-queue/`, `workflow/`, `vaults/`), a **published-artifacts layer** (`artifacts/`), and a **legacy raw layer** (`raw/`) — plus top-level `run-params.json`, `benchmark-report.json`, and a 7-byte `.store-zvec` marker. The canonical native-state dir set is code-defined at `src/registry.rs:127-132` (`PRUNE_DIRS = ["vaults","workflow","provider-queue","raw","artifacts"]`).

Two producer origins write the streams: **this bench crate** writes the answer/verdict/provenance/hypothesis/gold-eval/score/step-analytics/question-debug data; **external libraries** (`symbiotic-memory` engine + `symbiotic-foundation` provider queue) write the two big trace streams (`memory-events.jsonl`, `model-queue-traces.jsonl`), which the bench crate only reads back and copies into `artifacts/`.

---

## 2. Screens & Information Architecture

### Top-level shell — `dashboard/src/App.svelte`
Fixed 3-row layout: `topbar` (:83) / `stage` (:116) / `statusbar` (:124). Two top-level modes toggled via nav and function keys (:90-97, :54-59):
- **F1 → Leaderboard** (`router.view === "leaderboard"`, default) → `<Leaderboard/>` (:117-121)
- **F2 → Debugger** (`router.view === "debug"`) → `<Debugger/>`

A `/`-focused **command palette** (:99-111, :51-53) with `runCommand()` (:65-78) accepts `lb`/`leaderboard`, `dbg`/`debug`, or a fuzzy run-name/run-id match → `router.openRun(hit.run_id)`. The **status bar** (:124-153) shows LIVE/OFFLINE, RUNS/SYSTEMS/BENCHMARKS counts, PEAK ACC, an "N IN FLIGHT" deep-link to the first active run's Live screen (:138), errors, and a server-SHA / UI-bundle stamp. Polling: `store.load()` on mount + every 15s, gated on `document.visibilityState === "visible"`, re-pulled on focus (:28-34).

### Routing — `dashboard/src/lib/router.svelte.ts`
Hash routes (:1-5, parsed :26-58): `#/` → leaderboard; `#/debug` → debugger (subscreen defaults to `overview`, :33); `#/debug/<encoded-run>/<subscreen>` → focused run + subscreen; also the reversed `#/debug/<subscreen>/<encoded-run>` for hand-edited links (:48-54). `subscreen` is validated against `DEBUG_SUBSCREENS` (:7-24): `overview | questions | compare | traces | gold-coverage | live | tuner`. `router.go`/`openRun`/`openRunSubscreen` (:75-89) mutate `window.location.hash`; a `hashchange` listener re-parses (:62-64). State is a `$state`-backed singleton — **no history stack, no query strings**.

### Debugger sub-navigation — `dashboard/src/routes/Debugger.svelte`
Two-column grid: 248px `RegistryTree` rail + work area (:39-40, :94-102). Tabs are dynamic (:20-30): base `OVERVIEW · QUESTIONS · COMPARE · TRACES · GOLD COVERAGE`, then `LIVE` **only if** `selected.native_state_available`, then always `TUNER`. **Pending-run override** (:32-36, :58-72): if the run is in-flight (`store.isPending`), tabs collapse to a single non-clickable `▶ LIVE MONITOR` and the body force-renders `<Live>`. `activeTab` falls back to `overview` when the requested subscreen isn't in `tabs` (:34-36).

### The nine screens

| Screen | File | Purpose | API calls | Notable gaps |
|---|---|---|---|---|
| **Leaderboard** | `routes/Leaderboard.svelte` | Cross-run ranking within a cohort (benchmark × question-set × judge) + comparison workspace | `api.leaderboard()` once on mount (:36); all else client-side | Fixed-layout table (`min-width:980px`, :767-773) horizontally scrolls/truncates; no text search/filter; no pagination; comparison capped at 4 with no "5th blocked" cue; latency column silently disappears if no p50 (:42, :68-70) |
| **RegistryTree (rail)** | `components/RegistryTree.svelte` | Run picker + in-flight monitor | reads `store` directly (no fetch) | No text search across the registry (only toggles + sort); flight body capped `max-height:200px` |
| **Overview** | `sections/Overview.svelte` | Single-run scorecard + config provenance | `api.run(id)` (:15) with id race-guard | Run Parameters is a raw `JSON.stringify` dump, `white-space:nowrap` clips long values; artifact list is a hard-coded `ALL_KINDS` set (:28), not server-manifest-driven |
| **Questions** | `sections/Questions.svelte` | Per-question verdict browser + deep recall/answer debug drawer (richest screen) | `api.questions(id)` (:52); `api.questionDebug(id,path)` lazily per opened row (:61-76), cached per path | Render cap at 200 with "SHOW 200 MORE" (:94, :248-253); drawer is a full-viewport overlay of stacked `<pre>` blocks all `open` by default, no collapse-all; filtering is client-side only; drawer state not URL-addressable |
| **Compare** | `sections/Compare.svelte` | A/B two runs (candidate = focused, baseline = picked) | `api.compare(baseId,id)` on baseline change (:25-36) | Single-baseline only (multi-way lives on the Leaderboard radar); changed table clips questions to 280px; no filter on the flip list |
| **Traces** | `sections/Traces.svelte` | Performance/throughput forensics for a completed run | `api.traces(id)` (:17) | Percentiles recomputed client-side from raw `queue_timing` (:34-67); waterfalls capped `max-height:360px`; Provider Queue Summary hidden by default so its data is easy to miss; `NO NATIVE TRACES` empty state for artifact-only runs (:173-175) |
| **Gold Coverage** | `sections/GoldCoverage.svelte` | Gold-evidence coverage + embed-vs-rerank recall diagnostics | `api.goldEval(id)` (:22-33) **plus** `api.runs()` + `api.goldEval()` for **three hard-coded reference runs** `c500-coh-1 / nemo-rpmfix-500 / pplx-rpmfix-500` (`COMPARE_RUN_NAMES` :60, :100-158) | Reference-run names hard-coded (:60, aliases :64-68) — not configurable, silently shows "not in registry" if absent; all percentile/bin math client-side (:190-217, :247-255); `NO GOLD-EVAL ARTIFACT` empty state shows the exact CLI to generate it (:302-307) |
| **Live** | `sections/Live.svelte` | Real-time monitor for a running/just-completed run | `api.live(id)` polled **every 2s**, visibility-gated (:28-46) | Only reachable when `native_state_available` (or pending); poll errors surface only as a small "poll err" tag; feeds are unbounded-height scroll |
| **Tuner** | `sections/Tuner.svelte` | Build a `membench` run command from a form (preview-only) | `api.runnerSchema()` + `api.run(id)` prefill (:14-25); `api.runnerPlan(values)` debounced 220ms (:30-42) | Explicit stub — "Live spawn + log streaming is the next milestone" (:143); Runs/Status panel + "SPAWN RUN" button non-functional (:143, :149-155); env-default secrets shown as `key=value` text |

**Modals/drawers**: the Questions drawer (`Questions.svelte:257-429`) is a fixed `role="dialog" aria-modal` overlay, Esc-to-close (:176), focus-trapped (:97-99); Leaderboard has two collapsible `<section class="fold">` accordions auto-toggled by selection count (`Leaderboard.svelte:72-79, :202-326`).

### Reusable components (`dashboard/src/components/`)
`Panel` (universal bordered container), `RingGauge` (270° arc gauge), `Radar` (spider chart, `min=0.5` floor to amplify accuracy deltas), `CategoryHeat` (6-qtype heat row), `Bar`, `DeltaBars` (diverging), `RegistryTree` (whole left rail, self-contained state), `TraceWaterfall` (one component, two `variant` shapes), `TraceLog` (event table, caps render at 1200 rows), `QueueSummary` (percentile table). Shared libs: `format.ts` (display helpers, `heatColor`, `shortQueue` mirroring the Rust backend), `run.ts` (`trialBadge`, `runKindLabel`, `runKindChipClass`), `async.svelte.ts` (`createAsyncData<T>()`), `store.svelte.ts` (`Store` singleton).

### IA gaps
- **Every section runs its own `$effect` fetch with an `if (runId !== id) return` race guard** (Overview:16, Questions:53, GoldCoverage:26) — a repeated, non-centralized pattern.
- **Truncation/density is the dominant UX gap**: horizontal-scroll fixed tables, 200/1200-row render caps, `text-overflow:ellipsis` clipping across nearly every table, internal `max-height` scroll on waterfalls/feeds.
- **Hard-coded config leaks into components**: gold-coverage reference runs (`GoldCoverage.svelte:60`), artifact-kind list (`Overview.svelte:28`).
- **No global search/deep-link into individual questions** — the palette resolves runs only; the Questions drawer state is not URL-addressable.

---

## 3. HTTP API Surface

`pb` = `symbiotic_mem_bench::dashboard_proto::membench::dashboard::v1` (`membench-server.rs:28`), whose messages come from `proto/membench/dashboard/v1/debugger.proto` via `build.rs`+`OUT_DIR` (`src/dashboard_proto.rs:1-8`). Every JSON endpoint has a `.pb` twin that runs identical run-dir reads and produces an equivalent protobuf message. **All 14 `.pb` routes are uncommitted; the 13 JSON routes pre-exist in `HEAD`** (see §7).

| # | Path | Method | Handler | Files read | Compute vs pre-materialized | Response / message |
|---|------|--------|---------|------------|------------------------------|--------------------|
| 1/2 | `/health` · `.pb` | GET | `health` (:329) · `health_pb` (:339) | none | pure constants | inline `{ok,service,version,git_sha,binary_sha}` · `pb::HealthResponse` |
| 3/4 | `/runs` · `.pb` | GET | `list_runs` (:1398) · `list_runs_pb` (:1429) | walks all `benchmark-report.json` via cached `registry_snapshot` | **cached** snapshot (mtime-keyed, :132); request only filters | `{runs:[RunSummary]}` · `pb::RunsResponse` |
| 5/6 | `/pending` · `.pb` | GET | `pending_handler` (:1460) · `pending_pb` (:1467) | `registry::scan_pending` re-scans roots (:182-184) | **NOT cached** — re-scans every hit | `{pending:[PendingRun]}` · `pb::PendingResponse` |
| 7/8 | `/leaderboard` · `.pb` | GET | `leaderboard_handler` (:1549) · `leaderboard_pb` (:3151) | cached snapshot summaries | `leaderboard::build_cohorts` groups+ranks at request time (`src/leaderboard.rs:52`); `.pb` does Value→re-parse double round-trip (:3168-3172) | `{cohorts:[Cohort]}` · `pb::LeaderboardResponse` |
| 9/10 | `/run` · `.pb` | GET | `run_detail` (:1574) · `run_detail_pb` (:3176) | `benchmark-report.json`(cached), `run-params.json`(cached), `model-traces.jsonl` | **heavy, uncached**: `cost::rollup_model_traces` (`src/cost.rs:377`) + `compute_cohort_fields_with_rollup` re-derived per hit | `{summary,report,params,cohort,cost}` · `pb::RunDetailResponse` |
| 11/12 | `/run/live` · `.pb` | GET | `run_live` (:1480) · `run_live_pb` (:3216) | pending scan; `hypotheses.jsonl`; `vaults/` count; model/memory trace JSONL | `live::live_detail` (`src/live.rs:739`) → **cached 2.5s TTL, mtime-keyed** (:189); `.pb` serializes→`live_detail_pb` (:3269) | `{pending,detail:LiveDetail}` · `pb::LiveResponse` |
| 13/14 | `/run/questions` · `.pb` | GET | `run_questions` (:1612) · `run_questions_pb` (:1628) | per-run scored/verdict artifacts via `artifacts::question_rows` | request-time read+parse, **no cache** | `{total,questions:[QuestionRow]}` · `pb::QuestionsResponse` |
| 15/16 | `/run/question-debug` · `.pb` | GET | `run_question_debug` (:1652) · `..._pb` (:3279) | one `**/question-debug.json` (path-validated, :1674) | pre-materialized file passed through; **`.pb` re-stringifies JSON into a string field** (`json_text_value`, :3296) | `{path,json}` · `pb::QuestionDebugResponse` |
| 17/18 | `/run/artifact` · `.pb` | GET | `run_artifact` (:1737) · `..._pb` (:3304) | 1 of 10 whitelisted artifacts via `artifact_file` (:1721) | pre-materialized; JSONL paged lazily (`read_jsonl_values` :1777, offset/limit≤2000); **`.pb` re-stringifies rows/json** (:3323/:3336) | `{kind,total,offset,limit,rows}` or `{kind,json}` · `pb::ArtifactResponse` |
| 19 | `/run/gold-eval.pb` | GET | `run_gold_eval_pb` (:3347) | `artifacts/gold-eval.json` | pre-materialized → typed `gold_eval_pb` projection (:1214) | `pb::GoldEvalResponse` (**PB-only, no JSON twin**) |
| 20/21 | `/run/traces` · `.pb` | GET | `run_traces` (:1810) · `run_traces_pb` (:3368) | `memory-traces.jsonl` (cap 4000), `model-traces.jsonl`, `provider-queue/*`, `workflow/**/queue.sqlite` | **HEAVIEST, uncached**: `summarize_memory_stage_timing`, `rollup_model_traces`, `summarize_queue_timing`, `summarize_trace_waterfall` (:1991), `summarize_dependency_waterfall` (277-line, :2091-2367), `summarize_trace_events` (:1884, cap 12000), `read_workflow_queue` (opens SQLite); `.pb` adds full Value round-trip via `traces_pb_from_value` (:985) | inline `{memory_traces, memory_stage_timing, model_rollup, queue_timing, trace_waterfall, dependency_waterfall, trace_events, workflow_queue}` · `pb::TracesResponse` |
| 22/23 | `/compare` · `.pb` | GET | `compare_handler` (:3040) · `compare_pb` (:3405) | two run roots from cached snapshot; both runs' scored/verdict artifacts | `compare::compare_runs` (`src/compare.rs:82`) diffs at request time, uncached | `{base,candidate,result}` · `pb::CompareResponse` |
| 24/25 | `/runner/schema` · `.pb` | GET | `runner_schema` (:3080) · `..._pb` (:3445) | cached snapshot `records[].params` | `runner::symem_param_schema()` (mostly static) + scan of observed values | `{system,benchmark,fields:[ParamField]}` · `pb::RunnerSchema` |
| 26/27 | `/runner/plan` · `.pb` | POST | `runner_plan` (:3140) · `..._pb` (:3501) | none (reads `repo_root`) | `runner::plan_from_params` builds command preview from POSTed JSON — pure derivation | `RunnerPreview` (or null) · `pb::RunnerPreview` |

**Non-`/api` route:** `uiVersion` fetches `/version.json` (`cache:no-store`, no `/api` prefix, JSON-only) — `api.ts:370-374`.

---

## 4. Raw Trace / Event Streams

Reference run: `runs/symbiotic-memory/long-mem-eval/500/factconsol-thinkon-500-20260624/` (500 questions, completed).

### 4.1 memory-traces (native stage trace)
The memory engine's internal stage events, written by the `symbiotic-memory` engine via a `MemoryTraceSink` (imported `src/symbiotic_memory_adapter.rs:38`).
- **Canonical:** `traces/memory-events.jsonl`; mirrored byte-for-byte to `artifacts/memory-traces.jsonl`.
- **Row type:** `MemoryTraceEvent` (`src/lib.rs:236-262`); read/append helpers `src/lib.rs:301-334`.
- **24 top-level keys:** `schema_version, trace_id, parent_trace_id, source_system, instrumentation, run_id, question_id, source_id, operation, stage, event, attempt, timestamp, started_at, finished_at, duration_ms, input_hash, output_hash, item_count, model_trace_ids, queue_item_ids, metrics, error_class, error`.
- **63,897 rows** in the reference run (17,040 in `f500-gate-1`).
- **On-disk `operation`/`stage` values EXCEED the `MemoryTraceOperation` enum** (`src/lib.rs:207-225`): observed `consolidate, fact_search, raw_search, query_plan, embed_query, support_check, answer_context, answer` — none are enum variants. The engine serializes free-form strings; the Rust enum is not authoritative for what's on disk. `event` is a lifecycle kind (`operation_started/succeeded/failed`, `batch_*`, `branch_started/joined`); high-volume rows are `embed_facts/distill/embed_raw` `batch_succeeded`. `metrics` is an open `serde_json::Value` payload bag varying by operation.

### 4.2 model-queue-traces (provider queue lifecycle)
LLM/embedding queue events written by the `symbiotic-foundation` provider queue.
- **Canonical:** `provider-queue/model-queue-traces.jsonl`; mirrored byte-for-byte to `artifacts/model-traces.jsonl`.
- **Row shape is NOT `BenchQueueEvent`.** `BenchQueueEvent` (`src/lib.rs:169-188`) is the bench's lenient *reader* type (aliases `kind→operation`, `pending→queued`). On-disk rows vary by status: `queued` = `queue_id, item_id, operation, status, attempt, timestamp, request_units, input_units, input_hash`; `running` adds `queue_wait_ms, throttle_wait_ms`; `succeeded` (embedding) adds `run_ms`; `succeeded` (chat) adds `run_ms, usage{cache_hit_tokens, cache_miss_tokens, completion_tokens, prompt_tokens}`; `failed` adds `run_ms, error`.
- **Rows are lifecycle events, not terminal calls** (~3 rows per logical request). Reference distribution: embedding 26,593 queued / 26,596 running / 26,593 succeeded / 3 failed; chat 18,130 queued / 18,134 running / 18,130 succeeded / 4 failed. **Total 134,183 rows.**
- The on-disk `usage` sub-keys (`cache_hit_tokens`, `prompt_tokens`, …) differ from the bench's `TokenUsage` fields (`input_tokens`, `cached_input_tokens`, …) at `src/lib.rs:118-124`; the reader normalizes.

### 4.3 Response cache
`provider-queue/responses/<queue-id-slug>/<operation>/<hash>.json` (e.g. `.../chat-deepseek-deepseek-v4-flash/chat/<sha256>.json`). Shape `{text, usage, finish_reason}`, keyed by input hash (dedup/replay). 18,081 chat files in the reference run (1,500 in `f500-gate-1`).

### 4.4 workflow queue.sqlite
The per-question **WORKFLOW** queue — distinct from the provider queue; schedules one work item per benchmark question, not per LLM call. `workflow/longmemeval/queue.sqlite` (+`-shm`,`-wal`). Tables: `queue_items` (500 rows, one `longmemeval.row` per question), `queue_events` (3,239 rows), `queue_cooldowns` (0), `sqlite_sequence`. `queue_items` cols: `item_id, queue_id, kind, payload_json, status, attempt, max_attempts, run_after, lease_owner, lease_until, idempotency_key, last_error, created_at, updated_at` (all `succeeded`; `idempotency_key = "<qid>:<workflow_input_hash>"`). `queue_events` cols: `event_id, item_id, queue_id, kind, status, attempt, timestamp, error` (distribution: pending 500 / running 2,239 / succeeded 500 — retries inflate running). **No `model_queue.sqlite` exists on disk** despite a `model_queue_sqlite` trace-artifact key at `src/symbiotic_memory_adapter.rs:2093`; the only sqlite files are this workflow queue + one `memory.sqlite` per vault.

### 4.5 Per-question record streams (one row per question)
Written to `raw/` (the native writer dir, `native_raw_dir = run_root/raw`, `src/bin/membench.rs:1921`), then `copy_artifact` (`:1992-2007`) byte-duplicates each into `artifacts/`.
- **hypotheses.jsonl** — 8 keys: `question_id, question_type, question, hypothesis, debug_artifact, router_initial, router_final, router_reason` (reader `Hypothesis` `src/artifacts.rs:53-67` also tolerates `router_pick`). `debug_artifact` → `vaults/<qid>/debug/hypotheses/<run_id>/question-debug.json`. 500 rows.
- **verdicts.jsonl + partial-verdicts.jsonl** — 10 keys: `question_id, question_type, question, answer, hypothesis, judge_raw, autoeval_label{model,label}, label, is_abstention, error` (reader `Verdict` `src/artifacts.rs:13-43`). 500 rows each. **They are NOT byte-identical but their per-qid content is identical (0 differing rows / 500)** — the only difference is ROW ORDER: `partial-verdicts.jsonl` is completion/stream order (the in-progress mirror), `verdicts.jsonl` is the final sorted-by-qid version.
- **provenance.jsonl** — 23 keys: `answerer, benchmark, consolidate_briefs, dataset, debug_artifact, distiller, embedder, final_pick, initial_pick, judge_model, judge_operator, judge_workers, memory_config, memory_trace_ids[], query_planner, question_id, routed, router_reason, run_name, schema("membench.provenance.v1"), scorer, store, system` (reader `Provenance` `src/artifacts.rs:69-79` reads only 4). `memory_trace_ids` links each question to its stage traces (30 in the sampled row). 500 rows.
- **question-debug.json** — written by `src/symbiotic_memory_adapter.rs:~2145-2160` to TWO places per vault: `vaults/<qid>/debug/question-debug.json` (latest) AND `vaults/<qid>/debug/hypotheses/<debug_run_id>/question-debug.json` (snapshot), both via `write_json_atomic`. 11 top-level keys (full schema in §6).

### 4.6 Derived / aggregate (single JSON each)
- **gold-eval.json** — written to `artifacts/gold-eval.json` by the `gold-eval` subcommand (`src/bin/membench.rs:459`). **NOT present in every run** (absent from the reference 500 run). Full structure in §5/§6.
- **scored.json + score-summary.json** — both written to `raw/`, copied to `artifacts/` byte-identically. `scored.json`: `counts, judge_model, judge_prompt_mode, overall_accuracy, per_question_type, task_averaged_accuracy`. `score-summary.json`: `elapsed_ms, hypotheses_file, judge_model, judge_prompt_mode, metrics, schema_version, scored_file, scorer, verdicts_file`.
- **step-analytics.json** — `artifacts/step-analytics.json`, a DERIVED rollup of streams 4.1, 4.2, and question-debug (~12 MB), by `write_step_analytics_artifact` → `step_analytics::derive_run_step_analytics` (`src/step_analytics.rs:65-66`).

### 4.7 Duplication situation (CONFIRMED, both runs, via `sha256`/`ls -li` — independent copies, NOT hardlinks)

| artifacts/ file | byte-identical duplicate of |
|---|---|
| `memory-traces.jsonl` | `traces/memory-events.jsonl` |
| `model-traces.jsonl` | `provider-queue/model-queue-traces.jsonl` |
| `hypotheses.jsonl` | `raw/hypotheses.jsonl` |
| `verdicts.jsonl` | `raw/verdicts.jsonl` |
| `partial-verdicts.jsonl` | `raw/partial-verdicts.jsonl` |
| `provenance.jsonl` | `raw/provenance.jsonl` |
| `score-summary.json` | `raw/score-summary.json` |
| `scored.json` | `raw/scored.json` |

Copies produced by `std::fs::copy` via `copy_artifact()` (`src/bin/membench.rs:1992-2002`). The redundancy is a **load-bearing lookup fallback**: `membench-server.rs:226-231` and `src/live.rs:935-941` both try candidate paths (`provider-queue/`→`artifacts/`→`raw/`→`traces/`) for the same logical trace file. **`raw/memory-traces.jsonl` and `raw/model-traces.jsonl` do NOT exist in native runs** — those paths are fallback READ locations only (`src/cost.rs:383`, `src/live.rs:894/1147`, `src/registry.rs:977-978`) for the external-import path. Only `step-analytics.json` is unique to `artifacts/`. A **third** in-vault duplication: `vaults/{id}/debug/question-debug.json` == `vaults/{id}/debug/hypotheses/hypotheses/question-debug.json` (both 270,288 B). Net: the two large trace files each exist in exactly 2 copies (~248 MB of duplicated trace data in a ~270 MB run), the six small per-question artifacts each in 2 copies, and `step-analytics.json` is a 3rd derived re-encoding of the same trace+question-debug data.

---

## 5. Constructed Artifacts & Materialization Timing

Materialization vocabulary: **post-run persisted file** (written once at finalize/score, survives as a named file); **incremental/live-during-run** (tail-based, ephemeral); **request-time-derived-at-endpoint** (recomputed per HTTP request, never persisted); **client-derived** (computed in the Svelte app off a payload). There is **no on-question-completion derived artifact** — per-question outputs are raw appends during the run; every per-question *analytic* is materialized later (post-run file) or per request.

| # | Artifact / structure | Builder (file:line) | Inputs | Timing |
|---|---|---|---|---|
| A1 | `benchmark-report.json` → `cohort`/`models`/`config_signature`/`metrics.{cost,latency,cache}` | `enrich_report_with_cohort` (`membench.rs:1821-1858`) → `registry::compute_cohort_fields` (`registry.rs:671-713`) wrapping `cost::rollup_model_traces` (`cost.rs:377-655`) + `cohort::{dataset_fingerprint,config_signature}` (`cohort.rs:64-126`) | `model-traces.jsonl` (or `provider-queue/…` fallback, `cost.rs:377-388`), `scored.json`, `run-params.json`, `verdicts.jsonl` | **post-run persisted** (at report finalize) |
| A2 | `artifacts/step-analytics.json` (`membench.step_analytics.v1`) | `step_analytics::derive_run_step_analytics` (`step_analytics.rs:64-96`), written by `write_step_analytics_artifact` (`membench.rs:2009-2020`) | `memory-traces.jsonl`, `model-traces.jsonl`, `vaults/*/debug/question-debug.json`; queries **hashed not stored** for publishability (`step_analytics.rs:353-394`) | **post-run persisted** (finalize :2197, import :1797, standalone :2383) |
| A3 | `artifacts/gold-eval.json` (schema 1) — sole source of the embed-vs-rerank rank heatmap | `gold_eval` (`membench.rs:2725-3089`), atomic `.json.partial`→rename (:3065-3068) | `verdicts.jsonl`/`scored.json`, gold dataset, per-question recall candidate sets (embedding_score + rerank_score) | **post-run persisted**, auto-refreshed after every scored run (:3901-3905) and `--rejudge` (:3725-3727) |
| A4 | Trial ledgers: `trial-stack.json`, `trials.jsonl`, `trial-question-deltas.jsonl` (under `runs/analysis/**`) | `trials::derive_trial` (`trials.rs:119-153`) | 2–3 runs' merged `QuestionRow`s (`RunView::load` :491-518 → `artifacts::question_rows`), each run's report/scored aggregate, per-question debug bundles | **post-run persisted** (on-demand via `trial` CLI, not auto at finalize) |
| B5 | `LiveDetail` (`QueuePressure`/`QueueBreakdown[]`/`ModelLive`/`StageProgress[]`+`StageSegment[]`/`ErrorCategory[]`/`LiveActivity[]`) | `live::live_detail` (`live.rs:739-1110`), served by `/run/live` | bounded tail (`TAIL_BYTES=1_200_000`) or capped-whole read (`PROVIDER_FULL_CAP=80MB`, `MEMORY_FULL_CAP=40MB`) of model/memory traces | **incremental/live** (recomputed each poll; **cached 2.5s TTL**) |
| B6 | `PendingRun[]` | `registry::scan_pending` (`registry.rs:942-1035`) | `run-params.json`, `raw/*.jsonl` mtimes, `vaults/` entries; status from mtime windows (`RUNNING_WINDOW_MS=180s`/`STALLED_WINDOW_MS=300s`) | **incremental/live** (uncached re-scan) |
| C7 | Leaderboard cohort matrix `Vec<Cohort>` w/ `RankedRow[]` | `leaderboard::build_cohorts` (`leaderboard.rs:52-134`) | cached snapshot `Vec<RunSummary>` | **request-time** (from cached snapshot) |
| C8 | `RunSummary` index rows | `registry::summarize_with_trials` (`registry.rs:351-494`) | `benchmark-report.json`, `run-params.json`, `scored.json`, `verdicts.jsonl` | **request-time**, but memoized in the registry snapshot |
| C9 | `TrialMarker[]` | `registry::scan_trial_markers` (`registry.rs:216-276`) | `runs/analysis/**/trials.jsonl` | **request-time** (folded into snapshot) |
| C10 | Run-detail cohort + cost payload (`ModelTraceRollup`, `cost.rs:131-154`) | `/run` recomputes `cost::rollup_model_traces` + `compute_cohort_fields_with_rollup` **live** — does NOT read persisted `report.cohort`; re-parses `model-traces.jsonl` | pricing from static table + lazily-loaded OpenRouter catalog (`cost.rs:212-216`); rerank billed per-search (`cost.rs:344-349`) | **request-time** (duplicate of persisted A1) |
| C11 | Run "traces" analytics bundle (6 structures) — `memory_stage_timing` (`:2645-2753`), `queue_timing` (`lib.rs:351-400`), `trace_waterfall` (`:1991-2065`, cap `WATERFALL_BLOCK_CAP=8000`), `dependency_waterfall` (`:2091-2365`), `trace_events` (`:1884-1964`, cap `TRACE_EVENT_CAP`), `workflow_queue` (`:2890-2959`) | `/run/traces` | `memory-traces.jsonl`, `model-traces.jsonl`, `workflow/**/queue.sqlite` | **request-time** (all six recomputed per request, none persisted) |
| C12 | Compare result (`CompareResult`/`CompareCounts`/`TypeDelta[]`/`ChangedRow[]`) | `compare::compare_runs` (`compare.rs:82-177`) | both runs' `question_rows` | **request-time** |
| C13 | Merged `QuestionRow[]` | `artifacts::question_rows` (`artifacts.rs:239-299`) | LEFT-joins `verdicts.jsonl` + `hypotheses.jsonl` + `provenance.jsonl` on qid | **request-time** (also input to compare + trials) |
| C14 | Question-debug passthrough | `/run/question-debug` (:1652-1671) | one `vaults/*/debug/question-debug.json`, path-validated | **request-time file read** (not a derivation) |
| D15–19 | Radar chart, category-heat matrix, compare-metric deltas, gold rank-distribution heatmap + tail percentiles, compare transition tiles | `Leaderboard.svelte`, `GoldCoverage.svelte`, `Compare.svelte` | `per_question_type` (A1/C8), persisted `gold-eval.json` (A3), `/compare` payload (C12) | **client-derived** |

**Cross-cutting**: the cohort/cost rollup exists BOTH as a persisted file field (A1) AND recomputed request-time (C10/C11) — `compute_cohort_fields` is deliberately shared so "live == persisted" (`registry.rs:5-8` doc; `cost.rs:12-14` doc). Every JSON endpoint's `.pb` twin runs the SAME derivation then serializes via `*_pb` mappers (e.g. `gold_eval_pb` :1214, `queue_timing_pb` :1048) — no extra derivation, just encoding.

---

## 6. The De-Facto Question/Gold Join Model (`QuestionRunRecord`)

The join already exists, materialized two ways:
1. **`QuestionRow`** (`src/artifacts.rs:82-101`), built by `question_rows()` (`:239-299`), LEFT-joins `verdicts.jsonl` + `hypotheses.jsonl` + `provenance.jsonl` on `question_id`. This is the closest thing to an explicit `QuestionRunRecord`.
2. **`question-debug.json`** (`write_question_debug`, `src/symbiotic_memory_adapter.rs:2072-2152`) — the per-question debug bundle nesting the full recall/answer trace plus (post-hoc) scoring.

The canonical key everywhere is **`question_id`** (LongMemEval's per-question hash, e.g. `e47becba`). It is also the vault directory name: `vault_dir = run_root/vaults/{question_id}` (`symbiotic_memory_adapter.rs:1518, 2210, 2430`).

### What `question_id` joins to

| Entity | Source | Field / path |
|---|---|---|
| Question text/type/date | dataset → debug bundle | `question.{text,type,date}` (`adapter.rs:2106-2111`) |
| Gold answer | dataset `record.answer` | `question.gold_answer`; `answer` in verdict (`artifacts.rs:24-25, :87`) |
| Gold evidence pieces | dataset `answer_session_ids` (`adapter.rs:117`) → `has_answer` turns via `gold_turn_ids()` (`membench.rs:2585-2600`) | computed at gold-eval time (not in bundle) |
| Hypothesis / model answer | `hypotheses.jsonl` / `answer.json` | `hypothesis.hypothesis`; `recall.final_answer.text` |
| Judge verdict + prompts + model | `verdicts.jsonl` | `label`/`autoeval_label.{model,label}`, `judge_raw`, `judge_system_prompt`, `judge_user_prompt`, `is_abstention` |
| Provenance / router picks / profile | `provenance.jsonl` | `initial_pick`, `final_pick`, `router_reason`, `routed`, `query_planner`, `answerer`, `embedder`, `distiller`, `store`, `memory_config` |
| Query plan / expected_answer_type | bundle | `recall.query_plan.{canonical_query,dense_queries,sparse_terms,expected_answer_type,needs_raw_turns,time_window}`, `recall.query_planner_call` |
| Retrieved candidates / evidence | bundle | `recall.rerank_trace[].candidates[]`, `recall.initial_profile.{facts[],raw_turns[]}`, `recall.final_answer.evidence_ids[]`, `recall.answerer_calls[0].context[]` |
| Memory trace ids | `provenance.jsonl` | `memory_trace_ids` (keys into `memory-traces.jsonl`) |
| Provider trace ids | bundle | `provider_trace_artifacts.{model_traces_jsonl, model_queue_traces_jsonl, model_queue_sqlite, response_cache_dir}` |
| Debug bundle path | hypothesis + provenance | `debug_artifact = vaults/{qid}/debug/hypotheses/{debug_run_id}/question-debug.json` |

`question_rows()` currently pulls **only** the verdict/hypothesis/provenance slice — it does **not** open the debug bundle or gold-eval, so retrieved candidates, gold pieces, and traces are addressed but not folded into the flat row.

### `question-debug.json` shape (written `adapter.rs:2104-2138`)
Top-level keys: `schema_version:1`, `question{id,type,text,date,gold_answer}`, `source{haystack_dates, haystack_session_ids, haystack_session_count, source_turn_count, indexed_turn_count}`, `workflow{routed, answer_only, consolidate_briefs, input_hash}`, `ingest{active_fact_count, manifest_path}`, `runtime{bench_owned_metadata, trace_note}`, `recall` (the meat, 13 sub-keys), `hypothesis` (BenchHypothesis), `provider_trace_artifacts{model_traces_jsonl, model_queue_traces_jsonl, response_cache_dir}`, `scoring:null` (back-filled post-hoc).

`recall.*` nesting: `schema_version, question, reference_date, recall_profile, initial_profile{question, intent{expected_slot,intents[],requires_artifact,requires_raw_turns}, facts[], raw_turns[], support}`, where `facts[].fact = {memory_id, content, search_text, confidence, event_time, source_refs[]{turn_id,source_id,receipt_id,captured_at}, subjects[], tags[], status, slot_key, embedding_profile}` + `facts[].score` (used for gold-eval fact ranks); `raw_turns[] = {ordinal, score, speaker, text, source_ref{turn_id,source_id}}`; `query_plan{canonical_query, dense_queries[], sparse_terms[], expected_answer_type, needs_raw_turns, time_window}`; `query_planner_call{mode, system_prompt, user_prompt, response_text, parsed_plan, usage, finish_reason}`; `retrieval_queries[]`; `rerank_trace[]{candidate_type, top_k_target, candidates_submitted, candidates[]{candidate_id, embedding_rank, embedding_score, final_rank, rerank_score, text}}`; `answerer_enabled, answerer_calls[]{phase, system_prompt, prompt, context[], response_text, processed_text, reasoning, finish_reason, usage{prompt_tokens,completion_tokens,cache_hit_tokens,cache_miss_tokens}}`; `final_answer{text, support, evidence_ids[]}` (evidence_ids mix mem-/brief-/raw ids); `fallback_used, policy{fact_top_k, raw_turn_top_k, answer_system_prompt, evidence_ledger}`.

The `scoring` block is null at write time, back-filled by `update_question_debug_score` (`adapter.rs:2508-2543`) into BOTH copies: `scoring{scorer, scored_artifact, verdicts_artifact, verdict{question_id, question_type, label, error}}`.

**Schema drift under a single `schema_version:1`:** an older sampled file (`runs/.../50/n50-coh-3/vaults/6aeb4375/…`) has a top-level `gold_positions` key (`{candidates, method, first_initial_fact_rank, first_initial_raw_turn_rank, first_fallback_raw_turn_rank}`) that the current writer no longer emits.

### `gold-eval.json` + gold evidence mapping (`membench.rs:2725-3068`)
- **Gold pieces** = `record.answer_session_ids` (session-level). **`gold_turn_ids()`** refines to turn-level: for every message with `has_answer==true`, emit `"{session_id}:{turn_index}"` (`membench.rs:2585-2600`). **`gold_piece_of_turn()`** maps a turn id back to its session by splitting on `:` (`:2570-2572`). Matching is strictly **by turn id, never substring** (`:2713-2724`).
- **Fact-vs-raw coverage** (`:2914-2935`), per gold piece as `both/fact/raw/none`: **fact-covered** = a kept distilled fact's `source_refs[].turn_id` maps to the piece (`:2807-2825`) — only fact coverage drives `class` (`covered_pieces` → `retrieval_gap` vs `reader_fail`). **raw-covered** = the piece appears in `final_answer.evidence_ids` OR in `rerank_trace` candidates of `candidate_type=="raw_turn"` (`:2826-2859`; `evidence_id_is_raw_turn` `:2576-2578`: has `:` and not `mem-`/`brief-`).
- **Two different rankers**: `gold_top_rank`/`gold_deepest_rank` = 1-based first/worst rank among kept facts sorted by `.score` desc (`:2896-2907`); `gold_embed_rank`/`gold_rerank_rank` = deepest gold **turn** rank among raw-turn candidates re-ranked among themselves by `embedding_score`/`rerank_score` (`deepest_gold_rank` `:2682`, `raw_turn_candidates` `:2620-2674`). `gold_turns_in_set` = rank denominator.

### Addressing: path vs id (redundant + inconsistent)
- **By `question_id` (the real key):** vault dir, join key in `question_rows()`/`verdict_by_question` (`adapter.rs:2395-2398`), queue idempotency key `{question_id}:{input_hash}` (`:2566`), primary field in every artifact.
- **By path (`debug_artifact` string):** `vaults/{qid}/debug/hypotheses/{debug_run_id}/question-debug.json`, computed `:2025-2032`, stored on `BenchHypothesis`/`Provenance`/`QuestionRow`, but **re-derived (not stored-then-read) for the scoring back-fill**: `update_question_debug_score` rebuilds `run_root/vaults/{qid}/debug/question-debug.json` from the qid (`:2516-2521`). `debug_run_id` derives from the hypotheses filename (`:2478-2505`, defaults to `hypotheses` → the observed doubled `.../hypotheses/hypotheses/...` path). The gold-eval reader ignores the pointer entirely and reconstructs from the id (`membench.rs:2785-2789`). A redesign can drop `debug_artifact` and address purely by `question_id` + a known layout.

---

## 7. Protobuf Migration State

**Single shared schema, no RPC.** One proto file: `proto/membench/dashboard/v1/debugger.proto` (~654 lines, ~60 messages, `proto3`, package `membench.dashboard.v1`). **Only messages — zero `service`/`rpc` blocks** (the lone "service" hit is `HealthResponse.service`). Transport stays plain HTTP GET/POST over axum, not gRPC/Connect.

### Toolchain
- **Rust — prost via build.rs (no tonic, no buf):** `build.rs:33-38` compiles with `prost_build::Config::new().compile_protos(&[proto], &[proto_root])` + `rerun-if-changed` (:35) → `OUT_DIR/membench.dashboard.v1.rs`. `src/dashboard_proto.rs` (8 lines) is the `include!` shim. `Cargo.toml` adds `prost = { version = "0.14", optional = true }` (folded into the `server` feature) and `prost-build` as `[build-dependencies]`. `Cargo.lock`: prost/prost-build/prost-derive/prost-types present, **tonic count = 0**.
- **TS — protobuf-es (@bufbuild) v2:** `dashboard/package.json` adds `@bufbuild/protobuf@^2.12.1` + `@bufbuild/protoc-gen-es@^2.12.1` and a manual `proto:gen` npm script (needs system `protoc` at `/opt/homebrew/bin/protoc`). Generated `dashboard/src/lib/gen/membench/dashboard/v1/debugger_pb.ts` (3085 lines, `protoc-gen-es v2.12.1`, `target=ts`) exports 61 `*Schema` descriptors + camelCase types, consumed via `fromBinary()`. Not ts-proto, not protobuf-es v1.
- **Flutter/Dart** target exists only in the spike (`spikes/flutter-http-debugger/scripts/gen-proto.sh`, official `protoc_plugin`); no Dart gen in the main repo.

### What is protobuf end-to-end vs still JSON
Every endpoint is **dual-served**: 27 JSON routes + 14 parallel `.pb` twins (`membench-server.rs:267-293`). `proto_response()` (:349-353) sets `content-type: application/x-protobuf`. The Svelte dashboard consumes **PB-first with JSON fallback on every call** (`getPb`/`postPb` → `.then(mapXxx).catch(()=>get(json-url))`, `api.ts:365-457`). PB is the happy path everywhere; JSON is the safety net.

### The defining smell: double-encoding, JSON-bracketed on both ends
The `.pb` handlers do NOT build protobuf from native types directly — most serialize to `serde_json::Value` first and re-read field-by-field into prost structs:
- ~50 `*_pb(value: &Value)` converters (`:571-1346`) read via string keys (`str_field`, `opt_f64_field`).
- Even native-typed converters round-trip: `run_summary_pb(&RunSummary)` calls `value_obj(summary)` = `serde_json::to_value(..)` then reads keys back (`:449-503`); same for `pending_pb_row`, `question_row_pb` (:505-529).
- Handlers holding native results still round-trip: `run_detail_pb` → `cohort_fields_pb(&serde_json::to_value(...)...)` + `model_rollup_pb(&serde_json::to_value(rollup)...)` (:3203-3208); `run_live_pb` → `live_detail_pb(&serde_json::to_value(&*detail)...)` (:3269-3271); `run_traces_pb` builds a giant `json!({...})` then `traces_pb_from_value` (:3388-3398); `compare_pb` → `serde_json::to_value(result)` → `compare_result_pb` (:3433-3437). On the PB path the server pays **serialize-to-JSON + reparse-into-prost + prost-encode**.
- Symmetrically the TS client decodes PB then maps straight back into the snake_case JSON-shaped `types.ts` DTOs via `mapRunSummary`/`snakeKeys`/`camelToSnake` (`api.ts:71-84, 108-153, 266-288`). **Protobuf never reaches the rest of the app** — `types.ts` (705 lines, all hand-written) is unchanged in shape. PB is a wire-only interlayer bracketed by JSON on both ends.

### Design quality of the messages
A flat DTO mirror of the existing JSON, not a real envelope+typed-payload model. Each response maps 1:1 to a top-level `*Response` message with snake-cased JSON keys. **No shared envelope, no `oneof` typed-payload union, no error message type** (errors still return JSON `{"error":...}`, `membench-server.rs:326`; the TS fallback reads `res.json().error` even on the PB path, `api.ts:22,31`). Trace data is parallel flat repeated rows (`TraceEventRow`, `QueueTiming`, `TraceWaterfall*`, `DependencyWaterfall*`), not a typed event stream — `kind`/`status`/`operation`/`event` are all free `string`s. **No enums anywhere** — every categorical (`status`, `decision`, `transition`, `severity`, `run_kind`, `classification`) is a bare string. Numeric quirk: `int64 cost_micro_usd` decodes to JS `bigint`, coerced back with `Number(...)` (`api.ts:62,135`).

**8 opaque `*_json` string escape-hatch fields** carry whole JSON blobs inside protobuf, defeating the typed schema: `roles_detail_json` (proto 190), `report_json`/`params_json` (214-215), `answer_json` (578), `QuestionDebugResponse.json` (606), `ArtifactResponse.json` + `rows_json` (610-611), `ParamField.default_json` (621). The client re-`JSON.parse`s these (`api.ts:63-70, 223-224, 295, 321, 325, 331, 343`). So `run-detail`, `question-debug`, `artifact`, and gold-eval `answer` payloads are **effectively still JSON, merely tunneled through a PB envelope**. The `.pb` variants for artifact/question-debug buy nothing but an extra copy.

### Migration completeness / half-done / dead scaffolding
- **Server: complete but duplicative.** All 14 `.pb` handlers exist and compile behind the `server` feature; each is a *second copy* of its JSON handler (`run_live` :1480-1537 vs `run_live_pb` :3216-3277 are near-identical). Nothing was deleted — JSON handlers all remain. The `git diff src/bin/membench-server.rs` = **+1470 / −1** (baseline commit `c623000` is JSON-only). Roughly doubles handler surface plus ~50 `*_pb` converters.
- **`/run/gold-eval.pb` is brand-new with NO JSON twin** (only registration :285). The JSON path to gold-eval is instead the generic `/run/artifact?kind=gold_eval` (row 17), which returns the raw file untyped — a different endpoint AND shape than the `.pb` primary. The Flutter spike doc (`README.md:319-324`) still says these messages are "not added to the schema" — it **trails the Rust/TS side, which already has them**.
- **TS client: complete on the wire, gains thrown away** by mapping back to JSON-shaped `types.ts`; components were not migrated to consume prost types directly. The dashboard client (`api.ts`) calls **only** the `.pb` endpoints as primary (`:367-454`) — the SPA has fully cut over; the JSON routes are legacy fallback.
- **All proto scaffolding is UNCOMMITTED/untracked:** `proto/`, `src/dashboard_proto.rs`, `src/bin/trace_pb_bench.rs`, `dashboard/src/lib/gen/` are `??`; `Cargo.toml`, `Cargo.lock`, `build.rs`, `dashboard/package*.json`, `api.ts`, `types.ts`, `Live.svelte` are `M` — consistent with the repo's "keep, don't commit" cadence.
- **`src/bin/trace_pb_bench.rs`** (148 lines) is a throwaway micro-benchmark: reads a captured `traces.pb`, loops decode/encode/materialize, prints medians. Its hand-rolled `materialize_trace_events` rebuilds a `serde_json::Map` per row — it measures the exact PB→JSON re-materialization double-work the architecture imposes. Dead product scaffolding, useful only as a transport spike artifact.
- **No `buf` lint/breaking-change tooling, no CI codegen check.** `proto:gen` is manual; prost regenerates on build.

### Frontend/transport spike conclusions
Two converging spike bodies:
- **`spikes/DASHBOARD-MATRIX.md`** (framework bake-off vs egui parity): (1) Web = **Svelte** (54 KB brotli, zero rendering bugs, vs iced 1,352 KB / egui 1,518 KB / Flutter ~2.6 MB CanvasKit); (2) app wrapper = **Svelte + Tauri 2**; (3) if canvas required, **egui over iced** (iced has 4 critical WebGPU bugs); (4) **Flutter not competitive for web** (~50× Svelte bundle); (5) Rust bridge (if any) = **FRB over rinf**.
- **`spikes/flutter-http-debugger/`** (newer): **dropped frontend Rust entirely** — the FRB layer exposed only 7 thin HTTP-loader functions, so Dart calls the backend directly (`README.md:21-45`). New shape: Flutter UI → Dart HTTP → membench-server, `--wasm`/skwasm, "backend owns compute/storage/schema." **Transport verdict = the shared Protobuf schema is the API contract** (`README.md:293-324`): prost + protobuf-es + official Dart plugin off the one `debugger.proto`; "do not reintroduce frontend Rust/FRB." Size: `--wasm` no-Rust Flutter = 2.06 MB (31.9× Svelte's 66.2 KB); Dart protobuf client cost only +15 KB.

**Net:** framework resolved to "**Svelte stays production; Flutter-HTTP (no Rust bridge) is the mobile candidate**"; transport resolved to "**one shared prost/protobuf-es schema over plain HTTP, PB-first with JSON fallback**" — exactly what the uncommitted migration implements, in a mechanically-transcribed, doubly-encoded, JSON-bracketed first cut.

### Frontend↔backend contract drift (proto ↔ `types.ts`) to resolve
- `QTypeScore`: proto has `total`, DTO drops it (`api.ts:112`).
- `Cohort`: proto has `judge_prompt_modes`, DTO omits it (`debugger_pb.ts:682` vs `types.ts:72`).
- `ModelStat`: proto has 7 cache/failed fields the DTO lacks (`debugger_pb.ts:764-796`).
- `ModelRollup`: proto has cache counters + `roles_detail_json` the DTO lacks.
- `QueueSummaryRow` (`types.ts:342`): has **no proto message** and is never produced by any `api.ts` method — JSON-only legacy or unused.
- `AnswererCallDebug` (`types.ts:151`): defined but not referenced by any other type — dead / consumed dynamically.
- `mapRunDetail`/`mapLive`/`mapTraces`/`mapGoldEval`/`mapCompare` don't hand-map every field; they call `snakeKeys()` (`api.ts:71-84`), so proto/DTO field drift is silently tolerated for those DTOs.

---

## 8. On-Disk Run Layout Reality

### Registry root
`runs/{system}/{benchmark}/{limit}/{run_name}/` → on disk `runs/symbiotic-memory/long-mem-eval/{N}/{run_name}/`, with `N` buckets `1,3,5,10,20,30,50,61,100,122,200,500` holding 2–102 runs each (500 has 102). Alongside: `runs/inputs/` (datasets: `longmemeval-cleaned/…`, `longmemeval-hard/…`), `runs/.locks/` (empty), `runs/.tmp/` (server stdout/stderr logs). The tracked/portable sibling `records/{system}/{benchmark}/{limit}/{name}/` is git-tracked, artifact-only (each record: `run-params.json`, `benchmark-report.json`, `artifacts/`, sometimes `provider-queue/model-queue-traces.jsonl`; no `vaults/`/`raw/`/`workflow/`).

### Canonical per-run directory (verified across two runs)
```
{run_root}/
  run-params.json          # schema "membench.run_params.v1"
  benchmark-report.json    # schema "membench.report.v1"
  .store-zvec              # 7-byte marker
  artifacts/               # PUBLISHED-ARTIFACTS LAYER
  traces/                  # native live-execution layer
  provider-queue/          # native live-execution layer (+ responses/)
  workflow/                # native live-execution layer (queue.sqlite)
  vaults/                  # native live-execution layer (memory substrate)
  raw/                     # LEGACY RAW LAYER
```
Native-state dir set is code-defined: `PRUNE_DIRS = ["vaults","workflow","provider-queue","raw","artifacts"]` (`src/registry.rs:127-132`; the walker prunes these and doesn't follow symlinks into them).

### File-level contents (500-run `f500-gate-1`)
- **artifacts/** (29 MB, 9 files): `hypotheses.jsonl` 500 lines, `verdicts.jsonl` 500, `partial-verdicts.jsonl` 500, `provenance.jsonl` 500 (1.5 MB), `scored.json` 1 KB, `score-summary.json` 1.6 KB, `memory-traces.jsonl` 17,040 lines (~13.3 MB, ~34/question), `model-traces.jsonl` 8,638 lines (~3 MB), `step-analytics.json` ~11.8 MB (unique to artifacts/).
- **traces/**: `memory-events.jsonl` (13,286,261 B / 17,040 lines).
- **provider-queue/**: `model-queue-traces.jsonl` (2,993,591 B / 8,638 lines) + `responses/{queue-label}/chat/{sha256}.json` (1,500 cached bodies).
- **workflow/**: `workflow/longmemeval/queue.sqlite` (655,360 B; durable row-level work queue, `docs/schemas.md:411-414`).
- **raw/**: the six answer/score/provenance files, 500 lines each.
- **vaults/**: 500 per-question vault dirs keyed by `{question_id}` (8-hex), plus variants `{id}_abs` (30) and `gpt4_{id}` (107). Apparent 320 MB but 315.7 MB real files + 1,500 symlink entries.

### The three storage layers (evidenced)
1. **Native live-execution layer** (`traces/`, `provider-queue/`, `workflow/`, `vaults/`) — raw unnormalized adapter output. `run-params.json`: `run_kind:"native"`, `provider_queue_available:true`, `workflow_queue_available:true`. `benchmark-report.json.artifact_manifest.native_state_available:true`. Heavy (`vaults/` alone 315.7 MB; whole run 374 MB) and local/private (may contain raw prompts, `docs/schemas.md:281-289`). Designated for external storage / meta-records that omit it (`records/README.md:33-46`, `canonical-record-storage-task.md:47-55`).
2. **Published-artifacts layer** (`artifacts/`) — the normalized, portable, git-trackable projection. `benchmark-report.json.artifact_manifest.available` enumerates exactly the 9 artifact kinds; `benchmark-report.json.artifacts.{kind}` carries per-file `{bytes, non_empty_lines, sha256, path}` (`docs/schemas.md:244-335`). Deliberately a copy of live-layer trace files plus derived rollups (`step-analytics.json`). `docs/schemas.md:271-274` documents `model_traces` as "copied from `provider-queue/model-queue-traces.jsonl`."
3. **Legacy raw layer** (`raw/`) — the answer/score/provenance outputs predating `artifacts/`. Every `raw/` file is byte-duplicated into `artifacts/`. Still authoritative in code: `score-summary.json` records `hypotheses_file`/`scored_file`/`verdicts_file` pointing at `.../raw/...`; `src/live.rs:941` defaults to `raw/memory-traces.jsonl`.

### Answer-only reruns: vault symlink substrate
`f500-gate-1` is an answer-only rerun (`answer_only:true`, `source_vault_root:"runs/.../factconsol-thinkon-500-20260624/vaults"`). Its `vaults/{id}/` hold **real** `answer.json`, `manifest.json`, `debug/` but **symlink** the immutable substrate (`archive`, `memory.sqlite`, `zvec-hybrid`) → the golden source run's vault (verified: 1,500 symlinks, 0 real `memory.sqlite`, 500 real `manifest.json`/`answer.json`; source run has a real 16,994,304 B `memory.sqlite`). Matches `docs/schemas.md:276-279`. By contrast a fresh ingest run (`stage0-h1-64x32-distill`) has a **real** 45,056 B `memory.sqlite` + real `archive/`, no symlinks, no `zvec-hybrid`/`debug` (config-dependent vault shape).

### Authoritative schema docs
`docs/schemas.md` (run-params `membench.run_params.v1`, report `membench.report.v1`, cohort identity, artifact_manifest vocabulary, memory-trace JSONL fields, queue events, trials), `docs/run-registry.md`, `docs/canonical-record-storage-task.md` (intended promotion model: `artifacts/` tracked, native state externalized as `native-state.tar.zst` + `external-artifacts.json`), `docs/bench-explorer-design.md`, `docs/environment.md`, `records/README.md`.

---

## 9. Structural Smells, Redundancies & Gaps (the redesign punch-list)

### Storage & duplication
1. **The two large trace files exist in exactly 2 byte-identical copies each** (`traces/memory-events.jsonl`≡`artifacts/memory-traces.jsonl`; `provider-queue/model-queue-traces.jsonl`≡`artifacts/model-traces.jsonl`) — ~248 MB of duplicated trace data in a ~270 MB run, via `std::fs::copy` (`membench.rs:1992-2002`). Not hardlinks (verified `ls -li`).
2. **The `raw/` layer is a full second copy of the six per-question answer/score artifacts** — every `raw/` file is byte-duplicated into `artifacts/`. `raw/` is legacy but still authoritative (`score-summary.json` points into it; `live.rs:941` defaults to it). Two write targets, read via fallback chains — the duplication is *load-bearing* (`membench-server.rs:226-231`, `live.rs:935-941` try `provider-queue/`→`artifacts/`→`raw/`→`traces/`).
3. **`question-debug.json` is written twice per vault** (latest + `hypotheses/{run_id}/` snapshot, both 270,288 B), producing the doubled `.../hypotheses/hypotheses/...` path from `debug_run_id` defaulting to `hypotheses`.
4. **`step-analytics.json` is a third derived re-encoding** (~12 MB) of the trace + question-debug data.
5. **Dead code path:** `model_queue_sqlite` is referenced as a trace-artifact key (`adapter.rs:2093`) but no `model_queue.sqlite` is ever written in native runs.

### Request-time derivation (belongs in finalize, written as artifacts)
6. **`/run/traces[.pb]` is the worst offender.** Every hit re-parses `memory-traces.jsonl` (up to 4000 rows) and rebuilds a dependency waterfall (277-line `summarize_dependency_waterfall` :2091-2367), a trace waterfall (:1991), a 12000-cap event stream (:1884), queue timing, a cost rollup, and opens the workflow SQLite — with **no caching** (unlike `live_detail`/registry). The `.pb` twin adds a full Value round-trip. This materialization belongs in the scoring/finalize step, written once as an artifact.
7. **`/run[.pb]` recompute the cost rollup + cohort fields on every poll** (C10), re-reading `model-traces.jsonl` each time, ignoring the already-persisted `report.cohort`. Deliberately shared code (`registry.rs:5-8`) but the recompute is uncached.
8. **`/pending[.pb]` re-scan roots on every request** with no cache, unlike the registry snapshot they could piggyback on.
9. **`/leaderboard.pb` double-converts** cohorts to `Value` then re-parses into PB (:3168-3172).

### Protobuf migration (defining smells)
10. **Pervasive double-encoding.** On the PB path the server pays serialize-to-JSON + reparse-into-prost + prost-encode; the TS client decodes PB then maps straight back to snake_case JSON DTOs. **Protobuf is a wire-only interlayer bracketed by JSON on both ends** — no component consumes prost types; `types.ts` is unchanged.
11. **8 `*_json` escape-hatch string fields** tunnel whole JSON blobs through PB (`report_json`, `params_json`, `QuestionDebugResponse.json`, `ArtifactResponse.json`+`rows_json`, `answer_json`, `default_json`, `roles_detail_json`). For artifact/question-debug/run-detail the `.pb` variant is only an envelope — the payload is still JSON, buying an extra copy.
12. **No shared envelope, no `oneof`, no typed error, no enums.** Trace event kinds/status/operation are free strings; every categorical is a bare string. It's a mechanical 1:1 transcription of the JSON, inheriting its shape warts.
13. **Full handler duplication.** All 14 `.pb` handlers are second copies of the JSON handlers; nothing was deleted → ~doubled handler surface + ~50 `*_pb` converters.
14. **Gold-eval asymmetry:** `.pb` primary (typed `GoldEvalResponse`) vs JSON fallback via a *different* endpoint and shape (`/run/artifact?kind=gold_eval`, raw untyped file).
15. **No `buf`, no CI codegen check;** manual `proto:gen`. The Flutter spike doc trails the actual schema state.

### Question/gold join model
16. **`QuestionRow` (the de-facto `QuestionRunRecord`) is under-populated** — it stops at verdict/hypothesis/provenance and never folds in the debug bundle (candidates, facts, traces) or gold-eval (gold pieces, coverage, ranks), though all are keyed on the same `question_id` and reachable from the same run_root.
17. **Dual, inconsistent addressing.** A stable id-addressed store (`vaults/{qid}`) coexists with a leaked path-addressed pointer (`debug_artifact`) stored on three artifacts — yet the scoring back-fill and gold-eval reader both ignore the pointer and reconstruct from the id. A redesign can drop `debug_artifact` and address purely by `question_id` + a known layout.
18. **Schema drift under a single `schema_version:1`** (stale `gold_positions` in older bundles; the writer changed without a version bump). The `MemoryTraceOperation` enum is not authoritative for the free-form `operation`/`stage` strings actually on disk.

### Frontend / IA
19. **No on-question-completion derived artifact** — per-question analytics are all deferred to post-run file or per-request, so nothing incrementally materializes as each question finishes.
20. **Hard-coded config leaks into components** — gold-coverage reference runs (`GoldCoverage.svelte:60`) and the artifact-kind list (`Overview.svelte:28`) are baked in, not server-manifest-driven.
21. **Truncation/density is the dominant UX gap** — fixed-layout horizontal-scroll tables (Leaderboard `min-width:980px`), 200/1200-row render caps, ellipsis clipping across nearly every table, internal `max-height` scroll on waterfalls/feeds.
22. **No global search/deep-link into individual questions;** the Questions drawer state is not URL-addressable; routing has no history stack or query strings.
23. **Repeated, non-centralized fetch pattern** — every section re-implements the same `$effect` + `if (runId !== id) return` race guard.
24. **The Tuner is a preview-only stub** — Runs/Status panel and "SPAWN RUN" button are non-functional; live spawn + log streaming is explicitly the next milestone. Env-default secrets are shown as `key=value` text.
25. **Contract drift proto↔`types.ts`** (see §7): dropped/absent fields (`QTypeScore.total`, `Cohort.judge_prompt_modes`, `ModelStat`/`ModelRollup` cache fields), a DTO with no proto (`QueueSummaryRow`), and a dead DTO (`AnswererCallDebug`) — silently tolerated by the `snakeKeys()` structural mappers.
