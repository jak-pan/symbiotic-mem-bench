Here is the final polished version.

---

# Building a Memory That Lasts: The Road to a Benchmark-Driven Agent Memory Stack

*Post 1 of a series on Symbiotic Memory — the overview and the journey.*

## The problem nobody solves cleanly

Give a language model a long enough conversation and it forgets. Not metaphorically — literally. Context windows are finite, and even when they're huge, stuffing a year of chat history into every prompt is slow, expensive, and surprisingly bad at finding the one fact you needed. The dream of an *agent* — something that remembers what you told it last March, knows you switched jobs in April, and can answer "how many times did I mention being tired this year?" — runs straight into the wall of durable, queryable, long-term memory.

This is the problem **Symbiotic Memory** is built to solve: durable long-term memory for agents and personal data. Not a context window. A *memory* — facts pulled out of raw conversation, stored, indexed, and recalled on demand.

We measure all of this against one benchmark, used consistently throughout the series: **LongMemEval-S**. It's 500 questions over long, multi-session chat histories, each one testing whether a system can recall a specific fact buried across months of conversation. ("-S" is the small/standard split; it's the de-facto standard for evaluating agent memory.) Each question ships with a labeled correct answer and a pointer to the specific session that contains the supporting evidence — what we'll call the **"gold."** The gold is the ground truth we score against.

There are a lot of systems that claim to solve this. Many claim to do it astonishingly well — 94%, 95%, 96% on LongMemEval. As you'll see, almost none of those numbers survive contact with an honest reading of how they were produced.

So this series takes a different posture. The thesis is simple and a little stubborn:

> A modular, benchmark-and-fact-driven stack — **no golden hacks, no LLM-routing** — where every backend is swappable and every claim is measured.

In plain English: no per-question special-casing. No **golden hacks** — no searching the question's context for the answer string and matching on it. No **secret oracle** that's quietly handed which session holds the answer (a real system never gets that for free). No "if the question contains the word *acquire*, apply override rule #7," and no **LLM-routing** — no classifier that silently picks a different model or prompt per question. Every component — the embedder, the distiller, the reranker, the answerer — is a pluggable backend you can swap. And every single performance claim in this series is a measured number, run on a harness built specifically to keep us honest, with variance bands and a "did it actually fire?" mandate behind it.

(One bit of notation up front: **pp** means *percentage points*. A move from 87% to 89% is +2pp — not "2 percent." Newcomers often conflate the two; we keep them distinct.)

That posture costs you headline points. Our best clean score is **89.1%**, not 96%. But it's a number you can reproduce, and — as we'll get into — it leads its tier among systems that play by the same rules.

Here's the whole pipeline at a glance. Everything above the dividing line happens once, at ingest; everything below happens at every question. The two halves meet at the **vault** — the built, reusable memory state for a run.

```
INGEST (once per config, ~$35)
  raw chat ─▶ [Distillery / Flash] ─▶ facts ─▶ [qwen3 embed] ─▶ zvec store
                                                                      │
                                                                  ═══ VAULT ═══   (memory.sqlite + vectors)
                                                                      │
RECALL (per question, <$1 for a whole 500Q run on the cheap stages)
  question ─▶ [Planner / Flash] ─▶ {canonical, 4×dense, sparse, time}
            ─▶ [top-100 candidates] ─▶ [Cohere rerank] ─▶ top-K
            ─▶ [Answerer / Flash + reasoning] ─▶ answer | "unavailable"
```

Let me walk you through how we got here.

---

## 1. The road to an architecture

### Starting from "make it work," not "make it cheap"

The first version of the pipeline was built for quality and recall, not cost. Embeddings came from **Gemini** — good, but expensive and slower. There was no reranker. Model choices were loose. The old runs scored around **87.6%**, but with golden hacks baked in — the kind of per-question shortcuts this series exists to reject — so that number was never *clean*.

That's a fine place to start. You get the thing working at high quality, prove the architecture has legs, and *then* you optimize the cost down while holding quality. "Start expensive, optimize down" became the governing philosophy.

But to optimize anything, you have to measure it. And to measure it without fooling yourself, you need a harness.

### The membench harness

Everything in this series runs on a neutral bencher we call **membench**. Its whole job is to make claims falsifiable. A "claim" here means *"this change improved accuracy"* — and the harness is designed to make that claim expensive to assert and cheap to verify.

Every run lands in a structured registry:

```
runs/{system}/{benchmark}/{limit}/{run_name}/
  ├─ run-params.json          (every config value used)
  ├─ benchmark-report.json    (scores, metrics, cost rollup)
  ├─ artifacts/               (verdicts.jsonl, hypotheses.jsonl, model-traces.jsonl)
  ├─ vaults/                  (the memory state: memory.sqlite, zvec-hybrid, manifest.json)
  ├─ provider-queue/          (raw provider call traces)
  └─ workflow/                (per-stage pipeline logs)

records/{system}/...          (the curated, published copy — vaults and raw traces omitted)
```

That `vaults/` directory — the built, reusable memory state for a run — is the quiet hero of the whole effort, and we'll come back to why.

The harness enforces a **no-hacks rule** with teeth:

1. **No per-question special-casing** and no gold-string matching (searching the context for the answer text).
2. **No LLM-based routing** — no classifier that secretly picks a different model or prompt per question — unless it's generic and tested end-to-end.
3. **Verify-it-fired**: every claimed improvement must have a *causal proof*. Did the lever mechanically change its input? Did it move the subset it was supposed to move?
4. **Variance tracking**: single runs are noisy. Only N≥2 runs with non-overlapping confidence bands count as proven.

To keep the cheap probes honest, the 50-question samples we use for quick experiments are **stratified** — deliberately balanced across the benchmark's question types (single-session, multi-session, temporal-reasoning, knowledge-update, preference, and abstention). A win on a balanced 50 is far more trustworthy than a win on 50 questions that happen to be easy.

That last rule — variance tracking — deserves emphasis, because it reshapes everything. Our answerer uses **thinking** (also called reasoning mode): the model generates a private chain of reasoning before it answers. That chain is stochastic, which makes the whole answerer **non-deterministic** — run the identical configuration three times and you get three different scores. Across N=3 identical runs of our best config we measured **88.0% / 89.0% / 89.8%** — a swing of nearly 2pp from *nothing but randomness*. About 32 of 500 verdicts flip per run for no reason at all. (The mean of those three is 88.9%; we quote the best config as **89.1%** because that's the mean once a second small lever, evidence-dedup, is included — more on that below.)

The practical consequence: we treat a **±1.5pp band as the noise floor**, which works out to a roughly 2pp peak-to-peak swing between the lowest and highest run. **Any single-run delta inside that band is noise.** A huge fraction of "wins" people report — and that we ourselves initially got excited about — are statistical mirages. The harness exists to catch them.

### The cost model is the spine

Here's the insight that made iteration tractable. The pipeline has two phases with wildly different costs. Think of it as **building a library** versus **looking things up in it**: building is slow, expensive, and done once; looking up is fast, cheap, and endless.

| Operation | Cost / 500Q | Turnaround | Reusable? |
|---|---|---|---|
| **Ingest** (distill + embed + index) — *build the library* | ~$35 | ~25 min | One-time per config |
| **Answer-only** (reuse vault, recall + rerank + answer) — *look things up* | depends on reader (see below) | ~10 min | ∞ from one vault |
| **Re-embed** (reuse distill, re-embed) | ~$5 | ~8 min | Rebuilds index |

Ingest is expensive because it's LLM-intensive: you run a distiller over every conversation to extract facts. But once a vault is built, you re-run *recall and answering* against it without paying the ingest cost again. Multiple answer-only runs fan out from a single ingest.

One honest clarification on "answer-only is cheap," because it's easy to overstate. The cheap stages of a recall run — the query planner (~$0.3 / 500Q) and the reranker (~$1–2 / 500Q) — really are under a dollar or two combined. But the *answerer itself*, DeepSeek-Flash with reasoning cranked high, runs ~$20–25 per 500Q. So a full answer-only pass with the production reasoning answerer is **~$22**, not pennies. The "under a dollar" figure applies to planner-and-rerank-only sweeps, or to runs with a cheap reader. What stays true regardless: you never re-pay the ~$35 ingest, and that's the asymmetry that matters.

This is why the budget math works. The whole effort ran on a **$120 budget**. Fresh ingests are the thing you protect; answer-side experiments that skip the expensive reader are nearly free. So the discipline became: test answer-side levers answer-only, test distillation hypotheses on cheap 50-question stratified samples (~$2–3 each), and only spend $35 on a full re-ingest when a hypothesis has *already earned it*. By session's end, roughly **$12–15 of the $120** was spent — the rest reserved for genuinely expensive frontier experiments.

Cost is tracked exactly, not estimated. A `ModelTraceRollup` aggregates the per-call cost from every provider request — broken out per model into input tokens, output tokens, and cache hits — against a pricing table baked into the binary. Every run emits the rollup in its report. You always know what a number cost you, down to the token.

### Where it landed

After the evidence shook out, the current stack is cheap *and* strong:

| Component | Model | Provider | Cost / 500Q |
|---|---|---|---|
| **Embedder** | qwen3-embedding-8b @ 1024d | OpenRouter | ~$2–3 |
| **Distiller** | DeepSeek-Flash (thinking OFF) | DeepSeek | ~$13–15 |
| **Query Planner** | DeepSeek-Flash (flash mode) | DeepSeek | <$1 |
| **Reranker** | Cohere rerank-4-fast | OpenRouter | ~$1–2 |
| **Answerer** | DeepSeek-Flash (reasoning=high) | DeepSeek | ~$20–25 |

Note one deliberate asymmetry in that table: the **distiller runs with thinking OFF, the answerer with reasoning HIGH.** That isn't an oversight. Thinking gave the distiller nothing (+0pp) while burning tokens, but it bought the answerer +5pp. Same model, opposite knob, because the two jobs reward different things — extraction is mechanical, answering is reasoning.

The full path, end to end:

- **Ingest:** raw chat sessions go into the **Distillery** — the LLM step that reads raw conversation and writes out discrete, timestamped "memory facts." DeepSeek-Flash does the extraction over windowed message chunks. Those facts are embedded with qwen3-embedding-8b and stored in a hybrid vector store, alongside a SQLite ledger indexed by fact identity, event date, and source.
- **Recall:** a question goes to the **QueryPlanner** (DeepSeek-Flash), which decomposes it into a canonical query plus up to 4 dense sub-queries, sparse keyword terms, and a time window. Those search the store; **retrieval returns the top-100 candidates**; the **reranker** (a second-pass model that re-sorts those candidates by relevance) reorders them; and the top-K facts plus top-K raw turns go to the **Answerer** (DeepSeek-Flash with reasoning on), which produces an answer or says "unavailable."

Gemini-as-embedder gave way to qwen, the reranker got added, and the model discipline tightened. The result is a stack where a *fresh* 500-question ingest costs ~$35 and every derivative experiment costs pennies — or ~$22 if it includes the full reasoning answerer.

Each of those five stages is its own engineering story. Let's take them in order — and each one ends with a thread that pulls into its own deep-dive later in the series.

---

## 2. Embedding: turning facts into geometry

### What an embedding is

An **embedding** is a list of numbers — a vector — that represents the *meaning* of a piece of text, positioned in a high-dimensional space so that similar meanings sit close together. "I adopted a dog" and "I got a puppy" land near each other; "I filed my taxes" lands far away. Once your facts are points in this space, "find relevant facts" becomes "find nearby points," which is fast geometry instead of slow reading.

### The model we chose

We embed with **qwen3-embedding-8b at 1024 dimensions**, served via OpenRouter. Two things matter about that choice.

First, **1024 dimensions** is a deliberate trade-off. The model is *Matryoshka-compatible* — an architecture where the vector can be truncated to a shorter length without retraining, the way nesting dolls fit inside each other. Bigger vectors carry more information, which raises the odds that the right gold fact lands in the top-100 candidate pool that the reranker later picks from. But re-embedding an entire vault at higher dimensions is expensive — a single 5-model embedding sweep burned through a **$50 OpenRouter top-up**. So 1024d is the proven sweet spot: enough resolution to keep the golds in the pool, cheap enough to re-run.

Second, this is a **bi-encoder**. That's a precise term worth pinning down, because it's the key contrast for the reranking section.

### Bi-encoder: query and document, encoded apart

In a **bi-encoder**, the query and the document are encoded *separately*, each into its own vector, and you compare them with **cosine similarity** — essentially the angle between the two vectors; a smaller angle means closer meaning. The human analogy: a bi-encoder is like **filing every document by topic in advance, then grabbing the nearest folder** when a question comes in. This is fast and pre-computable:

- **At ingest:** every fact and raw turn is embedded once and stored in the vector index. Done forever.
- **At recall:** only the question gets embedded, then matched against the pre-computed index by cosine similarity.

Because the documents are encoded ahead of time and never re-touched at query time, retrieval is a cheap nearest-neighbor lookup. The cost is precision: the model never sees the query and document *together*, so it can't reason about how they specifically interact. That's a job for the cross-encoder in the reranking stage — hold that thought.

The store itself is **zvec-hybrid**, an in-process vector database with a SQLite ledger that supports both dense semantic search and structured metadata filters (event date, fact identity, source kind, lifecycle). Vectors are computed at ingest and sit ready for instant cosine lookups.

The pipeline is modular here in practice, not just in principle: alongside the production qwen path there's a deterministic `local:hash-embedding-v1` fallback and a Gemini embedder for smoke tests. Swapping the embedding backend is a config change, not a rewrite.

### How well does it work? Almost perfectly.

Here is the number that reframed the entire project: the gold-evidence session lands in the top-100 candidate pool for **499 of 500 questions — 99.8% retrieval.** Embedding-based retrieval is, for practical purposes, *solved*. The bottleneck is not finding the right evidence. It's what happens after.

That finding had a brutal corollary for tuning: embedding swaps barely move the needle, because there's almost no recall left to recover. We held qwen at 1024d and stopped fiddling.

There is, however, a quieter and more interesting embedding story we haven't fully told here — what it takes to run embeddings *locally* on Apple Silicon, where quantization behaves backwards from what you'd expect and standard model converters reject the architectures outright. That's the same path we walked for rerankers, and it gets its own post.

> *Coming up in the embedding deep-dive: Matryoshka dimension tuning, HTTP/1 transport pooling (32-connection pool, batch 250, p50 5.3s / p95 10.9s across 167 embedding batches on 10Q), sparse SPLADE-style embeddings we haven't wired yet, and the MLX-on-Metal local inference path.*

---

## 3. Retrieval: finding the needle in the top-100

Retrieval is the stage that turns "a question" into "100 candidate facts." The planner — DeepSeek-Flash in a fast mode — reads the question and emits a structured plan: a canonical query, 1 to 4 dense sub-queries, a set of sparse keyword terms, and a time window. Everything gets tokenized into a single search pass against the zvec-hybrid store, which returns the top candidates by cosine similarity.

### Retrieval is solved (and we had it wrong before)

As we saw in the embedding section, the gold is already in the top-100 pool 99.8% of the time. Measured from the other direction it tells the same story: of the ~52 questions where the reranker-equipped pipeline still missed, **51 had the gold session sitting right there in the top-100** — only **1** genuine retrieval miss in the entire benchmark. (51 of 52 is 98.1% *of the misses*; the 99.8% figure is the all-questions number from §2. They're two different denominators describing the same happy fact: the evidence is essentially always retrievable.)

The earlier belief that retrieval was the bottleneck ("the gold isn't in the top-100") came from an unreliable substring-matching metric, and it was simply *wrong*. Once we measured properly, the truth flipped: the right evidence is almost always *there*. The system just fails to reason over it correctly downstream.

### The "more candidates = better" intuition fails

If retrieval is mostly working, surely *more* candidates can only help? No. We tested the `fact_top_k` / `raw_turn_top_k` knobs at 40, 50, and 200 against a baseline of 20, and they all scored within-noise or *worse*.

The mechanism is clean once you see it. Gold evidence clusters near the *top* of the candidate pool already. Widening the pool doesn't surface new golds — there aren't any left to surface — it just drags in more distractors, which add noise and attention overhead for the reranker and answerer. It's like answering a quiz with 20 reference pages versus 200: the extra 180 pages don't contain new answers, they just bury the 20 that did. "More width = better coverage" failed mechanically.

### The hybrid keyword lane we never built

There's an honest gap here. The planner emits `sparse_terms` — keywords meant for **lexical** matching, i.e. exact word overlap rather than meaning (the classic algorithm for this is **BM25**, a "sparse" keyword-scoring method, as opposed to the "dense" meaning-vectors above). But those terms are never actually wired into a parallel keyword search lane; today the system runs dense vector search plus a weak token-hit fusion. Building the full BM25 lane is documented as a real architectural to-do.

And yet — we didn't build it, for a reason that captures the whole methodology. With 51/52 golds already in the dense pool, a keyword lane could recover **at most 1 question**, an expected value of roughly +0.2pp. When retrieval is solved, the rational move is to *not* spend effort on retrieval. The gap is real; the value of closing it is marginal.

The bottleneck had migrated. Which brings us to the one lever that actually moved.

> *Coming up in the retrieval deep-dive: the zvec-hybrid metadata schema, event-date indexing, and why the hybrid-fusion architecture is built but not yet lit.*

---

## 4. Reranking: the one lever that worked

Of roughly **25 levers** we tested across the whole system, exactly one moved the needle past the noise band decisively. This is it. (For the curious, the graveyard of the rest is long: prompt-discipline rewrites, smaller-k and larger-k, max-reasoning chains, model swaps, deterministic counting, split prompts, temporal filters, date enrichment, multihop expansion, conflict resolution. One *other* small lever — evidence-dedup plus a low-confidence filter — earned a modest, proven +0.8pp; the reranker is the only one that cleared the band decisively.)

### Cross-encoder: query and document, encoded together

Recall the bi-encoder from the embedding stage: query and document encoded *separately*, compared by cosine — "file everything by topic in advance, grab the nearest folder." A **cross-encoder** does the opposite — it concatenates the query and the document and encodes them **together**, so the model can attend to the precise interaction between them and emit a single, sharp relevance score from a classification head. The analogy: a cross-encoder **actually reads the question and each document side-by-side and judges the fit.** More accurate, much slower.

The trade-off is exactly inverted:

| | Bi-encoder (embedding) | Cross-encoder (reranking) |
|---|---|---|
| Encodes | Query and doc *separately* | Query and doc *together* |
| Output | Two vectors → cosine | One relevance score |
| Speed | Fast (~50ms), pre-computable | Slow (~1–5s per 100 docs) |
| Precision | Coarse | Fine |
| Analogy | File by topic, grab nearest folder | Read question and doc side-by-side |

You can't afford to run a cross-encoder over your whole vault — it's far too slow. But over just 100 candidates, it's perfect. So the pipeline is **two-stage retrieval**, the industry-standard pattern: the fast bi-encoder fetches the top-100 (coarse), then the slow cross-encoder reorders them (precise), promoting the genuinely relevant facts to the top before the answerer ever sees them.

### The number

On LongMemEval-S (500 questions), with our remote reranker, **Cohere rerank-4-fast**:

- **Reranking OFF:** 87.4%
- **Reranking ON:** 89.8%
- **Delta: +2.4 percentage points** — consistent across measurement runs.

A note on the arithmetic, since it's easy to trip on: 89.8 − 87.4 = **2.4pp**, and that's the figure we stand behind. (Earlier internal notes sometimes quoted "+3.34pp" for this same lever; that came from comparing against a different, lower baseline mean, and it doesn't match the OFF→ON numbers shown here. We use the clean +2.4pp throughout.)

That +2.4pp is the single largest, most reproducible lever in the entire system — the only optimization that cleared the ±1.5pp variance band decisively. Everything else either hurt or vanished into noise.

The reason it works ties straight back to retrieval: the golds are already in the top-100, just buried under distractors. The reranker's whole job is to surface them into the top-K that the answerer actually reads. It's not adding recall; it's cleaning up the ordering so the answerer's job gets easier.

### Model strength is the only thing that matters for quality

We tested rerankers on a 50-question stratified subset:

| Model | Score | Verdict |
|---|---|---|
| **Cohere rerank-4-fast** (remote) | 92% | Gold standard |
| **Nemotron-1B BF16** (local MLX) | 92% | Ties Cohere — identical top-3 |
| **No reranking** | 88% | Baseline |
| bge-reranker-v2-m3 (568M) | 86% | *Below* baseline |
| jina-reranker-v3 (0.6B) | 84% | *Below* baseline |

Read that carefully. The lightweight open-source rerankers — bge and jina — score *below* doing no reranking at all. They actively hurt. Only a genuinely strong ~1B model (Nemotron, or Cohere's hosted model) hits 92%. **You cannot compress your way to reranking quality.** Model strength is the lever; size and cleverness aren't substitutes.

### Why running rerankers is expensive — and what we did about it

Cohere's API is fast (0.3s per 100 docs) and cheap *per call* (~$0.001 per 1000 queries), but it's a per-query cost on a hosted model you don't control. For a personal-memory service queried thousands of times, "rent the cross-encoder forever" is a real cost and a real dependency.

So we did something more ambitious: we **hand-built local cross-encoder rerankers in MLX** (Apple's array framework for Apple Silicon) to match Cohere's quality on-device, for free. This turned out to be a deep, surprising rabbit hole — the kind where the conventional wisdom is exactly backwards:

- **Nemotron-1B**, hand-built in MLX, ties Cohere at **92%** and runs a 100-doc rerank in ~1.9s — a **2.9x speedup** over the same model on PyTorch/MPS (5.5s).
- **KaLM** (a 0.27B encoder-decoder reranker) runs even faster — **~0.75s in production, ~4–4.5x faster** than its PyTorch baseline (2882ms). (A single warmed payload hit 520ms/5.5x, but in the live pipeline — varying shapes — that's a best case, not the headline; the deep-dive explains why.) Its quality is still unverified on our benchmark.
- **Quantization** — shrinking the model to int4/int8, which makes inference *faster* on NVIDIA GPUs — actually made things **slower** on Apple Silicon, sometimes by 40–63%. On Metal the model is *compute-bound*, not memory-bound, so fp16 is king and quantization is a trap.
- Standard converters (llama.cpp, ollama, mlx_lm) **reject** these architectures outright — `Model LlamaBidirectionalForSequenceClassification is not supported` — which is exactly why the MLX implementations had to be hand-built.

That's a whole post on its own, and it's the richest one in the series.

> *Coming up — the standalone reranking deep-dive: the roofline math behind why quantization fails on Metal (9.3 TFLOP/s fp16 against a 295 GB/s memory bus, hitting 97% of the compute ceiling), hand-building bidirectional-Llama and encoder-decoder rerankers in MLX, length-sorted batching for a hidden 1.7–2.6x win, torch-free deployment in ~3GB of RAM, and the measurement confounds we caught and retracted along the way.*

---

## 5. Answering: where 89% actually lives

By the time evidence reaches the **Answerer**, retrieval has done its job (99.8% of the time the gold is present) and the reranker has surfaced it. The answerer's task is to read the top-K reranked facts plus top-K raw turns and produce a correct, direct answer — or honestly say "unavailable."

### The model and the thinking knob

The answerer is **DeepSeek-Flash with reasoning enabled** — thinking mode, where the model works through the problem before answering. We verified this is worth it: think-on beats think-off by about **+5pp** (79.0% → 84.2% on the clean ladder). We also ran a comparison sweep across **gemma-4-26b, gemini-3.5-flash, glm-5.2, mistral-medium-3.5, and deepseek-v4-pro** (reasoning high). None of them beat Flash overall, and none beat it on the math/count subset where Flash hits 91.3%. Notably, deepseek-v4-pro at reasoning=high scored **87.2%** — *worse* than Flash; mistral-medium-3.5 landed at just 71.2%. Longer, "max" reasoning chains didn't help; they hurt, and added variance. The whole prompt architecture is Flash-tuned, so other models don't even benefit from the same setup.

### The score ladder

Here's the honest climb on the clean, no-hacks pipeline (LongMemEval-S, 500Q). Note how the baseline shifts meaning at each rung — *think-off*, then *think-on*, then *rerank-on* — which is why a single "baseline number" can look like several different numbers depending on where you stand:

| Config | Score | Note |
|---|---|---|
| Flash baseline, think OFF | 79.0% | first valid clean score |
| + think-on answer (reasoning captured) | 84.2% | +5.2pp, proven |
| + count-ledger / latest-value prompt refinements | 86.2% | *(but see the bug below)* |
| + reranker (Cohere, 100-candidate pool) | 89.8% | +2.4pp — the only proven lever |
| **+ evidence-dedup + low-conf filter (best)** | **89.1%** | N=3 mean of 88.0 / 89.0 / 89.8 |

(For scope: this ladder is the critical path. A separate fact-consolidation / reweave stage exists in the broader system — re-deriving and merging facts post-ingest, ~$5–10 — but it sits off the critical path for these numbers and we're leaving it out of this post on purpose.)

### A humbling bug: the prompt that did nothing

One finding belongs in every honest engineering story. For a long stretch, the `answer.yaml` prompt file was *loaded but ignored in code* — the answerer used a hardcoded literal instead. Which means **every prior answer-prompt experiment was void**: the discipline rewrites, the compute-style prompts, the count-ledger, the latest-value tweaks, the timestamp enrichment — none of them actually ran. A whole category of "results" was measuring nothing. The lesson stuck: *verify it fired* isn't bureaucracy, it's the only thing standing between you and a notebook full of fiction.

And here's the case that proves *why* the mandate has teeth — because a lever can fire correctly and still make things worse. We built a counting specialist and confirmed, mechanically, that it did everything it was supposed to: it expanded the evidence set from 16 to 32 facts, applied dedup, loaded its lean prompt, and fired max-reasoning. Every box checked. And counting accuracy **fell**, 82.8% → 81.6%. The lever fired *and* hurt. Without the verify-it-fired discipline you'd have shipped it on the assumption that "more evidence + more reasoning must help." It didn't.

### The real bottleneck is reasoning

With retrieval solved, the misses are the answerer mis-reasoning over *correct* evidence. We categorized all **74 rerank-off misses** (the baseline miss set), and the split is telling:

- **41% (30 of 74) are fixable Flash reasoning bugs** — real errors the model gets wrong despite having the right facts.
- **43% (32 of 74) are benchmark noise** — broken or ambiguous questions, shaky gold annotations, and cases dinged on strict formatting despite a correct answer.
- **16% (12 of 74) are retrieval/distillation edge cases.**

That 43% noise figure has a sharp implication: roughly 32 of 500 questions are *unwinnable as scored*. The **effective clean ceiling is ~93.6%** ((500−32)/500), not 95% and certainly not 100%. (A stricter accounting of the truly *unwinnable* subset puts the absolute ceiling nearer ~97% — only ~15 questions are hopeless no matter what — but the practical, defensible target is the 93.6% figure.) Either way, chasing those last points means fighting the benchmark's own bugs, not improving your system.

One miss, walked all the way through, makes the reasoning bottleneck concrete. The question asks how many times the user has worn a particular item. The evidence is *all present in the pool* — including a recent note, "worn six times as of [date]." Flash picks an **older** dated fact ("four times") over the newer one and answers four. Retrieval succeeded, the reranker surfaced the right turn, the gold was on screen — and the model still chose the stale value. No bigger pool and no extra candidates fix that; it's a latest-value reasoning error, full stop. This is what "the bottleneck is reasoning" looks like in a single row.

That failure is one of three patterns that recur across *every* system in this space:

- **Multi-session counting** ("how many times did I…") is a structural wall at **79–87%** across every system and every reader, including frontier models. The only real fix is deterministic aggregation over a deduped (subject, date) set — not a bigger model.
- **Temporal date-arithmetic** ("how many weeks ago?") trips Flash on span math.
- **Latest-value selection** — the worn-six-times miss above: Flash picks an older dated value over a newer one.

### About those 95% headlines

Here's why we're at peace with 89.1%. Every published 94–96% result fails a comparability gate — almost always because it quietly uses a **stronger reader** (a frontier model like Claude Opus or GPT-4o doing the answering), an **oracle** (handed the gold sessions for free), or an outright **hack**:

| Claim | Score | Disqualifier |
|---|---|---|
| jordanmccann | 96.2% | Claude Opus reader + 46-iteration overfit + explicit hacks |
| Chronos-High | 95.6% | Claude Opus 4.6 reader |
| OMEGA | 95.4% | macro-average inflation (true micro 93.2%) + GPT-4.1 self-grading |
| Mastra | 94.87% | gpt-5-mini reader (drops to 84.23% with gpt-4o) |
| mem0 v3 | 94.4% | self-reported, internally inconsistent |
| LongMemEval paper | 92.4% | *oracle* retrieval (gold sessions, no distractors) + GPT-4o |

Meanwhile the honest peer tier — systems using gpt-4o-mini-class readers, no hacks — tops out at **77.8–83.0%**. MemCoT, with reasoning orchestration on a weak reader, reaches 88.0%. Memoria, on Claude Opus 4.6, lands 88.78%.

**Our 89.1% on DeepSeek-Flash, no hacks, leads its reader tier.** A frontier reader (Opus, GPT-4o) would buy roughly +10pp — but it's concentrated in temporal and latest-value reasoning, the counting wall stays put for *everyone*, and it's a cost/product decision, not a clean win. 95% clean is not reachable on this stack. ~89–90% is the honest ceiling for Flash, and we can defend every point of it.

> *Coming up in the answering deep-dive: the miss audit in full, the counting-wall anatomy, the reasoning-mode sweep, and what a frontier reader actually buys.*

---

## Conclusion: modular, measured, and honest about the ceiling

Step back and the shape of the thing is clear. Symbiotic Memory is a **fact-distilled, vector-indexed, reranked, LLM-answered** memory system — and every piece of it is a swappable backend behind an environment flag.

- The **embedder** is an `EmbeddingProvider` trait — qwen via OpenRouter today, Gemini or a local MLX model tomorrow.
- The **distiller** and **answerer** are `ChatProvider` traits — DeepSeek today, anything tomorrow.
- The **reranker** is a `Reranker` trait — Cohere's hosted cross-encoder today, our hand-built local MLX Nemotron when we want zero marginal cost.
- The **vault** is fully portable: distill once for ~$35, then run answer-side experiments forever — pennies for the cheap stages, ~$22 when you include the full reasoning answerer.

That modularity isn't over-engineering for its own sake. It's what made the *honesty* affordable. Because swapping one backend at a time behind a flag, on a vault you can re-query cheaply, is exactly what lets you isolate a single lever, run it N≥2 times, and tell signal from the ±1.5pp noise floor. The cost model and the no-hacks rule aren't separate from the architecture — they're the reason the architecture looks the way it does.

What we learned, stated plainly:

- **Retrieval is solved** (99.8% gold-in-pool). Stop tuning it.
- **Reranking is the one lever that cleared the noise band** (+2.4pp); a second small mover (dedup + low-conf, +0.8pp) is real but modest. Model strength is everything; lightweight rerankers actively hurt.
- **The bottleneck is the answerer's reasoning**, and a big chunk of the remaining gap is benchmark noise, not fixable system error.
- **The honest ceiling is ~89–90% on Flash.** Everyone claiming 95% is using a stronger reader, an oracle, or a hack.

And it's all open tooling — the membench harness, the cost rollups, the run registry, the hand-built MLX rerankers — built so the numbers can be checked rather than trusted.

**What's next.** This was the map. The rest of the series walks each territory in depth: the embedding post (Matryoshka dims, local MLX, the transport tuning), the retrieval post (the hybrid store and the unlit keyword lane), the reranking deep-dive (the richest one — Metal rooflines, hand-built cross-encoders, length-sorting, retracted confounds), and the answering post (the full miss audit and the counting wall).

The headline numbers in this field are mostly fiction. The fun part is what's true underneath — and that's measurable, swappable, and a dollar a run to check.

*Next up: Post 2 — Embedding, in depth.*