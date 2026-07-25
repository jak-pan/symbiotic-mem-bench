# membench v2 — Product & Data-Model Redesign (capstone)

> The decisive synthesis. The four companion docs carry the evidence and the long-form
> reasoning; this one commits to a design. Where they explore, this decides.

## 0. Thesis

membench is not "a dashboard for symbiotic-memory." It is a **benchmark operating system for
memory systems**: plug in any adapter, run any benchmark, and get — with zero per-adapter,
per-benchmark UI code — tracing, queueing, composable tuning, evidence-coverage diagnostics,
regression diffs, a leaderboard, and live control.

Two facts from the ground-truth sweep force the redesign (not a refactor):

- **The "protobuf migration" is a stringly-typed 1:1 mirror of the old JSON view-DTOs** —
  double-encoded (serialize→JSON→reparse→prost server-side; decode→map-back-to-snake_case
  client-side), 8 whole JSON blobs tunneled through `*_json` string fields, **zero enums,
  no envelope, no `oneof`, no `Link`** (§7 of [01](01-status-quo.md)). It is a wire interlayer,
  not a data model.
- **There is no adapter abstraction and no benchmark abstraction.** `grep "trait.*Adapter"`
  → 0 hits; "the adapter" is one 4285-line module bound to `symbiotic_memory`'s concrete
  types. `run_selected_benchmark` is a `match` with one live arm `("symbiotic-memory",
  "long-mem-eval")`; LongMemEval's gold/judge/scorer/metrics are welded into the pipeline
  and frozen into the wire schema ([02](02-adapter-agnostic-core.md), [04](04-benchmark-multimodal-core.md)).

The redesign is one idea applied twice: **the pluggable side ships a typed manifest; the host
hardcodes the envelope and the renderers, never the content, and ignores what it doesn't
understand.** That is how LSP, VS Code, Kubernetes CRDs, OpenTelemetry, and Backstage all stay
open-ended. membench already half-does it (the Tuner's `ParamField`, the waterfall's open
`kind` union) — we promote it to first-class and extend it to both axes.

---

## 1. Information architecture — four persona workspaces

Top-level nav is **four workspaces**, one per persona job, keyboard-addressable, with the
**URL carrying identity** (`#/runs/<id>/questions?verdict=wrong&qtype=multi&cause=rank-miss`
is copy-pasteable and lands a colleague exactly there).

| Key | Workspace | Persona | Owns | Landing |
|-----|-----------|---------|------|---------|
| **F1** | **LEADERBOARD** | Evaluator / buyer | rank systems, prove comparability, quality-vs-cost, shareable Report | ranked cohort table |
| **F2** | **RUNS** | Engineer / tuner | drill run→question→evidence→trace, failure taxonomy, diff-to-baseline | registry tree → run → Overview |
| **F3** | **LAB** | Operator / FAFO | launch/stop/pause/retry, schema-driven composable tuning, budget cap, live | active-runs fleet board |
| **F4** | **CATALOG** | Everyone (the spine) | adapters × benchmarks manifests + capability/comparability matrix | the matrix |

**Why CATALOG is its own workspace, not a drawer.** Genericity is now the product. With many
adapters and many benchmarks, "what plugs in, what can it do, what's comparable to what,
which knobs does this pairing expose, where is multimodal refused" is a first-class, frequently
used surface — and the natural place to *show off* the manifest-driven design. It also holds the
one honest answer a multi-system leaderboard needs: the `(adapter × benchmark)` capability &
comparability resolution. It collapses to a Leaderboard drawer if ever desired; nothing else moves.

**Peer-nav is these four only.** The old eight screens were never peers: five are *tabs of a
selected run* (Overview/Questions/Compare/Evidence/Traces), and two were *fleet/operator* jobs
(Live/Tuner) that belong in LAB. This dissolves the "launch an experiment is buried three levels
deep under a run tab" problem.

### Screen → data-product map (screens compose products; screens do not define schemas)

| Surface | Renders from | Materialization |
|---|---|---|
| Leaderboard table | `CohortSummary` + `LeaderboardRow[]` (widened cohort key) | post-run/cohort, cache by cohort hash |
| Quality-vs-cost, robustness | reductions over `LeaderboardRow` | client-derived |
| Report projection | same rows, calm view + trust metadata | export (URL + static HTML/PDF) |
| RUNS / Overview | `RunSummary` + failure-taxonomy rollup | on-run-finalize |
| RUNS / Questions | `QuestionRunRecord[]` (the join row) | incremental on question-completion |
| RUNS / Question drawer | `QuestionRunRecord` + `EvidenceCoverage` + `RecallDropout` + `OracleDelta` | lazy, cached by source-hash |
| RUNS / Traces | `StepEnvelope` frames → waterfall view | source of truth + view |
| LAB / fleet | `LiveRunState` + `QueueState` (online counters) | incremental during run |
| LAB / Tuner | `AdapterCapabilityManifest.knobs ⊕ BenchmarkManifest.knob_deltas` | resolved live |
| LAB / ablation matrix | `RunPlan[]` (Cartesian of knob-diffs) | on-demand |
| CATALOG / matrix | resolve(`AdapterCaps` × `BenchmarkManifest.requires`) | on-demand |
| CATALOG / manifest cards | `AdapterCapabilityManifest`, `BenchmarkManifest` | served from adapter/benchmark |

---

## 2. The unified data model

Five proto packages, cleanly separated by lifetime and owner. **Query-critical fields are
first-class typed columns; enums for closed sets; `optional` where absent ≠ zero; Struct/JSON
only as a clearly-named leaf escape hatch.**

```
membench.adapter.v1      AdapterCapabilityManifest   (served by the adapter)
membench.benchmark.v1    BenchmarkManifest           (served by the benchmark)
membench.content.v1      Content = [ContentPart]      (modality-generic spine, shared)
membench.trace.v1        StepEnvelope + Link          (append-only source of truth)
membench.view.v1         materialized views           (was debugger.proto; provenance-stamped)
```

### 2.1 Manifests — the two pluggable contracts

Both follow the same rules: reserved namespaces (`membench.*` host-owned, `x-<id>.*`
adapter/benchmark-owned), *what* (schema) separated from *how* (view hints), unknown fields
stored not rejected.

```proto
// membench.adapter.v1
message AdapterCapabilityManifest {
  string id = 1; string version = 2; MemoryParadigm paradigm = 3; // EXTRACT_STORE|RAG_HYBRID|TEMPORAL_GRAPH|AGENTIC_SELFEDIT|TIERED_OS|NEURAL_PARAMETRIC
  Capabilities capabilities = 10;   // typed feature flags → host gates tabs/panels
  KnobSchema   knobs        = 11;   // the "model card of settings" → Tuner (ParamField++)
  repeated Preset presets   = 12;   // adapter-shipped named knob-diffs
  StepTaxonomy steps        = 13;   // pipeline vocabulary → Traces lanes/legend
  repeated TraceStream trace_streams = 14;
  repeated ModelRole   model_roles   = 15;  // role→CallKind→queue binding (shared queue infra)
  AdapterCaps  caps         = 16;   // modalities[], produces_retrieval_ranks, exposes_memory_ops
  ViewHints    view         = 17;   // Systems/Compare printer-columns (CRD-style)
}

// membench.benchmark.v1
message BenchmarkManifest {
  string id = 1; string version = 2; string display_name = 3;
  DatasetSpec dataset = 4;                       // fixed corpus OR generator (RULER/MM-NIAH)
  repeated QuestionTypeDef question_types = 5;   // DATA-DRIVEN vocabulary, NOT a Rust enum
  GoldEvidenceSpec gold = 6;                     // typed positional EvidenceRef, may be empty
  repeated Modality modalities = 7;              // TEXT default; opt into IMAGE/AUDIO/VIDEO/DOCUMENT
  repeated ScorerSpec scorers = 8; JudgeSpec judge = 9;
  repeated MetricSpec metrics = 10;              // open set → extensible report
  KnobDeltas knob_deltas = 11;                   // knobs it adds/removes for a run
  repeated string requires_adapter_caps = 12;    // e.g. "image_content","retrieval_ranks"
}
```

Full field-level sketches: [02 §2](02-adapter-agnostic-core.md) (adapter) and
[04 §2](04-benchmark-multimodal-core.md) (benchmark).

### 2.2 Content — the modality-generic spine (one renderer, everywhere)

Every message, gold piece, answer-context item, and trace payload is a `Content = [ContentPart]`.
Text is the degenerate 1-part case, so the current text pipeline is structurally unchanged;
image/audio/video/document are **additive**.

```proto
message Content { repeated ContentPart parts = 1; }
message ContentPart {
  oneof part { ContentText text = 1; ContentMedia media = 2; ContentReasoning reasoning = 3; }
}
message ContentMedia { MediaKind kind = 1; MediaRef ref = 2; string format = 3; }
message MediaRef { string locator = 1; string sha256 = 2; string format = 3; } // POINTER; base64 only at call-time
```

Storage is **pointer, not inline** (a media store beside the vault store, addressed by sha256).
UI renders parts through one `<ContentView>` component — multimodal is one component, not a
per-panel change ([04 §4](04-benchmark-multimodal-core.md)).

### 2.3 StepEnvelope — the append-only source of truth

One record type for **static traces AND live streaming**. Uniform header the tailer/differ/
waterfall builder never change; typed `oneof` payload keyed by `kind`; a `Link` DAG for the
fan-out/fan-in/queue/retry relations a parent tree loses; an `ext` bag as the last resort.

```proto
// membench.trace.v1
message StepEnvelope {
  string step_id = 1; string run_id = 2; string trace_id = 3;   // trace_id = one question/task
  string parent_id = 4; repeated Link links = 5;                // DAG edges (see below)
  string namespace = 6;                                          // user/session correlation
  TraceFamily family = 7;    // MEMORY|PROVIDER|RECALL|GRAPH|JUDGE|RUNNER
  string kind = 8; string lane = 9;                             // taxonomy id; unknown → neutral lane
  google.protobuf.Timestamp ts_start = 10; google.protobuf.Timestamp ts_end = 11;
  Status status = 12; uint32 attempt = 13;
  optional int64 wait_ms = 14; optional int64 run_ms = 15; optional int64 item_count = 16;
  oneof payload {              // exactly one; new prongs = new field numbers, old readers skip
    MemoryOp     memory   = 30;   // ADD|UPDATE|DELETE|NOOP|MERGE|SUPERSEDE|EVICT|FORGET
    ProviderCall provider = 31;   // model, tokens, batch_size, charge_unit, throttle/queue/http ms, cost
    RecallEvent  recall   = 32;   // corpora[], candidates[{id,corpus,mode,score,rank}], rerank[], gate{dropped}, gold_rank
    GraphEvent   graph    = 33;   // extracted{nodes,edges}, resolution[], invalidations[], traversal
    JudgeEvent   judge    = 34;   // question, answer, gold, support{ids,score}, verdict, question_type
    RunnerEvent  runner   = 35;   // dataset, concurrency{configured,observed}, stage_timings
  }
  map<string,string> ext = 40;  // x-<adapter>.* ; host stores, never rejects
}
message Link { string ref = 1; LinkRelation rel = 2; } // TRIGGERED_BY|BATCH_MEMBER|AGGREGATES|FOLLOWS_FROM|RETRY_OF|COMPENSATES
enum Status { OK=0; RUNNING=1; PENDING=2; FAILED=3; CANCELLED=4; }
// Streaming = the same envelope incrementally: StepStart(ts_end unset) / StepUpdate(merge by step_id) / StepEnd.
```

**Decision rules** (memorize): duration-work you'd draw as a bar → derive a **span** from a
matched start/end pair; point-in-time state change → an **event** (the primitive, source of
truth); count/sum/rate/quantile → a **metric** (always rebuildable, never the only record);
a causal edge the parent tree can't express → a **Link**. This is where OTel itself landed
(events are the atom; spans derive) — [03 §1](03-best-practices.md).

### 2.4 Views — materialized, provenance-stamped, hash-invalidated

Every derived artifact (waterfall, dependency graph, `EvidenceCoverage`, `RecallDropout`,
cost rollup, gold-rank distribution, `LeaderboardRow`, `CompareResult`, `LiveRunState`) is a
**projection you can throw away and replay from the log**, carrying a header:

```proto
message ArtifactProvenance {
  string schema = 1; uint32 schema_version = 2; string builder = 3;   // materializer + git_sha
  repeated SourceHash sources = 4;                                    // {path, sha256, byte_len}
  google.protobuf.Timestamp generated_at = 5; bool complete = 6;      // false = partial/truncated
  optional string invalidated_by = 7;
}
```

Cache validity = `sha256(current source logs) == provenance.sources[*].sha` — **invalidate by
source hash, never by clock**. This promotes the existing `SYMEM_IGNORE_SOURCE_HASH` staleness
gate to a first-class, checkable fact on every view. Online (cheap counters/gauges during the
run) vs post-hoc (whole-stream joins built lazily on first request) — [03 §3](03-best-practices.md).

### 2.5 QuestionRunRecord — the join row (fully populated)

The de-facto record already exists (`QuestionRow`) but stops at verdict/hypothesis/provenance.
Promote it to the central object and fold in what's already keyed on the same `question_id`:

```proto
message QuestionRunRecord {
  string run_id = 1; string question_id = 2; string question_set_id = 3;
  string question_type = 4;                 // from BenchmarkManifest.question_types (not an enum)
  Content prompt = 5; Content gold_answer = 6; Content hypothesis = 7;
  Verdict verdict = 8;                       // label, is_abstention, judge model/prompts/raw
  FailureClass failure_class = 9;            // RETRIEVAL_MISS|RANK_MISS|READER_MISS|ABSTAIN|HARNESS_ERR
  repeated GoldEvidenceRef gold_evidence = 10;
  EvidenceCoverage coverage = 11;            // per-piece: retrieved? embed_rank rerank_rank in_ctx cited
  optional OracleDelta oracle = 12;          // live vs oracle-gold → denoise-gap vs reader-wall
  CostSummary cost = 13; TimingSummary timing = 14;
  repeated string trace_ids = 15;            // → StepEnvelope frames for this question
  ReviewState review = 16;                   // durable human annotations (bad gold, judge issue, alias)
}
```

Address **purely by `question_id` + known layout** — drop the leaked path-addressed
`debug_artifact` pointer (§6/§17 of [01](01-status-quo.md); the writer already reconstructs from
the id). `FailureClass`, `EvidenceCoverage`, and `OracleDelta` are the crown-jewel differentiator
made data: they power the RUNS taxonomy tiles, the coverage matrix, and the recall drop-out view.

### 2.6 Cohort identity widens (comparability honesty)

```
cohort_id(benchmark_id, benchmark_version, adapter_id, adapter_caps_hash,
          limit, dataset_fp, judge_model, judge_prompt_mode, metric_version)
```

Two runs are comparable only when benchmark **and** the resolved component/metric set match —
so a rank-less adapter never silently pollutes a rank-scored cohort ([04 §3](04-benchmark-multimodal-core.md)).
The Leaderboard renders comparability as a first-class state, not a footnote.

---

## 3. The (adapter × benchmark) resolver — deterministic, fail-closed

A run is `Benchmark ⋈ Adapter`. The benchmark declares **needs**; the adapter advertises
**capabilities**; the host resolves the intersection in this order (CATALOG visualizes it):

1. **Capability gate (fail-closed).** `Benchmark.modalities ⊄ Adapter.modalities` or a missing
   required cap → **refuse** with a named gap (e.g. "text-only reranker + image-needle benchmark").
2. **Active components** = `Benchmark.declared ∩ Adapter-satisfiable`. `rank_scoring` activates
   only if `Adapter.produces_retrieval_ranks`; else dropped with a recorded reason (benchmark
   still runs, minus that view).
3. **Active knobs** = `engine_defaults ⊕ Adapter.knob_deltas ⊕ Benchmark.knob_deltas ⊕ RunOverrides`
   (key-wise override; `unset` removes). A benchmark need the adapter contradicts and cannot
   satisfy → capability gap (step 1); tuning conflicts resolve last-wins toward RunOverrides.
4. **Metric set** = `Benchmark.metrics` filtered to active components; dropped metrics are
   recorded so the leaderboard never compares partial sets.

This is the logic behind the CATALOG matrix cells (OK / PARTIAL / REFUSED) and the LAB Tuner's
appear/disappear knobs.

---

## 4. Transport & storage

- **Append log** = length-delimited protobuf frames (varint prefix per `StepEnvelope`) — the
  typed JSONL replacement: 3–10× smaller, typed, forward-compatible, tailable, truncate-recoverable.
- **Live tailing** = Connect server-stream (protobuf-es/Connect-Web, already in `dashboard/src/lib/gen`).
- **History** = cursor-paged fetch **separated by record type** (`events?trace_id&kind=RECALL&cursor`);
  never stream a full 500-question run through one socket.
- **JSONL/JSON stays** for human-grepped leaf artifacts (a single judge transcript, the final report).
- **Kill the duplication** (§4/§8 of [01](01-status-quo.md)): today the two large trace files each
  exist in 2 byte-identical copies (~248 MB of a ~270 MB run) via `fs::copy`, plus a legacy `raw/`
  layer. One canonical native log per stream + a published manifest that *references* it
  (hash/range), not re-copies it. Move request-time derivations (the uncached `/run/traces`
  waterfall rebuild) into finalize-time materialized views.

---

## 5. Migration — incremental, low-risk (the spines already exist)

1. **Wrap, don't rewrite.** Serve today's `symem_param_schema()` + trace DTOs as
   `AdapterCapabilityManifest` #1 and a `BenchmarkManifest` for LongMemEval. Nothing visual changes.
2. **Delete the two hardcodes:** `"system":"symbiotic-memory"` (`membench-server.rs:3128`) and the
   `("symbiotic-memory","long-mem-eval")` match arm (`membench.rs:1364`) → registry lookups.
3. **Lift Questions filter state into the URL** — the single change that unlocks tile-drilldown
   *and* shareable links across all personas.
4. **Extract `membench-core`:** workflow queue, provider queue, resumability (generic run-manifest
   keyed by `StepKind`, not `MemoryStage`), cost analytics — shared infra every adapter inherits.
5. **`StepEnvelope` + `Link` + streaming** → fan-out draws; Live and static Traces unify on one renderer.
6. **`Content` spine text-degenerate first** (zero behavior change), media additive after — the one
   hard external dependency is the `symbiotic-memory` `String`→`Content` boundary; sequence it last,
   guarded by the capability gate.
7. **Prove it:** write the **mem0 adapter** against `membench-core` — it supplies a manifest + a
   "run one item" impl and inherits Tuner, Traces, Systems, Compare, queueing, cost analytics with
   **no host change**. That is the open-closed guarantee, and the proof the core is agnostic.

Everything else — the 4-workspace spine, URL addressing, master-detail, waterfall components,
command palette, status bar — is kept and sharpened, not thrown away.

---

## 6. How the prototypes embody this

- **[runs.html](prototype/runs.html)** — `QuestionRunRecord` + `FailureClass` tiles +
  `EvidenceCoverage` matrix + `RecallDropout` funnel + `OracleDelta`. The differentiator.
- **[lab.html](prototype/lab.html)** — Tuner rendered from the manifest's `knobs ⊕ knob_deltas`,
  the ablation matrix as a Cartesian of knob-diffs, budget cap, fleet control, live `StepEnvelope` tail.
- **[leaderboard.html](prototype/leaderboard.html)** — widened cohort key, cross-adapter rows,
  comparability state, quality-vs-cost Pareto, Report projection.
- **[catalog.html](prototype/catalog.html)** — the `(adapter × benchmark)` resolver matrix +
  `AdapterCapabilityManifest` and `BenchmarkManifest` cards.
- **[steptrace.html](prototype/steptrace.html)** — one `StepEnvelope` renderer drawing
  symbiotic-memory AND mem0, `Link` DAG fan-out, graceful unknown-kind, capability-driven panel swap.
