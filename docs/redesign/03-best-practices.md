# MEMBENCH Redesign: Best Practices & Implications

**Status:** Architecture-driving decision document. Opinionated by design.
**Scope:** The observability/data model, wire/transport, materialization strategy, competitive positioning, and dashboard IA/UX for the membench redesign — built to generalize past memory systems to *any* multi-prong / hybrid / agentic eval (RAG, tool-calling agents, router+reranker pipelines, model ensembles, queue-driven batch workers).

**The one sentence:** Build *one append-only, generic trace substrate* as the source of truth, project a small set of *materialized views* on top of it, wrap those views in a *Braintrust-grade comparison UX + Langfuse-typed data model + a leaderboard object*, and expose all of it through a *three-workspace persona cockpit* where the URL — not the mode — carries identity.

**Grounding fact about where we are today:** `proto/membench/dashboard/v1/debugger.proto` is a **response/view schema, not a trace schema**. It hand-shapes ~40 request/response messages (`LiveResponse`, `TracesResponse`, `TraceWaterfall`, `GoldEvalResponse`) with heavy stringly-typing (`kind`, `status`, `operation`, `severity`, `transition` as bare `string`) and JSON-in-string escape hatches (`report_json`, `params_json`, `roles_detail_json`, `answer_json`, `debug_artifact`). That is *fine for a materialized view served to a browser*. It is *not* the source-of-truth trace model, and it is memory-benchmark-specific (`GoldRank`, `coverage_by_source`, `memory_stages`). This document's central structural move is to introduce a **separate, generic, append-only trace envelope** as source of truth and demote the `debugger.v1` messages to *one family of post-hoc materializations*.

---

## 1. Tracing / observability data model

### 1.1 Adopt OpenTelemetry's vocabulary as a MAP, not a dependency

Use OTel's conceptual vocabulary (Resource / Trace / Span / Event / Metric / Log / Link) as the mental model. Do **not** take a hard dependency on the OTLP proto, the SDK, or a collector. OTLP is optimized for machine-to-collector export at fleet scale (cardinality control, sampling, batching). An eval debugger wants the opposite: *complete, replayable, human-inspectable* per-run traces carrying domain payloads (gold ranks, rerank scores, token accounting). Keep an OTLP *adapter* as a later option, never a foundation.

Concept-to-role map:

- **Resource** — immutable identity of the run/process: `run_id`, `system`, `benchmark`, `git_sha`, `binary_sha`, host, config signature, dataset fingerprint. Emitted once per run as a header. This is today's `RunSummary` identity fields promoted to a stable resource key that every record carries *by reference, not by copy*.
- **Trace** — one logical unit of work end-to-end. **The natural trace root for an eval tool is one benchmark question / one task instance** (e.g. a LongMemEval question), *not the whole run*. A run is a *set of traces sharing a Resource*. This is what lets you tail, diff, and waterfall a single question — the actual debugging unit.
- **Span** — **work with a duration** (start + end): an embed call, a rerank call, a reader/LLM generation, a queue item's execution, a tool invocation, a retrieval stage. If you can draw it as a bar on a waterfall, it is a Span. Today's `TraceWaterfallBlock` (start/end/duration) and `QueueTiming` (wait/run/total) are Spans in disguise — model them with real `start`/`end` timestamps and *derive* duration, rather than storing pre-computed durations as the primitive.
- **Event** — an **append-only, point-in-time lifecycle fact with no duration**: `queued` / `started` / `progress` / `completed` / `failed` / `retry` / `dead`. **Events are the source of truth; a Span is derived from a matched pair of Events** (a `started` and a `completed`/`failed` sharing a span id). Today's `TraceEventRow`, `WorkflowQueueEventRow`, and `LiveActivityRow` are all Events — unify them into one event record type.
- **Metric** — a **derived counter / gauge / histogram, never a source of truth**. `calls`, `input_tokens`, `cost_micro_usd`, `p50/p95`, `queued/running/succeeded` gauges, `within_10/within_20` gold-rank buckets — all *reductions* over the event/span stream. Today's `ModelStat`, `QueuePressure`, `NumericMetricSummary`, `GoldRankDistribution` are metrics: correct as *views*, dangerous if they were ever the only place a fact lives.
- **Log** — free-text / semi-structured diagnostic output (an error string, a judge's raw output, a stack trace). For an eval tool a Log is just an Event whose payload is a message blob (`LiveErrorRow.message`, `judge_raw`). Do not build a separate log pipeline — make "log line" one payload variant of the event envelope.
- **Link** — the escape hatch from the strict parent tree. **This is the single most important OTel concept for an agnostic multi-prong tool, and the one the current schema cannot represent.** (Detailed below.)

**The decision rule — memorize this table:**

| If the thing… | model it as | primitive? |
|---|---|---|
| has a start and an end you'd draw as a bar | **Span** | derived from 2 events |
| is a point-in-time "state changed" fact | **Event** | **yes — source of truth** |
| is a count / sum / rate / quantile over other records | **Metric** | never — always rebuildable |
| is a human-readable message | **Log** | an Event payload variant |
| is a causal edge the parent tree can't express | **Link** | an Event/Span field |

**Current-guidance alignment (2026):** OpenTelemetry is deprecating the Span Events API and consolidating on "events are logs with names," correlated to a span by context. The practical lesson: **do not model lifecycle facts as attributes hanging off a span object.** Emit independent, timestamped event records that *reference* a span/trace id. This is exactly the event-sourcing stance below — and where OTel itself landed.

### 1.2 Why Links are non-negotiable

A strict parent tree assumes each span has exactly one parent. Agentic/hybrid/eval systems routinely violate this, and a parent tree *silently loses provenance* when they do:

- **Fan-out (one → many):** a router dispatches a query to N retrieval prongs in parallel; a planner spawns K parallel tool calls. Each child links back to the initiator (`TRIGGERED_BY`).
- **Fan-in / batch (many → one):** a single embed request batches items each belonging to a *different* question's trace; a rerank call consumes candidates gathered across several recall lanes. A span has *one* parent, so parent-child can encode *at most one* origin — **Links encode all of them** (`BATCH_MEMBER`, `AGGREGATES`). This is precisely the batched-embed case in this repo, and precisely where a parent tree loses the plot.
- **Async decouple (queue):** enqueue and dequeue live in different traces and times. OTel's own messaging conventions use *links, not parent-child*, to correlate producer↔consumer. Today's `WorkflowQueue` items crossing questions are exactly this.
- **Retry / compensation:** `RETRY_OF` a prior failed attempt, `FOLLOWS_FROM` a predecessor. Today's `attempt` / `max_attempt` / `retried_items` fields are begging for a `RETRY_OF` link.

A Link is cheap: `{ linked_trace_id, linked_span_id, relation_enum, attributes }`. Ship a `LinkRelation` enum (`TRIGGERED_BY`, `BATCH_MEMBER`, `AGGREGATES`, `FOLLOWS_FROM`, `RETRY_OF`, `COMPENSATES`) and you can render true DAG waterfalls (fan-out lanes, batch-collapse bars) for *any* system, not just memory. **This is the concrete generalization lever.**

### 1.3 The envelope: common header + typed payload via `oneof`

**Pick this pattern. Reject both alternatives:**

- ❌ **One giant flat row** (the failure mode the current view messages illustrate): dozens of `optional` fields where only a domain subset is ever set, `kind`/`operation`/`status` as bare strings. Doesn't generalize (every new prong = more nullable fields) and destroys type safety.
- ❌ **`google.protobuf.Struct` for everything** (the `report_json` / `roles_detail_json` / `answer_json` string-typed variant): unindexable, unversioned, ~2–3× the bytes of typed fields, all validation pushed to runtime.

The envelope — every append record is one `TraceRecord` with a small stable header + exactly one typed payload:

```proto
message TraceRecord {
  // ---- query-critical header: FIRST-CLASS, never in a Struct ----
  string   record_id      = 1;   // ULID/UUID — dedupe + ordering
  string   run_id         = 2;   // → Resource
  string   trace_id       = 3;   // the question / task
  string   span_id        = 4;   // set for span/event records
  string   parent_span_id = 5;
  google.protobuf.Timestamp ts = 6;  // authoritative event time
  RecordKind kind         = 7;   // enum, NOT string
  uint32   schema_version = 8;
  repeated Link links      = 9;   // the DAG edges from §1.2

  // ---- typed payload: exactly one ----
  oneof payload {
    SpanStart      span_start = 20;
    SpanEnd        span_end   = 21;
    LifecycleEvent lifecycle  = 22;  // queued/started/progress/failed…
    ModelCall      model_call = 23;  // tokens, cost, model id
    RetrievalStep  retrieval  = 24;  // candidates, scores, ranks
    ToolCall       tool_call  = 25;  // agent tool invocation
    LogLine        log        = 26;
    // new prong types append at NEW field numbers — old readers skip them
  }

  // ---- last resort only, clearly named as such ----
  google.protobuf.Struct ext = 100;  // experimental / unmodeled attrs
}
```

Why this wins for an agnostic tool: the **header is uniform across every system**, so the tailer, differ, and waterfall builder are domain-agnostic and never change; each **prong type gets a typed, first-class payload** you can query and validate. Adding "agent tool calls" or "GraphRAG hops" later = add one `oneof` arm at a new field number. Nothing else moves.

**Query-critical fields are first-class, full stop.** Anything you filter, sort, group, or join on — `run_id`, `trace_id`, `span_id`, `ts`, `kind`, model id, status, question type, rank — is a top-level typed field, never inside `ext` / Struct / a JSON string. **Rule of thumb: if a WHERE clause or a dashboard facet touches it, it is a column, not a blob.**

### 1.4 Versioning & evolution discipline (load-bearing)

- **Enums, not strings, for closed sets** (`kind`, `status`, `severity`, `transition`, `LinkRelation`, `final_status`). Reserve `0 = *_UNSPECIFIED`. Never renumber. Unknown enum values from a newer writer survive round-trip as their raw number in proto3, so an old reader degrades gracefully.
- **Field numbers are identity.** Never change or reuse a number. Deleting a field → `reserved <number>;` and `reserved "<name>";`.
- **Adding fields is always safe.** **`oneof` evolution is sharp:** adding an arm to an *existing* oneof is *forward-incompatible* (an old binary can't distinguish a new arm from "unset"). Safe moves: a single explicit-presence field → a *new* one-arm oneof; a one-arm oneof → a standalone field. **Practical rule: prefer a brand-new `oneof` arm (new payload type) over widening an existing payload's meaning.**
- **Never change a field's default, type, or semantics** across versions — silent version skew.
- **`optional` (explicit presence) wherever absent ≠ zero** — cost that might genuinely be `0`, a latency you haven't measured, a rank that could legitimately be `0`. The current schema already does this well; keep the discipline. It's the difference between "0ms because instant" and "no measurement."
- **`map<K,V>`** for genuine associative lookup with unique keys where order and per-entry metadata don't matter (`per_question_type`, `items_by_status`, `roles`). **`repeated Entry`** when you need ordering, duplicate keys, or per-entry fields — e.g. an ordered list of retrieval candidates each with score+rank+source is `repeated Candidate`, *never* a map.
- **Two versioning levels:** (a) **package version in the path** — `membench.trace.v1` → `v2` for breaking changes, both compiled in parallel during migration (you already do `dashboard.v1`); (b) **`schema_version` in the envelope** for within-v1 additive evolution. Bump the *package* only when you'd otherwise have to reuse or retype a field.

---

## 2. Wire & transport

### 2.1 Append stream: length-delimited protobuf — the typed JSONL replacement

Protobuf messages are not self-delimiting; a raw concatenation is unparseable. The standard fix: write a **varint length prefix** before each message, then the bytes; the reader loops `read varint → read N bytes → parse`. This yields a `.pb` append log that is the exact structural analog of a JSONL file (`writeDelimitedTo`/`parseDelimitedFrom`; `prost::encode_length_delimited`). One `TraceRecord` per frame. Benefits over JSONL: 3–10× smaller, typed, no float/precision drift, forward-compatible via unknown fields. **Keep the framing dumb** (varint length, no embedded index) so you can `tail -f`-equivalent it and truncate-recover a partial trailing frame.

### 2.2 Transport by access pattern (they are different problems — do not conflate)

- **(a) Live tailing → server-streaming RPC over Connect** (Connect-Web + protobuf-es in the browser). Connect gives a real `for await` server stream mapping cleanly onto the append log's frames, works natively in the browser without a translating proxy, and ships a compliant protobuf-es client with small bundles. This replaces today's poll-the-`LiveResponse` model with push. **SSE** is a legitimate lighter fallback (dead-simple infra, text frames) if a proxy/CDN forces it — but you lose typed binary framing and backpressure. Avoid raw WebSockets: you don't need client→server streaming for tailing.
- **(b) Large historical fetch → NOT a stream; a cursor-paged fetch, separated by record type.** A debugger loads *one lane at a time* (all rerank calls, then all gold-eval rows) over a possibly huge run. Formalize opaque-cursor pagination (`GET events?trace_id=…&kind=RETRIEVAL&cursor=…`; cursor = last `record_id` + type) and let the client request exactly the record types the current panel needs. Today's `ArtifactResponse{offset,limit,total}` and `TraceEventStream{truncated}` already lean this way — formalize it. **Never stream a 500-question run's full history through one server-stream; page it by type.**

### 2.3 When JSONL still beats protobuf

Be pragmatic, not dogmatic. JSONL/JSON wins for: (i) artifacts **human-grepped/`jq`'d by hand** during debugging; (ii) tiny volumes where tooling overhead isn't worth it; (iii) **interop** with external tools that expect JSON (LLM-judge dumps, ad-hoc notebooks); (iv) genuinely open/unknown field sets that would be all-Struct anyway.

**The stance for this tool:** protobuf for the **high-volume machine trace log** (embeds, spans, events, model calls — 10k+ records/run); JSONL/JSON for **leaf human-facing artifacts** (a single question's judge transcript, the final report). Don't binary-encode a 3-line judge dump.

### 2.4 Browser client

**Connect-Web with protobuf-es** — the only fully-compliant protobuf JS impl, tree-shakes well, speaks Connect/gRPC-Web/gRPC. This repo already generates the client under `dashboard/src/lib/gen/membench`, so the toolchain is in place; **point it at the generic trace protos too**, not only the view protos.

---

## 3. Materialized-artifact / event-sourcing pattern

### 3.1 Append-only raw facts are the single source of truth

The `TraceRecord` length-delimited log (per run, optionally sharded per record-type) is the ledger: immutable, ordered, replayable. Every derived thing — waterfalls, coverage, cost rollups, gold-rank distributions, the entire `LiveDetail` / `TracesResponse` / `GoldEvalResponse` family — is a **projection you can throw away and rebuild by replaying the log**. *State is a fold over events.* The payoff for a debugger is precisely the property you want: **re-derive a new view you didn't think of during the run (a rerank-score histogram, a new coverage cut) from old runs — zero re-execution, zero paid LLM calls.**

### 3.2 Every derived artifact carries a provenance header — mandatory, typed

```proto
message ArtifactProvenance {
  string schema                = 1;   // e.g. "membench.trace.waterfall"
  uint32 schema_version        = 2;
  string builder               = 3;   // materializer name + version/git_sha
  repeated SourceHash sources  = 4;   // {path, sha256, byte_len} per input log
  google.protobuf.Timestamp generated_at = 5;
  bool   complete              = 6;   // false = partial / streaming / truncated
  optional string invalidated_by = 7; // record_id/hash that superseded this
}
```

This is the concrete fix for the memory this repo already carries (source-hash staleness gates, the `SYMEM_IGNORE_SOURCE_HASH` escape hatch): make the source-hash gate a **first-class field on every materialized artifact**, not an ad-hoc side file. `complete=false` lets the UI honestly render "still building / truncated" (today's scattered `truncated` bools generalize into this). `invalidated_by` lets a newer materialization tombstone an older cached one. This directly addresses the logged `--answer-only` manifest-staleness pain: "why is this view stale?" becomes a checkable fact, not a vibe.

### 3.3 When to materialize — split by cost and shape

- **Online (during the run — cheap, incremental, O(1) per event, bounded memory):** counters, gauges, running histograms, last-N error rings. This is today's `LiveDetail` (`QueuePressure`, `ModelLive`, `StageProgress`, error categories). Compute in-process as events append; powers live tailing at ~zero cost. **Never let it be the only record** — it's a fast cache over the log.
- **Post-hoc (after the run — expensive, whole-stream, often a join):** waterfalls / dependency graphs (need global start/end + all spans), gold-coverage joins, cross-question DAGs, cross-run diffs (`CompareResult`), leaderboards. Build these **lazily on first request** and cache the result *with its provenance header*.

### 3.4 Cache invalidation is by source hash, not by clock

A materialized artifact is valid iff `sha256(current source logs) == provenance.sources[*].sha`. On request: hash the source frame log(s) → compare → serve cache on match, rebuild (stamp fresh provenance, set the old one's `invalidated_by`) on mismatch. Deterministic, survives restarts, no TTL guessing. **Never invalidate by TTL/clock.**

---

## 4. Competitive patterns from eval products

### 4.1 The canonical object graph everyone converges on

The observability tier (LangSmith, Langfuse, Braintrust, Phoenix, Weave, Opik) implements the same 5-level spine, with different names:

```
Project / Workspace
  └─ Dataset (versioned) ──► Example / Item / Datapoint   (input + gold/target)
  └─ Experiment / Run    ──► Run-item / Prediction        (one per Example)
        └─ Trace ──► Span / Observation   (tree of timed steps)
                        └─ Score / Feedback / Annotation  (attaches at ANY level)
```

The load-bearing insight: **an Experiment is a set of Traces (one per Dataset Example) plus the Scores over them.** Example = *frozen input+gold*; Trace = *what happened this time*; Score = *how good it was*. **This is exactly our `run → question → trace → verdict` graph.**

### 4.2 Convergent patterns to COPY (table stakes — don't reinvent)

1. **The 5-level spine**, with Score attachable at every level.
2. **Example is frozen input+gold; Run/Trace is the mutable execution; Score is the verdict — never conflate them.** The *join row* (Langfuse `DatasetRunItem`, HELM `PerInstanceStats`) is what carries per-example results. Adopt it: a `run_question` row carrying the verdict + evidence-coverage metrics, linking question ↔ its traces.
3. **Dataset versioning / content-hashing** so every result is provably tied to the exact test set + gold (LangSmith auto-version, Humanloop immutable hash). **We already have source-hash gating — this is the same instinct; formalize it.**
4. **Per-example table with an aggregate header** — the default eval view: one row per example; columns for output / gold / score(s) / cost / tokens / latency / status; an aggregate summary bar on top.
5. **Cost + tokens + latency measured at the leaf (span/generation) and rolled up** to trace → run. Everyone surfaces per-run cost as a first-class column, not a footnote.
6. **Trace = waterfall/tree of timed, typed spans**, leaves = model calls, drill-in shows span input/output/metadata/metrics.
7. **Comparison = diff-against-baseline** with improved/regressed/same classification + heatmap red-green encoding.
8. **LLM-judge reasoning shown inline** with the score (Braintrust CoT, LangSmith judge trace) — the feature people actually use.
9. **Statistical honesty:** stderr / repetitions / variance (Inspect stderr, LangSmith Repetition Summary). For small Q-sets this matters.

### 4.3 Differentiators to STEAL DELIBERATELY

| Axis | Best-in-class | The move |
|---|---|---|
| **Regression-diff UX** | **Braintrust** (baseline + improved/regressed/same + BTQL filter + inline judge CoT) | The gold standard for "diff two runs." **Copy wholesale.** |
| **Typed span data model** | **Langfuse** (generation/span/event/tool/retrieval) | Typed observations → render provider vs memory vs recall vs judge as *distinct span kinds*. |
| **Formal leaderboard object** | **Weave** (rows=models, cols=eval×scorer×metric), **HELM** (public) | Leaderboard as a first-class, *shareable* object — not just a sorted table. Our vault-store is its natural backing; a leaderboard is the vault's public face. |
| **Retrieval-evidence metrics** | **Ragas** (context precision/recall) + **Phoenix** (retriever spans expose docs+relevance) | Our axis. Nobody owns benchmark-grade evidence-coverage UI. |
| **Event-log transcript (not just spans)** | **Inspect** (chronological events: model/tool/score) | A memory recall pipeline reads better as an ordered event log than a nested span tree. |
| **Per-instance artifact separation** | **HELM** (PerInstanceStats separate from aggregate) | Store per-question verdicts as first-class artifacts; drill leaderboard-cell → question → trace. |
| **Dataframe-native comparison** | **Inspect** (evals_df / samples_df) | For a research tool, exportable per-question tables beat a locked-in UI. |

### 4.4 Where we WIN — the axis the market structurally cannot render

Spend the entire differentiation budget here. No competitor's objects are *evidence-piece-shaped*, so none can produce these views:

- **Evidence coverage is a first-class metric, not a score sub-field.** Build a **gold-evidence coverage matrix**: rows = gold evidence pieces for a question; columns = {retrieved? at what rank? survived rerank? in final context? cited by answer?}. This is our retrieval-anatomy (merge/collapse, the rank-31 leak past the ~30 context cut) *made visual* — and it is the thing that isolates retrieval-wall vs reader-wall. Ragas has context-precision/recall as *aggregate numbers*; Phoenix shows retrieved docs in a *span*; **neither shows per-question, per-evidence-piece coverage as the primary axis.**
- **Trace model = 4 typed lanes, not a generic span tree.** Render provider / memory / recall / judge as four distinct, color-coded lanes. The recall lane shows the **retrieve → dedup → rerank → context-cut** pipeline with evidence pieces flowing through and *dropping out* at each stage — visualizing "dedup-before-rerank is where the 57% dup mass is" and "gold leaks past the ~30 context cut."
- **Event-transcript over span-tree for the recall lane** (borrow Inspect): the recall lane is a *sequence of transformations on an evidence set* — it reads better as an ordered log ("32 facts retrieved → 18 after dedup → reranked, gold now p98≈43 → 30 kept, gold #31 dropped") than as a nested tree.
- **Question-type as a primary pivot** (we already compute hard/control + per-type; competitors bury it). The multi-session counting wall (79–87%) is our recurring story — make question-type a first-class facet in table *and* leaderboard.
- **Oracle/ceiling as a comparison mode.** Our "feed the reader only gold evidence" method has no analog in these tools. Add a run-comparison mode — *live run vs oracle run on the same questions* — where the diff column literally is "how much of the gap is denoise (retrieval) vs reader." This operationalizes the oracle-ceiling-first method as a UI primitive.

**Skeleton = Braintrust comparison UX + Langfuse typed data model + Weave/HELM leaderboard object. Differentiation = the evidence-coverage matrix + the 4-lane recall-pipeline drilldown.**

---

## 5. Persona-driven IA & UX

### 5.1 Three workspaces, not two — and the URL carries identity

The current dashboard already makes the two hardest IA calls correctly: a **two-mode workspace split** (Leaderboard / Debugger, `F1`/`F2`) rather than a flat 8-tab bar, and **ID-addressed master-detail drilldown** (`#/debug/<run-id>/<subscreen>`, run registry tree left, run-scoped tab bar right). Keep the spine; fix the seams. **Promote 2 modes to 3 named workspaces** so each persona has an unambiguous home:

| Workspace | Persona | Key | Landing | Owns |
|---|---|---|---|---|
| **LEADERBOARD** | (A) Evaluator / buyer | F1 | Ranked cohort table + quality-vs-cost | choose a system, prove robustness, export a report |
| **RUNS** (rename of "Debugger") | (B) Engineer | F2 | Registry tree → run → Overview | drill run→question→evidence→trace, taxonomy, diff-to-baseline |
| **LAB** (new; absorbs Live+Tuner+lineage) | (C) Operator | F3 | Active-runs board + launch/queue | launch/stop/pause/resume/retry, budget cap, live logs, lineage |

**Why split LAB out** rather than leave Live/Tuner as run-tabs: the operator's job is **fleet-level and future-tense** ("what should I launch, what's running, what will it cost, kill the bad one") — it is not naturally scoped to one already-selected run the way the engineer's tabs are. Burying "launch an experiment" three levels deep under `#/debug/<run>/tuner` is why it reads as "copy a script" instead of "run it."

**Invariants (mostly already true — keep them):**
- **The URL addresses state; the workspace is just a lens.** `#/runs/<id>/questions?verdict=wrong&qtype=multi-session` must be copy-pasteable and land a colleague exactly where you are. This is what makes three personas *one tool*: an evaluator pastes a run-id into Slack → the engineer opens it in RUNS → the operator re-launches it in LAB — *same object, three lenses.*
- **Cross-workspace deep links, not mode silos.** Every run-id anywhere is a link. Leaderboard row → RUNS/overview. "N in flight" → LAB. A wrong question → its trace.
- **Peer-nav is the three workspaces only.** The eight current screens are not peers — five are *tabs of a selected run*, one is *fleet control*. Don't promote them to top-level.

### 5.2 Master-detail drilldown: run → question → evidence → trace

The engineer's spine and the tool's most important flow. Reference model: LangSmith's run tree — a trace is a tree of runs, a waterfall reveals sequence and timing, each node expands to inputs/outputs. We already have `TraceWaterfall.svelte` / `TraceLog.svelte`; wire them into the drilldown so the four levels form *one continuous zoom.*

- **Level 1 (run) — persistent left master.** Registry tree pinned left (248px), always visible, so the engineer jumps runs without losing the workspace.
- **Level 2 (question) — master-detail inside the Questions tab.** List is master; selecting one opens the detail drawer (already built). **Fix: put `question_id` in the URL** (`.../questions/<qid>`) so a wrong question is directly linkable — the "here's the exact question that broke" handoff depends on it.
- **Level 3 (evidence/retrieval) — the drawer body.** Show **gold evidence vs retrieved evidence side by side with rank/score**, gold highlighted with a rank badge (the codebase thesis is "rerank rescues gold to p98" — make gold-rank visually obvious). This is Grafana's "aggregate → exact traces behind the anomaly" pivot, applied to retrieval.
- **Level 4 (prompts/provider → full trace) — expand in-drawer, then "open full trace."** Inline: answerer prompt, provider call, judge verdict. An **"open trace"** action deep-links to the full `TraceWaterfall`. Keep the drawer stacked *over* the master list so `Esc` returns the engineer to the filtered list exactly where they were.

**Drawer vs full-page rule:** drawer for the leaf you're inspecting (question → evidence → prompt) so the filtered list is preserved and `Esc`-dismissible; full-page route only when the artifact is itself a workspace (the full waterfall, a large prompt diff). **Never make the user lose their filter set to look at one row.**

### 5.3 There is no dead number — every tile is a clickable filtered drilldown

The single highest-leverage cockpit principle, and where today's Overview under-delivers. Every aggregate is a saved query over rows; clicking it lands you in the row list *pre-filtered to exactly the rows behind that number.*

- Overview "wrong: 42" → `#/runs/<id>/questions?verdict=wrong`
- A category-heatmap cell (`per_question_type`) → `?verdict=wrong&qtype=<type>`
- "abstentions: 7" → `?verdict=abstain`; "errors: 3" → `?verdict=error`
- Leaderboard cost/latency/qtype columns → the same drilldown scoped to that run.

**The one change that unlocks this *and* shareable links:** lift the Questions filter state (`verdict`, `qtype`, `debouncedSearch`) out of component-local `$state` and **into the router (URL params)**. Then tiles can deep-link into filters, and the filtered view is shareable across all three personas. **Mandatory return path:** show the active filter as a removable chip ("wrong ✕", "qtype: multi-session ✕") with the aggregate still visible above the list.

### 5.4 Failure taxonomy — group wrong answers by cause (Sentry's edge)

The engineer should not read 42 wrong questions linearly; they should see **buckets grouped by cause** — derivable from data we already have per question (was gold retrieved? at what rank? in-context? verdict?):

- **retrieval miss** — gold never retrieved (a *recall* problem)
- **rank miss** — gold retrieved but ranked past the context cut (the rank-31 leak; a *rerank* problem)
- **reader miss** — gold *in* context but answer wrong (the *reader wall* / counting failures)
- **abstention** and **judge/harness error**

Surface as (a) a **taxonomy bar on Overview** where each bucket is a drill tile (§5.3), and (b) a **group-by toggle in the Questions list**. This makes the three walls the memory files spent weeks isolating (retrieval-wall vs rank-wall vs reader-wall) a *first-class UI object* — one click instead of hand reconstruction.

### 5.5 Comparison / diff — two distinct jobs, two patterns (don't conflate)

- **5a. Evaluator: N-way leaderboard comparison.** Today's `Leaderboard.svelte` already does this well (select ≤4, color-key, auto-flip to comparison mode, metric matrix + radar). **Keep it.** Add a **quality-vs-cost scatter** as the buyer's headline chart (accuracy Y, cost X, Pareto frontier highlighted — data is on every `RankedRow`), and make the whole comparison a **shareable report** (§5.7).
- **5b. Engineer: 2-way per-question regression diff.** A *different* pattern. The data model is already built for it: `TrialMarker` carries `improvements / regressions / unchanged_wrong / unchanged_correct` against `compared_to_run_id`. Render the classic **4-quadrant diff**: fixed (wrong→right, green), regressed (right→wrong, red), still-wrong, still-right — each quadrant a drill list into those exact questions. Put a **"compare to baseline"** action on every run *and* on a single question's drawer ("show this question in run B"), so the diff is reachable fleet-wide and from one failing id.

### 5.6 Live monitoring + control — observe *and* act (Operator)

Today the tool is strong on **observe** (2s poll, queue segments, per-stage progress) and weak on **act** (Tuner only previews a script to copy). Close the loop in LAB:

- **Fleet board (landing):** all active+recent runs as cards with live queue mini-bars, each carrying **inline primary actions**: Stop / Pause / Resume / Retry-failed. Controls live *next to* the thing they act on.
- **Budget cap + cost estimate as a first-class control, not a footnote.** Every launch shows a **pre-run cost estimate** (`RunnerPreview` cost) and accepts a **budget cap** that auto-pauses when hit, with a live spend-vs-cap bar. Given each oracle/answerer run is a paid OpenRouter call (~$0.4–2.7), a hard budget cap is the operator's main safety control, not a nicety.
- **Live logs:** a streaming pane (extend `TraceLog.svelte`) with follow-tail + level filter.
- **Run lineage:** render `TrialMarker` (`compared_to_run_id`, `original_baseline_run_id`) + `tuning_cohort` as a small **DAG/thread** ("this run was launched from X, changing param Y").
- **Destructive-vs-reversible split:** Stop/kill → confirm (typed run-name or `⌘⏎`). Pause/resume/retry → immediate, optimistic, reversible.

### 5.7 Dense terminal vs guided/report mode — keep dense, add a report projection

**Rule: dense terminal cockpit is the native mode for the two internal repeat-user personas (Engineer, Operator); add a lightweight report projection for the one external, low-frequency persona (Evaluator).**

- **Engineer + Operator → dense stays.** They live in the tool daily, want maximal density + keyboard speed; the terminal aesthetic *signals* "expert instrument" and reduces chrome. **Progressive disclosure inside the dense view — not a separate "simple mode" — handles "don't overwhelm."**
- **Evaluator → add a Report/Share projection.** A CRT-green terminal table is wrong for a decision artifact pasted into a procurement doc. Add a **Report view** of Leaderboard/Compare: same data, calmer typography, self-explanatory labels (not `T·AVG`/`ABST`), the quality-vs-cost scatter, per-category robustness, **methodology/judge provenance** (`judge_model`, `judge_prompt_mode`, `dataset_fingerprint`, `oracle_gold` — the trust metadata a buyer needs), and an **export** (shareable URL + static HTML/PDF). This is the *one* place a guided mode earns its keep.

### 5.8 Command palette / keyboard-first shell

Upgrade the current `/` command *line* (type a verb, hit enter) to a real **palette** (`⌘K`; fuzzy list of objects + actions + recents you arrow through; keep `/` as quick run-jump). Contents fuzzy-ranked: runs by name/id, workspace destinations, and verbs ("compare A vs B", "launch experiment", "open trace for Q-1234", "filter questions: wrong + multi-session"). Two-column layout with a preview panel for the highlighted run/question. Keep the visible keyboard map in the status bar; extend with `⌘K`, `F3` (LAB), and `?` for a cheat-sheet. `j/k` list nav + `Enter` to drill + `Esc` to pop the drawer = full mouse-free traversal of the run→question→evidence→trace spine.

### 5.9 Migration is small and low-risk (the spine already exists)

1. Add `F3` LAB as a third workspace; move `Live`/`Tuner` there from the run tab bar.
2. **Lift Questions filter state into the router** — the single change that unlocks tile-drilldown (§5.3) *and* shareable links (§5.7) across all personas.
3. Add the failure-taxonomy derivation (§5.4) + the 2-way diff quadrants (§5.5b) — data already on `RunSummary` / `TrialMarker`.
4. Wire real launch/stop/budget actions behind the runner instead of copy-a-script.
5. Add the `⌘K` palette + the Report projection.

Everything else — modes, URL addressing, master-detail, command box, status bar, waterfall components — is kept.

---

## 6. DO / DON'T — tailored to membench

### DO

**Data model**
- **DO** introduce a *generic, append-only `TraceRecord` envelope* (`membench.trace.v1`) as source of truth, distinct from the memory-specific `dashboard.v1` **view** protos. The `debugger.proto` messages become materializations, not primitives.
- **DO** adopt OTel vocabulary as a *map*: duration-work → **Spans**; point-in-time state changes → **Events** (source-of-truth atom); aggregates → **Metrics** (always rebuildable); add a first-class **`Link`** type with a `LinkRelation` enum — Links are what make the tool agnostic to fan-out / fan-in / async DAGs.
- **DO** derive Spans from matched start/end **Events**; keep Events primitive (where OTel itself landed).
- **DO** use **common envelope + typed `oneof` payload**; add new prong types (agent tool calls, GraphRAG hops) as **new oneof arms at new field numbers**.
- **DO** keep every **query/filter/sort/facet** field **top-level and typed** (`run_id`, `trace_id`, `span_id`, `ts`, `kind`, model, status, rank).
- **DO** use **enums** for closed sets, reserving `0 = *_UNSPECIFIED`; use **`optional`** wherever absent ≠ zero (cost, latency, rank).

**Wire & storage**
- **DO** frame the append log as **length-delimited protobuf** (varint prefix per record) — the typed JSONL replacement; keep the framing dumb (tailable, truncate-recoverable).
- **DO** serve **live tailing via Connect server-streaming** (protobuf-es/Connect-Web) and **history via cursor-paged fetch separated by record type**.
- **DO** point the existing `dashboard/src/lib/gen` Connect client at the generic trace protos too.
- **DO** stamp **every materialized artifact with an `ArtifactProvenance` header** and **invalidate by source hash**; split materialization into **online** cheap counters and **post-hoc** waterfalls/graphs/coverage/diffs built lazily.

**Product & UX**
- **DO** map our objects to the canonical spine (`run`=Experiment, `question`=Example w/ gold as target, `verdict`=Score, four traces = typed spans on one Trace/question) and adopt a `run_question` **join row** carrying verdict + evidence-coverage.
- **DO** copy the table-stakes: per-question table + aggregate header, per-run cost as a first-class column, inline judge reasoning, dataset/gold content-hashing (formalize the source-hash gate), Braintrust-style baseline diff.
- **DO** spend the differentiation budget on the **gold-evidence coverage matrix** + the **4-lane (provider/memory/recall/judge) recall-pipeline drilldown** — the views the market structurally cannot render.
- **DO** make **question-type a primary pivot** and add an **oracle-vs-live comparison mode** (denoise gap vs reader gap).
- **DO** keep the 3-workspace spine (LEADERBOARD / RUNS / LAB) with **URL-addressed state**; make **every summary tile a filtered drilldown**; add a **failure-taxonomy** bucket view; give the operator **real launch/stop + a hard budget cap**; keep dense for engineer/operator and add a **calm Report projection** for the buyer.

### DON'T

**Data model**
- **DON'T** depend on the OTLP proto / SDK / collector — map the model, keep an adapter optional.
- **DON'T** ship the "one giant flat row" (the current view messages' dozens-of-optionals shape) as the *trace* model — it doesn't generalize and loses type safety.
- **DON'T** stuff query-critical data into `google.protobuf.Struct` or **JSON-in-string** fields (`report_json`, `roles_detail_json`, `answer_json`). Struct/JSON-string is a last-resort leaf-display escape hatch only, clearly named `ext`.
- **DON'T** use bare `string` for closed vocabularies (`kind`/`status`/`operation`/`severity` are enums-in-waiting).
- **DON'T** ever renumber/reuse a field number, change a default, or change a field's type/semantics; `reserved` deleted numbers *and* names.
- **DON'T** widen an *existing* `oneof` casually — adding an arm is forward-incompatible; prefer a new payload type at a new number.
- **DON'T** put `trace_id`/`span_id` as **metric attributes** (cardinality explosion) — keep correlation on events.
- **DON'T** let a **Metric be the only record** of a fact — aggregates are rebuildable projections.
- **DON'T** model lifecycle facts as attributes hanging off a span object (the deprecated Span-Events shape).

**Wire & storage**
- **DON'T** stream a full historical run through one server-stream — page it by type with an opaque cursor.
- **DON'T** binary-encode small human-grepped leaf artifacts (judge dumps, single-question transcripts) — JSONL/JSON wins there.
- **DON'T** invalidate caches by TTL/clock — invalidate by **source-file hash** in the provenance header.

**Product & UX**
- **DON'T** conflate Example (frozen input+gold), Trace (this execution), and Score (the verdict) — keep the join row distinct.
- **DON'T** conflate the two comparison jobs — N-way leaderboard (buyer) and 2-way regression quadrants (engineer) are different patterns.
- **DON'T** flatten the eight current screens into top-level peers — five are run-tabs, one is fleet control; peer-nav is the three workspaces only.
- **DON'T** leave a **dead number** on Overview — every aggregate must deep-link to its filtered rows.
- **DON'T** keep Questions filter state component-local — lift it into the URL (the change that unlocks tile-drilldown *and* shareable links).
- **DON'T** ship "launch an experiment" as copy-a-script buried under a run tab — give the operator a real fleet-level act surface with a budget cap.
- **DON'T** dilute the dense terminal cockpit for the internal personas to serve the buyer — solve that with a *separate* Report projection, not a watered-down main view.
- **DON'T** try to out-observability the observability SaaS on generic traces — win on the one axis they can't touch: **per-evidence-piece coverage + the recall-pipeline drop-out view.**
