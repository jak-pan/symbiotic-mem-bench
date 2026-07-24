# Redesign — hard requirements (from user, 2026-07-01)

## North star
MEMBENCH is an **agnostic benchmark + debug tool** for multi-prong / hybrid / agentic
**memory** systems. symbiotic-memory is just the first adapter. It must run mem0, hymem,
Zep, Letta, or "really anything that gets an adapter."

## Non-negotiables the design must satisfy
1. **Adapter-agnostic core.** Tracing, queueing, model-adapters must be UNIVERSAL across
   memory-system adapters — not symbiotic-memory-specific. A new adapter plugs in and
   "just works" in the UI.
2. **Extreme-tunability showcase.** Knob tuning is driven by the memory system's
   **model card / settings schema**, and is **composable** (stack/override/ablate knobs).
   The Tuner must not hardcode symbiotic-memory's 54 fields — it renders from the
   adapter's declared schema.
3. **Async-first, generic steps.** Traces / step-based things emit **as async as
   possible** AND stay **generic** so the UI renders any adapter's pipeline correctly
   without per-adapter UI code.

## Design implications (to prove in the prototypes)
- An **AdapterCapabilityManifest**: each adapter declares its knobs (from model card),
  its trace-stream types, its pipeline steps, and which optional capabilities it supports.
- A **generic-but-typed trace/step envelope** (common denominator fields + typed payload +
  open extension) so a generic timeline/waterfall renders ANY adapter.
- A **schema-driven Tuner** (knobs from manifest; composable presets/overrides/ablations).
- A **Systems** surface: multiple adapters as first-class, capability-diffed, comparable.
- Multi-system leaderboard/compare must be honest about capability gaps + comparability.

## Second genericity axis: benchmark-agnostic (added by user)
LongMemEval is ONE benchmark (text QA). We will run others incl. **multimodal** ones.
Adding/removing a benchmark warrants adding/removing knobs (for symbiotic at least) AND
swapping some membench components (scorer/judge/gold-eval/evidence-coverage are all
LongMemEval-shaped today).

Genericity is therefore a **2D matrix**: `(adapter × benchmark)` determines:
- which **knobs** are live (composable/conditional — a multimodal benchmark adds e.g.
  image-embedder / vision-reader knobs, removes some text-only ones),
- which **membench components** apply (dataset loader, question/gold model, evidence /
  coverage metric, judge, scorer, metrics),
- which **modalities** the data model + UI must carry (text / image / audio / video in
  questions, gold evidence, answer context, and traces).

Design implication: a **BenchmarkManifest** (parallel to AdapterCapabilityManifest)
declaring dataset shape, question types, gold/evidence model, modalities, metrics,
scorer/judge components, and knob enable/disable deltas. The Questions / Gold-Coverage /
scoring surfaces must render from the BenchmarkManifest, not from hardcoded LongMemEval
schema. Multimodal ⇒ content is ref-addressed (not just strings) end-to-end.
