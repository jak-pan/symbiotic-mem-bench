# Adapter-Agnostic Core & Composable Tunability

**A membench architecture brief — from "the symbiotic-memory harness" to "a memory-system benchmarking platform"**

## Thesis

membench must stop being *a driver for symbiotic-memory* and become *a host that renders and observes any memory adapter from a typed manifest*. The move that makes this possible is the one every durable plugin system (LSP, VS Code, K8s CRDs, OpenTelemetry, Backstage) has already made: **the adapter ships a typed manifest declaring what it can do and what it emits; the host hardcodes the envelope and the renderers, never the content, and ignores what it doesn't understand.** membench already does this in two places — the Tuner's `RunnerSchema`/`ParamField` and the Traces waterfall's `kind` + well-known-fields envelope. This brief promotes that implicit contract to a first-class `AdapterCapabilityManifest`, extends it to streaming/Systems/Compare, and defines the generic-but-typed trace/step model, the composable tuning layer, and the shared queue/model-adapter infra that all adapters plug into rather than reimplement.

---

## (1) The problem today — what couples membench to symbiotic-memory

There is **no `MemoryAdapter` trait** (`grep -rn "trait.*Adapter" src/` → zero hits). "The adapter" is a single 4285-line module, `src/symbiotic_memory_adapter.rs`, statically compiled against the `symbiotic_memory` crate's concrete types (`RecallEngine`, `IngestPipeline`, `MemoryStore`, `MemoryRunManifest`, `MemoryStage`, `MemoryTraceSink`). The coupling, ranked by how much it blocks a second adapter (mem0):

1. **The trace emitter/reader split.** `src/lib.rs` has a clean, neutral **reader** vocabulary — `MemoryTraceEvent`, `BenchQueueEvent`, `TraceEventRow` — whose round-trip test already uses `source_system: "mem0"` (`lib.rs:495`). But every actual **event** is minted by `symbiotic_memory::trace::MemoryTraceEvent::native_stage(...)` and the memory crate's own internal instrumentation via an injected `dyn MemoryTraceSink`. A second adapter can reuse the reader but **not the emitter**, and the two enums (lib.rs's neutral one and the memory crate's) must be kept in lock-step by hand.

2. **The manifest/stage state machine — the single biggest non-portable mechanism.** Resumability, redo-stage invalidation, answer-only reuse, and the source-hash gate are all built on `MemoryRunManifest` keyed by a fixed `MemoryStage` set (`Capture, DistillWindow, WriteArchive, EmbedRaw, EmbedFacts, Consolidate, Index, Answer`). Any adapter that wants membench's resume/answer-only/redo semantics must reimplement it against the symbiotic manifest type.

3. **`ProviderRuntime` + config coupling** (`membench.rs:4222-4870`). Providers, provider queue, pricing, thinking-mode, and role bindings (`DISTILL/CONSOLIDATE/QUERY_PLANNER/ANSWER/EMBED/JUDGE`) are all symbiotic-typed: `MemoryConfig::load_yaml`, `ProviderAdapterConfig`, `QueueRegistry`, `config.queue.resolve_provider_queue(&adapter)` → `Queued{Chat,Embedding,Reranker}`. mem0 gets **nothing reusable** from the provider queue — retry/rpm/lease all live in `symbiotic_memory`/`symbiotic_foundation`, not a shared membench crate.

4. **The knob surface has no schema boundary.** The tuner's ~54 knobs are the *union* of: 23 dashboard `ParamField`s (`runner.rs:336`) + ~90 `SYMEM_*` env reads + the `MemoryConfig` YAML `recall/queue/providers` blocks. There is no single settings-schema object, and the server's `runner_schema` handler hardcodes `"system":"symbiotic-memory"` (`membench-server.rs:3128`). `symem_param_schema()` is hardcoded and symbiotic-only.

The **de-facto contract** a run satisfies today (consume `&[LongMemEvalRecord]` + factory closures → drive ingest→recall→answer → emit two trace streams → write `answer.json`/`question-debug.json`/`memory-traces.jsonl` → declare capabilities via `BenchSupportedCapabilities` → expose knobs via `ParamField`) is real and reasonable — it is just **implicit, scattered, and typed against one crate**. The whole of this brief is: make that contract explicit, typed, and adapter-served.

---

## (2) `AdapterCapabilityManifest` — the typed contract a new adapter declares

One document, served from one endpoint (`GET /adapters/{id}/manifest`), that the host renders and observes purely from. Design rules, imported from the plugin-systems that work:

- **Handshake, then render** (LSP `initialize`, CRD registration): the host asks once, gets a typed answer, lights up only what's declared.
- **Ignore-what-you-don't-understand is *specified***, not accidental (LSP + Backstage mandate it) — this is what makes the manifest forward-compatible.
- **Reserved namespaces** separate host-owned from adapter-owned keys: `membench.*` reserved for the host; `x-<adapter>.*` for adapter extensions (mirrors `backstage.io/*` + OpenAPI `x-`).
- **View hints travel *with* the data but are separable** (CRD `additionalPrinterColumns` vs structural schema; JSON Forms `schema` vs `uiSchema`): the schema says *what*, the hints say *how*.

### Protobuf sketch

```proto
// membench.adapter.v1
message AdapterCapabilityManifest {
  string id = 1;                 // "symbiotic-memory" | "mem0" | "zep"
  string version = 2;
  string display_name = 3;
  MemoryParadigm paradigm = 4;   // enum below — drives Systems/Compare grouping

  Capabilities        capabilities   = 10;  // optional features (host gates tabs on these)
  KnobSchema          knobs          = 11;   // the "model card of settings" → Tuner
  repeated Preset     presets        = 12;   // adapter-shipped named diffs (vault presets)
  repeated Profile    profiles       = 13;   // layered base diffs
  StepTaxonomy        steps          = 14;   // pipeline step vocabulary → Traces
  repeated TraceStream trace_streams = 15;   // typed trace families the adapter emits
  repeated ModelRole  model_roles    = 16;   // model-adapter/queue bindings
  ViewHints           view           = 17;   // Systems/Compare printer-columns + ordering
}

enum MemoryParadigm {
  EXTRACT_STORE     = 0;  // symbiotic-memory, mem0
  RAG_HYBRID        = 1;  // symbiotic-memory (also), HippoRAG, GraphRAG
  TEMPORAL_GRAPH    = 2;  // Zep/Graphiti
  AGENTIC_SELFEDIT  = 3;  // Letta/MemGPT
  TIERED_OS         = 4;  // MemoryOS
  NEURAL_PARAMETRIC = 5;  // Titans/hymem
}

// ── CAPABILITIES: host renders only what's declared (LSP/CRD guarantee) ──
message Capabilities {
  bool live_traces          = 1;  // Live tab lights up
  bool dependency_waterfall = 2;
  bool provider_queues      = 3;  // provider-queue lane + cost analytics
  bool graph_traces         = 4;  // graph subgraph views (Zep/Cognee/mem0-graph)
  bool memory_ledger        = 5;  // ADD/UPDATE/DELETE/NOOP write-log view (mem0/Cognee/Letta)
  bool gold_eval            = 6;
  bool oracle_gold          = 7;
  bool resume_answer_only   = 8;  // manifest-backed resume / answer-only reuse
  bool spawn_run            = 9;  // Tuner "SPAWN RUN" button gates on this
}

// ── KNOB SCHEMA: JSON-Schema-lite, = today's ParamField + composition fields ──
message KnobSchema { repeated Knob knobs = 1; }
message Knob {
  string name = 1; string label = 2;
  KnobKind kind = 3;              // PATH|INT|BOOL|ENUM|STRING|FLOAT  (= ParamField.kind)
  string default = 4;
  repeated string options = 5;    // enum members
  repeated string observed = 6;   // live values seen in the run registry (host enriches)
  string group = 7;               // proto-UISchema: field grouping
  string help = 8; bool required = 9;
  Rule depends_on = 10;           // if/then/else visibility (rjsf dependencies)
  string unit = 11; string format = 12;    // view hint (ms, tokens, $, %)
  string knob_class = 13;         // "model" | "retrieval" | "dedup" | "temporal" | "orchestration"
}
message Rule { string when_knob = 1; repeated string when_in = 2; }  // show iff when_knob ∈ when_in

message Preset  { string name = 1; string description = 2; map<string,string> diff = 3; }
message Profile { string name = 1; map<string,string> base_diff = 2; }

// ── STEP TAXONOMY: declares the trace vocabulary the host will see ──
message StepTaxonomy { repeated StepKind kinds = 1; }
message StepKind {
  string kind = 1;                // canonical id from §6 taxonomy, e.g. "distill" | "retrieve_dense"
  string lane = 2;                // which waterfall lane it belongs in
  repeated PayloadField payload = 3;   // typed payload meaningful for THIS kind
  string color = 4; string legend_label = 5;   // view hint → waterfall legend (no host code)
  string semantic = 6;            // optional tag → host renderer-registry lookup (rerank_ladder…)
  bool is_gate = 7;               // support-check / ontology-validate → dropped-items affordance
}
message PayloadField { string name = 1; string type = 2; string unit = 3; bool well_known = 4; }

// ── TRACE-STREAM TYPES: the streaming envelope + which families flow ──
message TraceStream {
  TraceFamily family = 1;         // MEMORY|PROVIDER|RECALL|GRAPH|JUDGE|RUNNER  (§3, §6)
  string envelope = 2;            // "membench.step/v1"
  repeated string messages = 3;   // "StepStart","StepUpdate","StepEnd","QueueSample"
  bool supports_links = 4;        // fan-out DAG via links[]
  string order = 5;               // "unordered" → reconcile by id+timestamp, not arrival
  string jsonl_path = 6;          // where it lands under the run tree
}
enum TraceFamily { MEMORY=0; PROVIDER=1; RECALL=2; GRAPH=3; JUDGE=4; RUNNER=5; }

// ── MODEL-ADAPTER / QUEUE BINDINGS: shared infra, not per-adapter (§5) ──
message ModelRole {
  string role = 1;                // "extractor"|"updater"|"consolidator"|"reader"|"embedder"|"reranker"|"judge"
  CallKind call_kind = 2;         // CHAT | EMBED | RERANK
  string default_queue_id = 3;    // binds into the shared QueueRegistry
  bool required = 4;              // adapter can null out roles it doesn't use
}
enum CallKind { CHAT=0; EMBED=1; RERANK=2; }

// ── VIEW HINTS: Systems/Compare cards (CRD additionalPrinterColumns) ──
message ViewHints {
  repeated PrinterColumn systems_cards = 1;   // {label, jsonpath, unit, format, priority}
  repeated string default_group_order = 2;
  repeated string compare_dimensions = 3;     // knob names that define the ablation axes
}
message PrinterColumn { string label=1; string jsonpath=2; string unit=3; string format=4; int32 priority=5; }
```

The equivalent JSON-Schema of the *knob* half is what the Tuner consumes; `knobs` is a flat list of typed fields with a `group` (UISchema-lite) and a `depends_on` (if/then/else), exactly generalizing today's `ParamField`. Everything a new adapter must supply now lives in **one document**; nothing is hardcoded host-side.

---

## (3) Generic-but-typed trace / step model

OpenTelemetry is the reference and membench already half-implements it: `TraceEventRow` (`types.ts:418`) is a solid common envelope, and the waterfall block `kind` (`types.ts:443`) is an open discriminated union (`… | string`) whose components already degrade gracefully (`blockClass` just uses the kind as a CSS class). Formalize it into **common envelope + typed payload keyed by `kind` + open extension bag + `links[]` DAG**, streamed as incremental messages over the *same* envelope.

### Message sketch

```proto
// membench.step.v1 — ONE envelope for static traces AND live streaming
message StepEnvelope {
  // ── common, host-rendered generically for EVERY adapter ──
  string step_id     = 1;   // stable id — place by this, never by arrival order
  string run_id      = 2;
  string namespace   = 3;   // user_id/session_id/group_id correlation
  TraceFamily family = 4;   // memory|provider|recall|graph|judge|runner
  string kind        = 5;   // step taxonomy id; unknown → neutral default lane
  string lane        = 6;
  google.protobuf.Timestamp ts_start = 7;
  google.protobuf.Timestamp ts_end   = 8;
  Status status      = 9;   // OK | FAILED | PENDING | RUNNING
  int32 attempt      = 10;
  int64 duration_ms  = 11; int64 wait_ms = 12; int64 run_ms = 13;  // provider timing (measure-don't-reason)
  int64 item_count   = 14; string item_unit = 15;                  // memory batch sizing

  // ── DAG: parent isn't enough; fan-out/fan-in needs links ──
  string parent_id = 20;
  repeated Link links = 21;         // gather-step points at all its scatter-steps

  // ── TYPED PAYLOAD, keyed by `kind` (discriminated union) ──
  oneof payload {
    MemoryOp   memory   = 30;  // ADD|UPDATE|DELETE|NOOP|MERGE|SUPERSEDE|PROMOTE|EVICT|FORGET
    ProviderCall provider = 31;  // provider,model,in/out_tokens,batch_size,charge_unit,queue/throttle/http_ms,retries,cost
    RecallEvent recall  = 32;  // corpora[], candidates[{id,corpus,mode,raw_score,rank}], fusion, rerank[], gate{dropped,kept}, gold_rank
    GraphEvent  graph   = 33;  // extracted{nodes,edges}, resolution[], invalidations[], traversal{seed,hops}
    JudgeEvent  judge   = 34;  // question,answer,gold,support{claim,supporting_ids,score}, verdict, question_type
    RunnerEvent runner  = 35;  // dataset,n_items,concurrency{configured,observed}, stage_timings, totals, config_hash
  }

  // ── EXTENSION BAG: anything only one adapter/kind needs ──
  map<string,string> ext = 40;   // keys namespaced x-<adapter>.*; host ignores unknown
}

message Link   { string ref = 1; string rel = 2; }   // rel: "scatter"|"gather"|"supersedes"|"derived_from"
enum   Status  { OK=0; FAILED=1; PENDING=2; RUNNING=3; }

// STREAMING = the same envelope, incrementally. Host reduces into the static lane/block model.
message StepMessage {
  oneof m {
    StepEnvelope start  = 1;   // ts_end unset
    StepEnvelope update = 2;   // partial — merge by step_id
    StepEnvelope end    = 3;   // final envelope
    QueueSample  sample = 4;   // periodic queue depth / in-flight snapshot
  }
}
```

**Rendering rules (host-side, adapter-independent):**
- The **envelope renders generically** — timeline, waterfall, and step-graph all come from `{step_id, lane, ts_start, ts_end, status, links[]}`. The three existing shapes (trace waterfall, dependency waterfall, bottleneck bars) already normalize to `WfLane{id,label,chipKind,blocks[]}`/`WfBlock` (`Traces.svelte:119-137`); every adapter now feeds that same shape.
- The **payload surfaces conditionally** — `kind` (via the manifest's `StepKind.payload`) declares which typed fields are meaningful; the host shows those, nothing more.
- **Well-known kinds get styled lanes/legends from the manifest; unknown kinds render in a neutral default lane** (the CRD/OTel graceful-degradation guarantee).
- **Order-independence:** stream is `unordered`; place by `step_id` + timestamp, reconcile append-and-merge, never append-by-arrival. (`TraceEventRow` is already timestamp-keyed.)
- **Fan-out draws from `links[]`** — batched embed / re-embed and provider-queue scatter/gather point a gather step at all its scatter steps, so the waterfall draws fan-out with no bespoke layout.
- One `semantic:"rerank_ladder"` tag opts a step into a **registered custom renderer** (JSON Forms testers pattern); otherwise the generic renderer wins.

This single envelope unifies `Live.svelte`'s reducer with the static Traces view: **one renderer, two feeds.**

---

## (4) Composable tuning — knobs, layered profiles, ablation matrices

membench's workflow is inherently ablation-driven ("stack of knob-diffs"). Model it explicitly instead of as flat values. `ParamField` (`runner.rs:336`) is already a hand-rolled JSON-Schema-lite: `group` is proto-UISchema, `observed` is live-value enrichment, `kind` maps to a widget — the right architecture, missing only the **composition layer**.

**Config = a layered stack of sparse diffs** (Kustomize / VS Code settings tiers):

```
adapter defaults  (manifest Knob.default)
   ▼ overlaid by
profile           (manifest Profile.base_diff — e.g. "longmemeval-raw-light")
   ▼ overlaid by
preset            (manifest Preset.diff       — e.g. "factconsol-thinkon-500")
   ▼ overlaid by
run overrides     (Tuner edits)
   =  EFFECTIVE VALUE  (+ provenance: which layer set it)
```

Each layer is a sparse diff over the manifest schema. The Tuner shows the **effective value plus its provenance layer** — mirroring how `observed` already distinguishes declared-vs-seen. Presets and profiles are **first-class manifest data shipped by the adapter**, not hardcoded (your `factconsol-thinkon-500` vault key *is* a named diff already).

**The ablation matrix is the Cartesian (or hand-picked) product of selected knob-diffs.** The manifest's knob schema *is* the "model card of settings"; the Tuner iterates `view.compare_dimensions` to enumerate a matrix and previews the exact command per cell. `RunnerPreview` already turns values → exact shell — **the matrix is N previews**, each dispatched through the existing `runnerPlan`. Each cell carries its `config_hash`, so Compare/Leaderboard can group results by the diff that produced them.

**Validation is declarative.** `RunnerPreview.warnings` is your `if/then/else` validator today (inputs-present check). Push it into the schema: `required`, enum membership, and `depends_on` rules let the host **gray out invalid combos before planning** and prune impossible matrix cells.

**How the Tuner renders generically (unchanged loop, new inputs):** iterate `knobs` → `group` → typed widget by `kind` (zero per-field code, as `Tuner.svelte` does now); apply `depends_on` for conditional visibility; apply a `Preset.diff` to seed; overlay layers to compute effective value + provenance; take the Cartesian product over `compare_dimensions` to build the matrix; call `runnerPlan` per cell. `capabilities.spawn_run` gates the button. **No per-adapter Tuner code, ever.**

---

## (5) Generic step / queue model — shared infra, not per-adapter

Today the provider queue (`resolve_provider_queue`, `QueueRegistry`, `Queued{Chat,Embedding,Reranker}`, retry/rpm/lease) and the workflow queue (`symbiotic_queue::SqliteQueue`, per-question concurrency/heartbeat/lease) live inside `symbiotic_memory`/`symbiotic_foundation` and are invoked inline in the symbiotic run function. A second adapter gets nothing. **Extract both into a shared `membench-core` crate**; the manifest binds into them declaratively.

**Two queues, both host-owned:**

1. **Workflow queue** (per-item concurrency, `buffer_unordered(cap)`, lease/heartbeat/resume). Already near-generic — lift it out of the symbiotic run function into `membench-core`. Every adapter drives one item at a time through its own pipeline; the host owns fan-out, concurrency, resumability, and the `RunnerEvent` trace. Resume/answer-only/redo semantics move here too, backed by a **generic run-manifest** keyed by the manifest's `StepKind` set (not symbiotic `MemoryStage`) — this dissolves coupling #2 from §1.

2. **Provider queue / model-adapter layer** (rate-limit, retry, backoff, rpm/tpm buckets, cost). This is a *cross-cutting orchestration concern nearly every other memory system delegates to the raw SDK* — which is exactly why it must be shared membench infra, and exactly why the `PROVIDER` and `RUNNER` trace families exist. An adapter declares `ModelRole`s (role → `CallKind` → `default_queue_id`); the host wraps every model call the adapter makes in a `Queued{Chat,Embed,Rerank}` provider bound to the shared `QueueRegistry`. The adapter never touches queueing; it just asks the host for a chat/embed/rerank handle for a role.

**Both queues emit the neutral trace families** (`PROVIDER`, `RUNNER`) that `BenchQueueEvent`/`summarize_queue_timing()` already read. The `charge_unit` field on `ProviderCall` is load-bearing: it makes the per-text rpm-overcharge bug class visible in the trace, and `throttle_wait_ms`/`queue_wait_ms` are the "measure-don't-reason" fields the cost/analytics code consumes.

Net: queueing and model-adapters become **shared infrastructure any adapter inherits**, not code every adapter duplicates. A mem0 adapter written against `membench-core` gets the workflow queue, the provider queue, resumability, and cost analytics for free — it only supplies its manifest and a "run one item" implementation.

---

## (6) Concrete generic step taxonomy (covers symbiotic-memory AND mem0 / graph-memory)

A **16-step superset**. No system implements all; each declares its subset in `StepTaxonomy`. These are the canonical `StepKind.kind` ids.

| # | `kind` | What it does | Systems |
|---|--------|--------------|---------|
| 0 | `capture` | accept turns/docs/events; normalize; assign ids/ts/namespace | ALL (symem: capture turns) |
| 1 | `segment` | split into episodes / chunks / FIFO pages | Cognee, MemoryOS, Redis/pgvector, RAG |
| 2 | `distill` | LLM → atomic facts / entities / summaries | mem0, Zep, Cognee, LangMem, symem |
| 3 | `classify_memtype` | route to semantic/episodic/procedural/working | LangMem, Letta, MemoryOS |
| 4 | `graph_extract` | build nodes/edges | Zep, Cognee, mem0-graph, Memary, RAG-graph |
| 5 | `dedup_resolve` | candidate vs existing → ADD/UPDATE/DELETE/NOOP; merge nodes | mem0, Zep, Cognee, LangMem, symem (fact consolidation) |
| 6 | `consolidate` | merge related, resolve contradictions, roll-up summaries | LangMem, MemoryOS, Zep(community), Letta(sleeptime) |
| 7 | `temporal_validity` | validity intervals; invalidate on conflict | Zep (bi-temporal), symem (knowledge-update) |
| 8 | `decay_evict` | heat / recency / surprise forgetting; tier promotion | MemoryOS, Titans, Memary |
| 9 | `embed` | vectorize facts/nodes/chunks | ALL vector-based (symem: facts + raw turns) |
| 10 | `store_index` | write vector/graph/relational/KV; build ANN index | ALL (symem: dual store) |
| 11 | `retrieve_dense` | semantic/cosine ANN | ALL vector-based |
| 12 | `retrieve_sparse` | BM25 / full-text / keyword | mem0-v3, Zep, symem |
| 13 | `retrieve_graph` | BFS / PageRank / entity-rank | Zep, HippoRAG, Memary, mem0-graph |
| 14 | `rerank_fuse` | RRF/MMR/cross-encoder/node-distance/episode-mentions | Zep, mem0(opt), RAG, symem |
| 15 | `support_gate` | drop unsupported; provenance/ontology gate; threshold cut | symem (support-check), Cognee(ontology), agentic-RAG(reflect) |
| 16 | `assemble_answer` | build context; generate with reader LLM | ALL that answer (symem: assemble → answer) |

**symbiotic-memory's line-up** = steps `0, 2, 5, 9, 10, 11, 12, 14, 15, 16` + the provider-queue cross-cut. Its distinctive slots the field mostly lacks: **dual-corpus retrieval over facts AND raw turns in one hybrid pass** (`RecallEvent.corpora = [facts, raw_turns]`), and an explicit **`support_gate`** (step 15) that only Cognee's ontology-validate and agentic-RAG's reflection loop parallel.

**mem0** = `0, 2, 5, 9, 10, 11, 12, 16` (+ `4, 5(entity)` in its graph variant), its ADD/UPDATE/DELETE/NOOP surfacing as `dedup_resolve` `MemoryOp`s in the `MEMORY` trace family. **Zep** = `0, 2, 4, 5(entity-resolve), 7(bi-temporal), 10, 11, 12, 13, 14, 16` with rich `GRAPH` traces. The taxonomy is a superset that both the extract-store spine and the temporal-graph spine map onto without either bending — and Titans (`memory-as-parameters`, no discrete search/rerank) is the deliberate boundary case: it declares only `capture` + a parametric `store_index`, proving the schema doesn't *assume* a retrieve/rerank stage exists.

**Two cross-cutting concerns sit beside the pipeline, not inside it:** working/hot-context management (Letta blocks, MemoryOS tiers) and provider orchestration (the queue). The manifest models these as `Capabilities` flags + the `PROVIDER`/`RUNNER` trace families, never as pipeline steps.

---

## (7) DO / DON'T + what each UI surface must render from the manifest

### DO

- **DO make the adapter ship one typed manifest; the host renders/observes purely from it.** Envelope and renderers are hardcoded; content never is.
- **DO specify "ignore what you don't understand" as a rule** — unknown `kind` → neutral lane; unknown `ext.*` key → stored, not rejected; undeclared capability → tab stays dark. Forward-compatibility depends on this.
- **DO reserve namespaces:** `membench.*` host-owned, `x-<adapter>.*` adapter-owned. Bake the ignore-unknown rule into every consumer.
- **DO keep *what* (schema) and *how* (view hints) as separable layers** — CRD structural-schema vs printer-columns; JSON Forms schema vs uiSchema.
- **DO extract the workflow queue, provider queue, resumability, and cost analytics into shared `membench-core`.** Every adapter inherits them; none reimplements them.
- **DO model config as a layered stack of sparse diffs with provenance**, and the ablation matrix as the Cartesian product of knob-diffs → N `runnerPlan` previews.
- **DO unify the live and static trace feeds on one `StepEnvelope`** — one renderer, two feeds.
- **DO use `links[]` (a DAG), not just `parent_id`**, so fan-out/fan-in (batched embed, queue scatter/gather) draws without bespoke layout.
- **DO reach for the renderer-registry + `semantic` tag only where a generic view underdelivers** (rerank ladder, gold-coverage) — additive registrations, never host forks.

### DON'T

- **DON'T hardcode `"system":"symbiotic-memory"`** anywhere (today: `membench-server.rs:3128`). The system is a manifest field.
- **DON'T hand-maintain two parallel enums** (lib.rs reader vs `symbiotic_memory` emitter). One neutral `StepEnvelope`; adapters emit it directly.
- **DON'T let the manifest/stage state machine stay typed against `MemoryStage`.** Generic run-manifest keyed by the manifest's `StepKind` set.
- **DON'T promote a field to the well-known envelope unless it's cross-cutting** (OTel semantic-convention rule). One-adapter/one-kind fields live in the typed payload or `ext` bag.
- **DON'T let any adapter touch queueing directly** — it declares `ModelRole`s and asks the host for role handles.
- **DON'T assume a retrieve/rerank/answer stage exists** — Titans/neural-parametric is a valid adapter with only `capture` + parametric `store_index`.
- **DON'T build per-adapter UI components.** If a new adapter needs host code to render, the manifest is missing a field — fix the manifest, not the host.

### What each UI surface renders from the manifest

| Surface | Renders from | Status | Behavior |
|---|---|---|---|
| **Tuner** | `knobs` (→ groups → typed widgets), `depends_on`, `presets`, `profiles`, layered provenance, `view.compare_dimensions` | 90% there (`Tuner.svelte`) | Iterate knobs zero-code; apply preset diffs; overlay layers → effective value + provenance; Cartesian product → ablation matrix → N `runnerPlan` previews; `capabilities.spawn_run` gates the button |
| **Traces** | `steps` (lane/color/legend/`semantic`), `trace_streams`, `StepEnvelope` + `links[]` | 70% there (`Traces.svelte:119`) | Keep `WfLane`/`WfBlock` normalization; drive lanes/colors/legend from `StepKind` view hints (delete hardcoded `.bkey` maps); unknown kind → neutral lane; `links[]` → fan-out edges; `is_gate` → dropped-items affordance; `semantic` → registered renderer |
| **Systems** | `view.systems_cards` (`{label, jsonpath, unit, format, priority}`), `paradigm`, `capabilities` | New, cheap | Generic key/value/unit grid straight from `jsonpath` into the run report (CRD printer-columns). Any manifest-filling adapter gets a Systems view with zero host code |
| **Compare / Leaderboard** | `view.compare_dimensions`, `config_hash` per cell, `judge`+`runner` trace totals, `paradigm` | Extend existing | Group runs by the knob-diff that produced them; align axes on the shared taxonomy so cross-adapter rows (symem vs mem0 vs Zep on LongMemEval) sit in one table; render `paradigm` as a facet |
| **Live** | `trace_streams` + `StepMessage` reducer | Unify | Consume the *same* `StepEnvelope` as static Traces — one renderer, two feeds |

### Migration path (low-risk, incremental)

1. Wrap `symem_param_schema()` + the trace DTOs in an `AdapterCapabilityManifest` served from `GET /adapters/{id}/manifest`; symbiotic-memory is manifest #1. **Nothing visual changes.**
2. Move Traces legend/colors and Tuner groups to read the manifest's view hints; delete the hardcoded `.bkey` mappings and the `"system":"symbiotic-memory"` literal.
3. Add `depends_on` + `presets` + `layer_provenance` → Tuner gains conditional fields, preset diffs, provenance, and the ablation matrix.
4. Extract the workflow + provider queues, resumability, and cost analytics into `membench-core`; introduce the generic run-manifest keyed by `StepKind`.
5. Add `links[]` + `StepStart/Update/End` streaming → fan-out draws, and Live/static unify on one envelope.
6. Write the **mem0 adapter against `membench-core`**: it supplies a manifest (paradigm `EXTRACT_STORE`, steps `0,2,5,9,10,11,12,16`, `MEMORY`+`RECALL`+`RUNNER` streams, its own knob schema and presets) and a "run one item" impl — and inherits Tuner, Traces, Systems, Compare, queueing, and cost analytics **with no host change**. That is the open-closed guarantee LSP, VS Code, CRDs, and Backstage all rely on, and the proof the core is finally adapter-agnostic.

**Net:** membench keeps its distinctive, purpose-built look for the memory domain it cares about, while any new memory adapter that fills the manifest gets the full platform for free.
