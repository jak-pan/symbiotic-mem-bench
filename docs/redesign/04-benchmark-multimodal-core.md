# Benchmark-Plugin & Multimodal Core — membench Architecture Brief

**Author:** Lead Architect · **Scope:** make membench run diverse memory benchmarks (LongMemEval today; multimodal + others next), composing with any memory adapter. **Verdict:** membench has no benchmark abstraction — LongMemEval is not *a* benchmark, it *is* the benchmark, welded into run/answer/gold/score/proto/UI. The path forward is a typed `BenchmarkManifest` + a `Content=[ContentPart]` spine, resolved against adapter capabilities. This is a large but bounded refactor: the registry/leaderboard/cohort/trials layer is already benchmark-parametric and does not move.

---

## 1. The problem today — where LongMemEval is hardcoded

There is **no `Benchmark`/`Dataset`/`Scorer`/`Judge` trait or enum** anywhere in `src/`. The concrete type `LongMemEvalRecord` (`src/symbiotic_memory_adapter.rs:108`) is threaded through the entire pipeline, and dispatch is a single match arm.

**The dispatch choke point.** `run_selected_benchmark` (`src/bin/membench.rs:1363`) is literally `match (system.as_str(), benchmark.as_str())` with one live arm — `("symbiotic-memory", "long-mem-eval")` at line 1364. Every other value hits `anyhow::bail!("unsupported benchmark selection")` at line 1520. The `--benchmark` CLI flag accepts any free string but has nowhere to route.

**Components that are effectively LongMemEval plugins** (each is a hardcoded implementation of a seam that must become swappable):

| Component | Where | Why it's LongMemEval-specific |
|---|---|---|
| **Dataset loader + record** | `adapter.rs:108,251,263` — `LongMemEvalRecord`, `load_longmemeval`, `longmemeval_to_source` | `serde_json::from_str::<Vec<LongMemEvalRecord>>`; hardcoded HF download of `longmemeval_s_cleaned.json` (`membench.rs:1592,1617`) |
| **Question model** | `adapter.rs:108` — `{question_id, question_type: Option<String>, answer_session_ids, haystack_sessions, …}` | `question_type` is a free string (good, data-driven) but consumers branch on literal `temporal-reasoning`/`knowledge-update`/`single-session-preference` |
| **Gold / coverage evaluator** | `membench.rs:2570-2900` — `gold_eval`, `gold_turn_ids`, `gold_piece_of_turn` (`session:turn`), `deepest_gold_rank`, `single_piece`/`multi_piece` | Pure LongMemEval concepts: session, has_answer, haystack_sessions, embed/rerank rank. **The single most-coupled component.** |
| **Judge / grader** | `membench.rs:5384` — `judge_prompt`, `is_abstention` (`5456`) | Verbatim arXiv:2410.10813 per-type grader prompts; English abstention keyword list; text-only I/O |
| **Scorer / metric set** | `membench.rs:5184` — `score_prepared_longmemeval_native` → `{overall_accuracy, task_averaged_accuracy, abstention_accuracy, per_question_type}` | Metric *mechanism* (LLM-judge → yes/no → count) is generic; the *metric set* is LongMemEval's |
| **Oracle-gold context** | `adapter.rs:2706` — `build_gold_oracle_context` | Session-shaped; the "feed only gold" method is generic, the impl isn't |
| **Runner arg builder** | `runner.rs:99-104` — always emits `--symbiotic-memory --long-mem-eval` | Hardcodes dataset filename + LongMemEval config defaults (`371-456`) |
| **Wire schema (proto + TS)** | `debugger.proto:58,71,545-600`; `types.ts:641-704` | `task_averaged_accuracy`, `per_question_type`, `GoldEvalSummary{single_piece, multi_piece, gold_pieces_needed/covered}`, `GoldRankSummary{embed, rerank}` — **every gold field is a LongMemEval concept, frozen into the wire format** |
| **Modality** | — | `grep image\|audio\|video\|multimodal\|mime` over `src/` returns nothing. `LongMemEvalMessage.content: String` (`adapter.rs:126`) → `SourceTurn.text: String`. Text-only wall. |

**Already generic — does NOT move:** registry / leaderboard / cohort / trials are parametric on the `benchmark: String` field (`registry.rs:39`; `cohort.rs`; `leaderboard.rs`) — they'd carry a new benchmark's runs today *if it wrote the expected report shape*. The **memory-engine config yamls** (`config/symbiotic-memory/longmemeval-*.yaml`) tune the system-under-test, not the benchmark — only *named* by convention. Provider/queue/cost/trace plumbing is benchmark-agnostic.

**The structural boundary:** the text-only wall is not entirely membench's to fix. `SourceTurn.text: String`, `ChatProvider::chat(system:&str, user:&str)`, and the embedder/reranker signatures live in the `symbiotic-memory` dependency. membench can define the manifest, the content model, and the resolution rules; **multimodal ingest/answer/rerank requires a coordinated change in symbiotic-memory** (see §4, §6 DON'T).

---

## 2. BenchmarkManifest — the typed contract a benchmark declares

A benchmark is `{Dataset, QuestionSpec[], GoldEvidenceSpec, Modalities, Scorer[], Judge?, Metrics[], KnobDeltas}`, discovered via a registry (YAML for declarative benchmarks; trait-object for programmatic ones — the lm-eval / BIG-bench split). This is the Scenario/Adapter/Metric separation HELM and Inspect converged on, extended with LongMemEval/LoCoMo/MMLongBench-style typed, positional, possibly-empty, possibly-multimodal gold.

Core design decisions baked into the contract:
- **Question types are data-driven, not an enum.** `type` is a declared string vocabulary in the manifest, not a Rust variant — a new benchmark ships its own type set (`counting`, `image-retrieval`, `forgetting`, …) with no code change. Consumers that today branch on literals become table lookups keyed by the manifest's `question_types`.
- **Gold generalizes to a list of typed positional references that may be empty.** `session:turn` becomes one *kind* of `EvidenceRef`; `page`, `needle_depth`, `doc:region` are peers. The empty set is first-class (LongMemEval abstention, MMLongBench-Doc 22.5% unanswerable).
- **Metrics are an open container, not a fixed struct.** A benchmark declares its metric set (accuracy, F1, ROUGE, recall@k, gold-rank distribution, memory-op count). The wire schema must carry an extensible `map<string,double>` + typed distributions, not `task_averaged_accuracy` as a named field.
- **KnobDeltas is a config diff, not a fork.** A benchmark ships a diff over engine defaults (`abstention_scoring: on`, `retrieval_rank_metric: on`, `image_turn_passthrough: required`), never an engine fork.

### Protobuf sketch (`proto/membench/benchmark/v1/manifest.proto`)

```protobuf
syntax = "proto3";
package membench.benchmark.v1;

message BenchmarkManifest {
  string id            = 1;   // "long-mem-eval", "locomo", "mm-niah"
  string display_name  = 2;
  string version       = 3;

  DatasetSpec              dataset      = 4;
  repeated QuestionTypeDef question_types = 5;  // data-driven vocabulary, NOT an enum
  GoldEvidenceSpec         gold         = 6;
  repeated Modality        modalities   = 7;    // TEXT default; benchmark opts into more
  repeated ScorerSpec      scorers      = 8;    // ordered; ≥1
  JudgeSpec                judge         = 9;    // optional; a specialization of Scorer
  repeated MetricSpec      metrics      = 10;
  KnobDeltas               knob_deltas  = 11;
  repeated string          requires_adapter_caps = 12; // e.g. "image_content","retrieval_ranks"
}

enum Modality { MODALITY_TEXT=0; MODALITY_IMAGE=1; MODALITY_AUDIO=2;
                MODALITY_VIDEO=3; MODALITY_DOCUMENT=4; }

message DatasetSpec {
  oneof source {
    FixedCorpus corpus = 1;       // LongMemEval, LoCoMo, MMLongBench-Doc
    GeneratorSpec generator = 2;  // RULER / MM-NIAH: length × #needles × depth × needle_modality
  }
  string loader = 3;              // registered loader id → record type + to_haystack mapping
  string default_uri = 4;         // HF path / local; loader-specific
  bytes  dataset_kwargs = 5;      // opaque JSON to the loader
}

// Question types are DECLARED, not compiled-in. Drives per-type metric breakdown + judge routing.
message QuestionTypeDef {
  string id = 1;                  // "temporal-reasoning","multi-hop","counting","abstention"
  string display_name = 2;
  bool   is_abstention = 3;       // gold is expected-empty for this type
  string expected_answer_type = 4;// live router (string, not enum) — mirrors today's live behavior
  string judge_prompt_ref = 5;    // optional per-type grader override; else JudgeSpec.default
}

// Generalizes "gold piece / gold turn / coverage" to typed positional refs, possibly empty.
message GoldEvidenceSpec {
  EvidenceRefKind ref_kind = 1;   // how a ref addresses the haystack
  bool allow_empty = 2;           // abstention / unanswerable is valid gold
  bool coverage_scoring = 3;      // compute pieces_needed vs pieces_covered
  bool rank_scoring = 4;          // compute retrieval gold-rank (embed/rerank stages)
  repeated string rank_stages = 5;// ["embed","rerank"] — generalized from GoldRankSummary
}
enum EvidenceRefKind { REF_TURN=0; REF_PAGE=1; REF_NEEDLE_DEPTH=2; REF_DOC_REGION=3; }

// A concrete gold entry per question (loader emits these):
message GoldEvidence {
  repeated EvidenceRef refs = 1;  // empty ⇒ abstention
  Content answer = 2;             // canonical answer as content-parts (text = 1-part case)
  map<string,double> answer_scores = 3; // multiple-choice targets, optional
}
message EvidenceRef {
  string locator = 1;             // "s12:4" | "p37" | "depth:0.6" | "doc7#fig2"
  Modality modality = 2;          // gold piece carries its modality (MMLongBench source-type)
  string source_type = 3;         // "text"|"figure"|"chart"|"table"|"layout" (optional refinement)
}

message ScorerSpec {
  string kind = 1;  // "model_graded" | "exact" | "f1" | "rouge" | "choice" | "retrieval_rank"
  bytes  config = 2;
}
message JudgeSpec {          // Judge = Scorer{kind="model_graded"} with model + prompt
  string model = 1;
  string default_prompt = 2;
  bool   multimodal = 3;    // judge sees Content, not just String
  string abstention_detector = 4; // named strategy id, NOT a hardcoded keyword list
}

message MetricSpec {
  string id = 1;            // "overall_accuracy","task_averaged","abstention_accuracy",
                            // "recall_at_k","gold_rank_p50","memory_ops"
  string aggregation = 2;   // "mean"|"task_mean"|"recall"|"distribution"
  bool   higher_is_better = 3;
  repeated string group_by = 4; // ["question_type"] → per-type breakdown, generalized
}

// A config DIFF over engine defaults — never a fork.
message KnobDeltas {
  map<string,string> set = 1;    // "abstention_scoring"="on", "image_turn_passthrough"="required"
  repeated string    unset = 2;  // knobs this benchmark disables
}
```

The existing LongMemEval `benchmark-report.json` metrics map onto this as: `overall_accuracy` → `MetricSpec{id:"overall_accuracy",aggregation:"mean"}`; `task_averaged_accuracy` → `aggregation:"task_mean"`; `per_question_type` → any metric with `group_by:["question_type"]`; `GoldRankSummary{embed,rerank}` → `GoldEvidenceSpec.rank_stages=["embed","rerank"]`.

---

## 3. The (adapter × benchmark) matrix — resolving live knobs and active components

A run is **Benchmark ⋈ MemoryAdapter**. The adapter is the system-under-test hook (HELM CompletionFn / Inspect Solver / lm-eval model type), kept *separate* from the benchmark. The benchmark declares **needs**; the adapter advertises **capabilities**; the tool resolves the intersection and refuses/warns on gaps.

**Adapter advertises a capability descriptor:**
```
AdapterCaps {
  modalities: [text, image, …]      // what content it can ingest/carry end-to-end
  produces_retrieval_ranks: bool    // can it report embed/rerank gold-rank?
  exposes_memory_ops: bool          // MemBench-style efficiency metrics
  knob_deltas: KnobDeltas           // adapter-specific config diff (e.g. symbiotic-memory recall knobs)
}
```

**Resolution rules (deterministic, in this order):**

1. **Capability gate (fail-closed).** `required = Benchmark.requires_adapter_caps`. If `Benchmark.modalities ⊄ Adapter.modalities`, or a required cap (`retrieval_ranks`, `memory_ops`) is absent → **refuse** with a named gap (e.g. "text-only reranker + image-needle benchmark"). This mirrors the existing source-hash gate for answer-only rigs — a hard, legible stop, not a silent degrade.

2. **Active components (union of what both sides support).**
   - Scorer/Judge/Coverage/Rank: `active = Benchmark.declared ∩ Adapter-satisfiable`. `rank_scoring` activates *only if* `Adapter.produces_retrieval_ranks`; otherwise it's dropped with a warning (benchmark still runs, minus that view). `memory_ops` metrics activate only if `Adapter.exposes_memory_ops`.
   - Coverage view activates iff `GoldEvidenceSpec.coverage_scoring && gold.allow_empty`-aware.

3. **Active knob set (layered merge, later wins on key conflict).**
   ```
   active_knobs = engine_defaults
                ⊕ Adapter.knob_deltas      // system-under-test tuning (the config/*.yaml today)
                ⊕ Benchmark.knob_deltas     // benchmark needs (abstention on, image passthrough)
                ⊕ RunOverrides              // CLI / tuner sweep values
   ```
   `⊕` = key-wise override; `unset` removes a key. **Conflict rule:** if a Benchmark KnobDelta requires a value the Adapter's delta *contradicts and cannot satisfy* (e.g. benchmark requires `image_turn_passthrough=required`, adapter forces it `off`), that is a **capability gap → refuse** (step 1), not a silent last-wins. Contradictions on *tuning* knobs (top-k, planner) resolve last-wins in favor of RunOverrides.

4. **Metric set = `Benchmark.metrics` filtered to active components.** A dropped component (e.g. rank scoring on a rank-less adapter) drops its metrics from the report; the report records *which* were dropped and why, so the leaderboard never silently compares partial metric sets across adapters.

**Cohort identity must widen.** Today `cohort_id(benchmark, limit, dataset_fp, judge, prompt_mode)` (`cohort.rs:73`). It must become `cohort_id(benchmark_id, benchmark_version, adapter_id, adapter_caps_hash, limit, dataset_fp, judge, prompt_mode)` so two runs are only comparable when benchmark **and** the resolved component/metric set match. Otherwise a rank-less adapter's run pollutes a rank-scored cohort.

**Concrete examples:**
- *LongMemEval ⋈ symbiotic-memory:* modalities `[text]` ⊆ adapter; `rank_scoring` on (adapter produces ranks) → embed/rerank gold-rank live; abstention on; per-`question_type` breakdown. Identical to today's behavior — the refactor is behavior-preserving for the existing path.
- *MM-NIAH ⋈ text-only adapter:* modalities include `image` ⊄ adapter → **refused** at step 1.
- *MM-NIAH ⋈ multimodal adapter:* image-needle passthrough required + satisfied; metrics grouped by `(task × needle_modality × depth)`; gold = needle content + depth ref.
- *MemBench ⋈ adapter with `exposes_memory_ops`:* efficiency metrics (memory-op count, capacity degradation) activate; a plain adapter runs effectiveness-only with a recorded drop.

---

## 4. Modality-generic data model — one content spine, everywhere

The convergent industry pattern (Inspect, chat APIs, lmms-eval, VHELM) is a **content-part array on every message**. Adopt it as the single spine.

```
Content     = [ContentPart]                       // text is the degenerate 1-part case
ContentPart = ContentText   { text }
            | ContentMedia   { kind: IMAGE|AUDIO|VIDEO|DOCUMENT, ref: MediaRef, format }
            | ContentReasoning { … }               // for traces
MediaRef    = { locator: uri | path | bytes(base64), sha256, format }  // POINTER by default
```

**Threaded uniformly through every stage** — question, gold, answer-context, trace all share it:

| Stage | Today | Modality-generic |
|---|---|---|
| Question prompt | `question: String` | `QuestionSpec.prompt: Content` |
| Haystack turn | `LongMemEingMessage.content: String` → `SourceTurn.text` | turn carries `Content`; text-only turns are 1-part |
| Gold evidence | `answer: JSON→String`, `session:turn` id | `GoldEvidence.answer: Content`; refs are pointers into the content graph |
| Answer context (to reranker/answerer) | list of strings | list of `Content` items — image turns survive retrieval→rerank→answer with no string-flattening |
| Trace / hypothesis | `BenchHypothesis.hypothesis: String` | `Content` (text degenerate) |

**Storage: pointer, not inline (mandatory).**
- Datasets and traces store `MediaRef` (locator + sha256 + format), **never base64 inline by default**. Base64 is materialized *only at model-call time* by the adapter. This keeps `benchmark-report.json`, gold-eval, and rerank-traces small, diffable, and cache-friendly — matching ViDoRe/M3DocRAG page-image and LoCoMo/MileBench handling.
- `sha256` is the content-address: media dedup across turns, cache key, and integrity check. A media store (`SYMEM_MEDIA_STORE`, sibling to the existing vault store) holds blobs by hash; refs resolve through it.

**Gold and answer-context are pointers into the same content graph.** This is the natural generalization of the existing turn-centric provenance star (briefs cite *turns*, not facts). "Turns are content-part lists" means the existing text pipeline is structurally unchanged (every current turn is a 1-part text turn) while image/audio/video become **additive** — no rewrite of the text path.

**UI renders parts generically.** A single `<ContentView parts={Content}>` component walks the part list: `ContentText` → markdown; `ContentMedia{IMAGE}` → `<img>` from resolved ref (lazy, by sha256); `AUDIO/VIDEO` → player; `DOCUMENT` → page-thumbnail. Every panel that shows question / gold / evidence / answer-context / trace uses this one renderer, so multimodal support is one component, not a per-panel change.

**Cross-repo dependency (call it out explicitly):** the content spine terminates at the `symbiotic-memory` boundary. Ingest (`longmemeval_to_source`), answer (`engine.answer_with_reference_date(&str)→answer.text:String`), and embed/rerank all take/return `String` today. Full multimodal requires symbiotic-memory to accept `Content` at `SourceTurn`, `ChatProvider::chat`, and the embedder/reranker. membench should define `Content` in a shared crate (or mirror it) and land the **text-degenerate** path first (zero behavior change), then coordinate the symbiotic-memory signature change for real media. Until then, the capability gate (§3) refuses multimodal benchmarks against text-only adapters rather than silently flattening images to strings.

---

## 5. Swappable vs core

**Must become swappable (the benchmark-plugin surface — everything a manifest declares or supplies):**
1. **Dataset loader + record type** — `trait BenchmarkLoader { fn load() -> Haystack + Vec<QuestionSpec>; }`. Replaces `LongMemEvalRecord` + `load_longmemeval` + `longmemeval_to_source`. Must abstract fixed-corpus vs parameterized-generator (RULER/MM-NIAH).
2. **Question / gold model** — `QuestionSpec` (data-driven `type`) + `GoldEvidenceSpec` (typed positional refs, possibly empty). Replaces the `answer_session_ids`/`has_answer`/`haystack_sessions` shape.
3. **Evidence / coverage view** — the whole `gold_eval` machinery (`membench.rs:2570-2900`) becomes a `trait CoverageEvaluator` driven by `GoldEvidenceSpec.ref_kind` + `rank_stages`.
4. **Scorer** — `trait Scorer { fn score(output: Content, gold: GoldEvidence) -> Score; }`; family: exact/F1/ROUGE, choice, retrieval-rank, model-graded.
5. **Judge** — a Scorer specialization: model + prompt + abstention-detection strategy, benchmark-supplied. Replaces `judge_prompt` + the hardcoded English `is_abstention` keyword list.
6. **Metric set** — open `MetricSpec[]` container + extensible report schema. Replaces the frozen `{overall, task_averaged, abstention, per_question_type}` struct.
7. **Oracle-gold context builder** — `build_gold_oracle_context` becomes ref-kind-driven (the "feed only gold" method stays; its session-specific impl goes).

**Stays core (benchmark-agnostic infrastructure — do NOT plug-in):**
- **Registry / leaderboard / cohort / trials** — already parametric on `benchmark`; only needs the widened cohort key (§3).
- **Content spine + media store** — `Content`/`ContentPart`/`MediaRef` are core primitives shared by all benchmarks.
- **Resolution engine** — the (adapter × benchmark) capability gate + knob merge (§3) is core logic, not per-benchmark.
- **Provider / queue / cost / trace plumbing** — operates on the engine, benchmark-agnostic.
- **Memory-engine config yamls** — tune the SUT, not the benchmark; stay as adapter KnobDeltas.
- **The `--benchmark` dispatch** — becomes a registry lookup instead of a match arm, but the CLI surface stays.

---

## 6. DO / DON'T + UI implications

**DO**
- Introduce `BenchmarkManifest` + a registry (`benchmarks/<id>/manifest.yaml`); make `run_selected_benchmark` a registry lookup, deleting the `("symbiotic-memory","long-mem-eval")` match arm.
- Make `question_type` a *declared vocabulary* consumed by table lookup; kill every literal branch on `temporal-reasoning`/`knowledge-update`/etc.
- Land the `Content=[ContentPart]` spine text-degenerate first (behavior-preserving), media additive after.
- Store media as `MediaRef` (pointer + sha256); materialize base64 only at call-time; add a media store beside the vault store.
- Fail-closed on capability gaps (text-only adapter + image benchmark → refuse), reusing the source-hash-gate pattern.
- Widen the cohort key with `adapter_id` + `adapter_caps_hash` + `benchmark_version` before any cross-adapter leaderboard comparison.
- Make the report metrics an extensible container in proto (`map<string,double>` + typed distributions) so a new benchmark's metrics have somewhere to land.

**DON'T**
- Don't add a second hardcoded benchmark beside LongMemEval — that doubles the coupling instead of removing it.
- Don't type any question/gold/answer/trace field as `String` where content could be multimodal — always `Content`.
- Don't inline base64 into datasets, gold-eval, reports, or traces.
- Don't flatten images to strings at the symbiotic-memory boundary to "make it work" — refuse instead until the adapter is genuinely media-capable.
- Don't freeze new benchmark-specific fields into `debugger.proto` the way `single_piece`/`task_averaged_accuracy`/`GoldRankSummary` are frozen now — generalize to `EvidenceRef` + `MetricSpec` + `rank_stages`.
- Don't compare runs across adapters in one cohort until the resolved metric/component set matches.

**UI — every panel renders from the manifest, not a hardcoded LongMemEval schema:**
- **Questions:** columns/filters/type-facets come from `manifest.question_types` (declared vocabulary), not a fixed 6-type list. Prompt cell uses `<ContentView>` — image/audio questions render inline.
- **Gold / Evidence Coverage:** driven by `GoldEvidenceSpec`. `ref_kind` labels the locator column (turn / page / depth / region); each `EvidenceRef` shows its `modality`/`source_type` badge (text/figure/chart/table). Empty-gold rows render as an explicit **abstention** state, not "missing." Coverage bars appear iff `coverage_scoring`; rank columns appear per `rank_stages` (generalizing the current embed/rerank `GoldRankSummary`) and only when the adapter produced ranks.
- **Compare:** diffs the *resolved* metric set of two cohorts; greys out metrics one side dropped (with the recorded reason) so partial-metric comparisons are never silently equated.
- **Leaderboard:** cohorts keyed by the widened id (benchmark×version×adapter×caps); a benchmark's metric columns come from `manifest.metrics`, so a recall@k / gold-rank benchmark and an accuracy benchmark render side-by-side without schema surgery.
- **Tuner:** knob set comes from `active_knobs = engine_defaults ⊕ Adapter.knob_deltas ⊕ Benchmark.knob_deltas`. Benchmark-added knobs (abstention scoring, image passthrough) and benchmark-removed knobs appear/disappear automatically; sweeps write RunOverrides at the top of the merge.

**Bottom line:** ship `BenchmarkManifest` + `Content=[ContentPart]` + the (adapter × benchmark) resolver. LongMemEval becomes the first manifest, not the hardcoded core; multimodal becomes an additive content-part case guarded by a capability gate; and the registry/leaderboard/UI render from the manifest instead of a frozen LongMemEval schema. The one hard external dependency is the symbiotic-memory `String`→`Content` boundary — sequence it after the text-degenerate spine lands.

---

**Key file anchors:** dispatch `src/bin/membench.rs:1363-1520`; record/loader `src/symbiotic_memory_adapter.rs:108,251,263`; gold-eval `src/bin/membench.rs:2570-2900`; judge/scorer `src/bin/membench.rs:5184-5462`; oracle context `src/symbiotic_memory_adapter.rs:2706`; runner `src/runner.rs:99-104,371-456`; frozen wire schema `proto/membench/dashboard/v1/debugger.proto:58,71,545-600` + `dashboard/src/lib/types.ts:641-704`; parametric-already layer `src/registry.rs:39`, `src/cohort.rs:73`, `src/leaderboard.rs`.
