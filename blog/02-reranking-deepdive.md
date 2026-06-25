The final version is written to `/Users/k/p/symbiotic-mem-bench/reranking-deep-dive.md`. Below is the published markdown.

---

# Reranking, From Scratch: How We Matched Cohere Locally for Free

*A companion deep-dive to Post 1. This is the specialized one — the story of building and tuning a local reranker that ties a bleeding-edge commercial API on accuracy, runs torch-free on a Mac, and stays swappable so we never get locked in. If you are new to retrieval, the early sections define every term as it shows up. If you are not, skip ahead to the MLX ports — that is where it gets weird.*

---

## The one lever that actually moved

We're building a personal-memory system — software that remembers facts from your past conversations so an assistant can recall them later. To tune it, we score it against **LongMemEval-S**, a 500-question benchmark that asks, in effect, "did the assistant remember the right thing from a long conversation."

One thing you have to internalize before any number in this post means anything: **benchmarks like this are noisy.** Re-run the exact same configuration and the score wobbles, because the answering model isn't deterministic — about 32 of the 500 verdicts flip from run to run. Three identical-config runs of our pipeline scored **88.0% / 89.0% / 89.8%**. That spread defines a **±1.5 percentage-point (pp) noise band**, and a "win" only counts if it clears that band. (Note: percentage *points*, not percent — going from 87% to 89% is +2pp, not +2%. The distinction matters when you're being honest about small gains.)

Over the project we tested somewhere around 25 levers — prompt variants, smaller candidate sets, max-reasoning modes, model swaps, deterministic counting, split prompts. **Almost all of them landed inside that ±1.5pp band**, which is a polite way of saying they did nothing.

One lever moved the needle and kept moving it across every run:

| Config | Accuracy (500Q) |
|---|---|
| Reranking OFF | 87.4% |
| Reranking ON (Cohere rerank-4-fast) | 89.8% |
| **Delta** | **+3.34 percentage points (pp)** |

That +3.34pp is the single largest reproducible improvement in the entire system. It showed up consistently, run after run, while everything else was busy adding noise. So this post is about that lever: what it is, why it works, and the long road to making it run locally for free without losing a single point of accuracy.

Let's start with what reranking even is.

---

## Embeddings, bi-encoders, and the coarse first pass

When you search a big pile of documents semantically — by *meaning* rather than keyword — you almost always start with **embeddings**. An embedding is just a list of numbers (a vector) that a model produces to represent a piece of text, positioned so that texts with similar meaning land near each other in that numeric space. "I love my dog" and "my puppy is the best" end up close; "quarterly tax filing" ends up far away.

The model that produces these is a **bi-encoder** (also called an embedding model). The "bi" matters: it encodes the query and each document **separately**, into their own vectors, and then you compare them with a cheap math operation (cosine similarity). Because the documents can be embedded once, ahead of time, and stored, searching is fast — roughly 50ms to score a query against a large index. The cost of that speed is precision: the model never sees the query and the document *together*, so it can only judge them through the blurry lens of two independently-made vectors.

In our pipeline, embedding-based retrieval does the first pass: from a memory store of distilled facts, it fetches the **top 100 candidates** for a question. Fast, coarse, good enough to make sure the right answer is *somewhere* in the pile. In fact, retrieval was essentially solved — **51 of 52** of our reranking-ON misses still had the correct gold answer sitting in that top-100 pool (and measured end-to-end, the gold-evidence session was present for 499/500 questions — 99.8%). The problem was never *finding* the evidence. It was getting the *right* piece to the top, where the answering model would actually read it.

That second job — reordering the pile so the best evidence rises — is reranking.

---

## Cross-encoders, and why they're slower but smarter

A **reranker** is a **cross-encoder**. Instead of encoding the query and document separately, it concatenates them — query *and* document, glued together — and runs them through the model as a single input. The model attends to both at once, sees exactly how the words in the query relate to the words in the document, and emits one number: a relevance score from a small classification head bolted onto the top of the network.

The intuition: a bi-encoder is like judging two résumés separately and comparing scores; a cross-encoder is like interviewing both candidates in the same room — slower, but it sees how they actually relate.

That joint view is why cross-encoders are far more precise than embeddings. It's also why they're slow: there's no precomputing. You can't encode a document "once" because its score only exists relative to a specific query. Every (query, document) pair is fresh work — roughly 1 to 5 seconds for 100 documents, versus ~50ms for an embedding lookup.

So the industry-standard pattern, and ours, is **two-stage retrieval** — like a hiring funnel: a cheap résumé screen over thousands, then an expensive interview for the top few.

1. **Embed and fetch** the top-100 — coarse, fast, ~50ms.
2. **Rerank** those 100 — precise, slow, ~0.3–5s — and reorder so the best evidence is on top.

You get the speed of embeddings over the whole corpus and the precision of a cross-encoder over the shortlist. That's the +3.34pp. The right evidence was already in the pool; the reranker is what dragged it into the top few slots (the "top-K") the answerer actually reads.

Reranking works. The question is what runs it — and the easy answer has a catch.

---

## The cost problem (and the economics that make any of this tractable)

The easy answer is a hosted API. Cohere's `rerank-4-fast` is the gold standard here: 92% accuracy on our stratified test set (more on "stratified" below), 0.3 seconds per 100 documents, and a price of roughly **$0.001 per 1,000 queries**. On paper that's almost free.

To see why "almost free per query" still isn't the whole picture, it helps to know how the rest of the system spends money — because the cost structure is the entire reason this project was tunable at all.

A memory pipeline has two phases. **Ingest** is the one-time, expensive phase: raw chat logs get distilled into facts by an LLM, then embedded and stored. **Recall** is the cheap, repeatable phase: a question comes in, gets planned and embedded, the top-100 are fetched and reranked, and an answerer reads the result. The dollar figures, per 500-question benchmark:

| Phase / component | Cost | Notes |
|---|---|---|
| **Ingest (one-time)** | **~$35** | distill ~$13 + embed ~$2–3, plus overhead |
| Answerer (per full run) | ~$20–25 | the dominant recall cost |
| Reranker (per full run) | ~$1–2 | the lever |
| Query planner (per full run) | <$1 | |
| Embedder | ~free | |
| **Answer-only rerun** | **<$1 to ~$22** | reuses the ingested store; planner ~$0.3 + rerank ~$1.5 + answer ~$20 |
| A 50Q diverse probe | ~$2–3 | the fast iteration unit |

The shape that matters: **ingest is paid once (~$35); after that, re-running the recall side reuses the stored facts indefinitely.** An answer-only rerun that touches only the cheap components can cost **under a dollar**. That cheap-first asymmetry — pay $35 to build the store, then iterate on it ~infinitely at <$1 a probe — is what made it possible to test 25 levers at all. Our total session budget was **$120**; we spent roughly **$12–15** and finished with **$105+** remaining. (Cost is tracked per-call in code via a built-in pricing table — `src/cost.rs`, `ModelTraceRollup`, `cost_micro_usd` — so every run reports exactly what it spent.)

Now the catch with Cohere. "Almost free per query" hides three structural costs that a price tag doesn't show. Rerankers are heavy models, and *every* recall in a persistent personal-memory product hits the reranker. An API line item that's modular today becomes a dependency you **can't tune, can't run offline, and can't swap** without a vendor migration. And the reranker being the *one proven lever* means it's also the one component you least want to outsource permanently.

So the goal crystallized: **match Cohere's 92% locally, for free, and keep it swappable.** Remote when you want it, local when you want it, same interface either way. That sounds tidy. The path there was not.

---

## Model strength is the lever (and lightweights don't help)

The first instinct when going local is to reach for a small, popular open-source reranker. There are good ones — bge, jina — and they're light enough to run anywhere. We tested them honestly on a fixed set of 50 **stratified** questions — i.e. sampled to mirror the category mix of the full benchmark, so a small set still represents it fairly (answer-only, noise band ±6–7% at this sample size):

| Model | Score | Verdict |
|---|---|---|
| Cohere rerank-4-fast (remote) | 92% | Gold standard |
| **Nemotron-1B BF16 (MLX, local)** | **92%** | **Ties Cohere** |
| No reranking | 88% | Baseline |
| bge-reranker-v2-m3 (568M) | 86% | Below baseline |
| jina-reranker-v3 (0.6B) | 84% | Below baseline |

Read that table twice, because it's the whole ballgame. The lightweight open-source rerankers scored **below the no-reranking baseline**. bge at 86% and jina at 84% are not "a bit worse than Cohere" — they are *actively worse than doing no reranking at all*. Putting a weak cross-encoder in front of your evidence reorders it confidently in the wrong direction.

The only local model that matched Cohere was NVIDIA's **Nemotron-1B** — and it didn't just match, it tied exactly: 92%, identical 46/50, same top-3 ranking on the documents. The finding is blunt and it generalizes: **model strength is the only lever that matters for reranker quality.** You cannot compress your way to 92%. A 568M model is a 568M model. To match a frontier API you need a genuinely strong ~1B cross-encoder. There's no clever trick that makes a weak reranker good; there's just the model.

That settled *which* model. It did not, unfortunately, give us any way to *run* it.

---

## Why nothing would run it: the architecture wall

Here is where the project stopped being about retrieval and became about systems engineering.

The convenient local-inference runtimes on Apple Silicon are `llama.cpp` (and its wrapper `ollama`) and `mlx_lm`, Apple's own LLM toolkit for its MLX array framework. (**MLX** is Apple's NumPy-like array library that runs natively on the Mac GPU via Metal, Apple's graphics/compute API.) Between them they run essentially every popular open model. So we tried to convert Nemotron.

It got rejected outright. `convert_hf_to_gguf.py` — the standard tool for turning a HuggingFace model into a GGUF file that llama.cpp can load — returns:

```
ERROR: Model LlamaBidirectionalForSequenceClassification is not supported
```

The reason is architectural. Nemotron isn't a normal Llama. It's a `LlamaBidirectionalForSequenceClassification`: a Llama backbone modified to use **bidirectional attention** (every token can see every other token, like an encoder, instead of the causal "only see the past" masking that text-generation models use) with a **sequence-classification head** on top (the small linear layer that emits the single relevance score). Standard runtimes only know causal Llama. They have no path for a bidirectional one.

`mlx_lm.convert` failed the same way — it rejects `model_type: llama_bidirec`. And the faster, more efficient reranker we also wanted to try, **KaLM** (more on it below), is a `t5gemma2` encoder-decoder with *merged self+cross attention* — an architecture so non-standard that no off-the-shelf runtime, MLX or otherwise, supports it. `convert_hf_to_gguf.py` rejects it too.

So the menu was: run these in PyTorch (which works, but is slow — about 5.5s end-to-end for Nemotron, 2.9s for KaLM, both far from Cohere's 0.3s), or build the inference path by hand in MLX. There was no third option. No runtime in existence would load these models on a Mac with any speed.

We hand-built them.

---

## Building Nemotron in MLX, by hand

Hand-building a model's inference means *reimplementing the forward pass* — the sequence of math operations that turns input tokens into an output score — using MLX primitives, then loading the original trained weights into your reimplementation. Get any of it subtly wrong and the model still runs; it just gives quietly wrong answers. So the whole exercise lives or dies on validation.

Concretely, the forward pass we rebuilt looked like this (skip the bullets if you're not implementing one yourself):

- Load the fp16 safetensors weights via `safetensors.load()` in `pt` (PyTorch) format. (Detail that cost real time: the mlx and numpy loaders can't decode bf16 tensors, so you have to go through the pt path.)
- `embed_tokens` to turn token ids into vectors.
- A manual for-loop over the transformer layers, each with **bidirectional** masking instead of causal — this is the part no runtime supports.
- A final `norm`.
- **Masked-average-pool** over all tokens — average the per-token vectors, respecting the attention mask so padding doesn't count — to get one vector per document.
- The score head: `score.weight.T @ pooled`, a single linear layer with weight shape `[1, 2048]`, temperature 1.0.

Then we validated it against the PyTorch reference, which is the only thing that makes any of this trustworthy:

- **Single document:** P(yes) of 0.14934 in MLX vs 0.15001 in PyTorch. A difference of **0.0007** — numerically identical, the gap is just floating-point accumulation order.
- **Batched 100-document ranking:** top-3 documents identical, `[2, 26, 0]` in both.

That 0.0007 is the number that lets you trust everything downstream. The MLX port is the same model. From there it was about speed.

---

## The optimization saga, and the discipline that kept it honest

**If you don't care about the millisecond-level detail, the takeaway is one line — *only length-sorting worked; everything else was measurement noise or worse.* The rest of this section is the evidence.**

This is the part of the project I'm proudest of, and it's not because of any one clever optimization. It's because of how many "obvious wins" turned out to be nothing — and how we caught that they were nothing.

Reranking on a Mac inverts a lot of NVIDIA intuition, so most of the standard tricks fail, and they fail in ways that look like wins if you measure carelessly. The discipline that saved us was simple and paranoid: **every claimed speedup gets re-measured in a clean, isolated process, or it doesn't count.** Here's what that bought us.

### The one real win: length-sorting

The documents being reranked vary wildly in length — from 11 to 139 tokens. When you batch them, every document gets padded to the length of the longest one in the batch, so a batch of 100 pads everything to 139 tokens. The model then does the full computation on those padding tokens too, even though they carry no information — pure waste. For the short documents that's roughly **4x wasted compute**.

The fix is to **sort candidates by length before batching**, then process them in batches of 25. Similar-length documents end up grouped, so padding waste collapses. The numbers:

| | Unsorted B=100 | Sorted B=25 | Win |
|---|---|---|---|
| Nemotron full rerank | 2387 ms | 1925 ms | ~20% |
| KaLM encode | 808 ms | 312 ms | 2.6x |
| KaLM full rerank | 1325 ms | 764 ms | 1.7x |

Length-sorting is a real, generalizable lever — it isn't specific to any model, it's just a consequence of how padding works. It's also the *only* speed optimization in this whole saga that survived clean measurement. Everything below is a corpse.

### Dead lever: quantization

**Quantization** means storing the model's weights at lower numeric precision (int8, int4) to make them smaller and, on the right hardware, faster. On NVIDIA GPUs it's a classic win, because generation there is memory-bound and int4 tensor cores are fast.

On Metal it's the opposite. We measured it both ways and it's a clear loss:

| KaLM stage | fp16 | int8 | int4 |
|---|---|---|---|
| Encode | 625 ms | 1002 ms (−60%) | 1018 ms (−63%) |
| Decode | 392 ms | 558 ms (−42%) | 551 ms (−40%) |

For Nemotron the story repeated. We tried several quantization schemes — the names (`affine8`, `affine4`, `mxfp8`, `mxfp4`) just denote int8 vs int4 and different rounding methods: affine8 at 2.54s, affine4 at 2.30s, mxfp8 at 3.10s (which *also* reordered the ranking, i.e. changed the answers), mxfp4 at 2.78s with score drift from 7.2 to 10.3 — all slower than fp16's 2.25s.

To understand *why*, we did a **roofline analysis**. A roofline analysis asks a simple question: is the model waiting on math (**compute-bound**) or waiting on weights to arrive from memory (**memory-bound**)? That distinction decides whether quantization can possibly help — because quantization only shrinks the weights, so it only helps if you were waiting on memory. The answer: at batch=1, the Nemotron forward is **97% of the compute ceiling** (34ms of actual compute against a 33ms theoretical floor), and the weights load in just 1.7ms of a 308ms forward. It's compute-bound, not memory-bound. Metal has no fast int4 matmul path, so dequantization overhead just gets added on top of an fp16 matmul that was already fast. Quantization *cannot* help here, and the measurements prove it can only hurt.

### Dead lever: compilation, concurrency, precompute

- **`mx.compile`** (MLX's graph compiler, which fuses operations into faster kernels): +4% on encode, +4% on decode — both inside a ±15–20% noise band. Its real value turned out to be *tightening variance* (3x narrower timing ranges), not raising speed. Not a speed lever.
- **Concurrency:** running 4 decode threads in parallel gave **0.87x** — slower. Python's GIL (the global lock that lets only one thread run Python at a time) serializes the CPU-side dispatch, and MLX uses a single GPU stream, so the threads just queue behind each other with extra overhead.
- **Precompute / pre-resolve** (precomputing positional-encoding tables, pre-resolving weights): −4%, actually slower. Noise.

Four ideas, all of which sound like they should help, all measured, all excluded.

### The confounds we caught, and the claims we retracted

The reason I trust the table above is that we got *burned* by bad measurement first, caught it, and retracted the bad numbers in our own notes:

- **A lingering server.** An old PyTorch KaLM server (pid 78834) was running on the GPU during a batch of "clean" MLX benchmarks, inflating the absolutes by ~20–30%. We killed it and re-measured everything. All pre-kill absolutes were retracted.
- **Cross-process compile contamination.** The first run's warmup was conflating `mx.compile`'s one-time tracing with the actual timing, producing an absurd "+16% then −14%" swing across runs. The fix was to isolate every stage in its own process. That's when compile resolved to its real, boring +4%.
- **PyTorch coexisting in GPU memory.** A "+626ms precompute win" turned out to be an artifact of a PyTorch model sharing GPU memory during the test. Same-process MLX-vs-MLX showed −4%. Retracted.

Three specific claims got struck: a "+13% encode / +17% decode from compile" win (a warmup-plus-lingering-server fluke), the "+626ms precompute win" (GPU memory confound), and a "KaLM 1.33s per query" benchmark figure (it required an ingest-time path that wasn't actually built; as-wired it was 5.95s). We also did a synthetic-vs-real-document check, because timings measured on fake uniform docs lie about the length distribution that makes sorting matter.

The lesson a newcomer should take from this section is not "MLX is hard." It's that **a 20–30% phantom speedup is one stray background process away at all times.** If you don't re-measure in isolation, you will ship numbers that are quietly wrong, and they'll be wrong in the flattering direction every time.

---

## The KaLM path: faster, stranger, and an honest "not yet"

While Nemotron was the proven pick, we also hand-built a second, more exotic reranker: **KaLM-Reranker-V1-Nano** — a Matryoshka model (one trained so its output vectors can be truncated to smaller sizes without retraining, like nesting dolls) with a `t5gemma2` encoder-decoder architecture and just 0.27B parameters.

KaLM's trick is a **decoupled passage encoder**. A normal cross-encoder has to re-process the document every single query, because the document and query are glued together. KaLM splits them:

- An **18-layer VL-Gemma encoder** processes each passage *once*, at ingest time, and caches the result (~40KB per document).
- An **18-layer Gemma3 decoder** with merged self+cross attention runs at query time, cross-attending over the cached passage encodings. Only the raw output score (logit) at decoder position 72 is read for the yes/no relevance score.

So at query time, only the tiny decoder runs against pre-encoded passages. If the same haystack of documents gets queried many times, you amortize the expensive encode across all those queries. This was the second architecture that required a full hand-built MLX port (a full second port, validated the same way) — it matched a max score diff of **0.0008** vs PyTorch, top-5 ranking identical.

The speed is genuinely impressive (100-doc payload, clean MLX):

| Stage | Time |
|---|---|
| Encode (precompute @ ingest, cached) | 310 ms |
| Decode (per query, warm cache) | 360 ms |
| Full naive rerank (sorted B=25, no precompute) | 751 ms |

That 751ms full rerank is **2.6x faster than Nemotron's 1925ms**. And the decode-only path at 360ms with a warm cache is *basically Cohere's 300ms API latency* — except local and free. KaLM is overhead-bound rather than compute-bound (decode runs at only 48% of peak compute, the GPU is 63% reclaimable), which is exactly why **its dead levers match Nemotron's**: quantization is a loss (KaLM's int8/int4 ran 40–63% slower, as the table above shows), compile lands within noise, concurrency is 0.87x.

So why isn't KaLM the pick? Two honest reasons.

First, **its quality is unverified on our benchmark.** It passed a single-query top-3 match, posts strong public scores on BEIR and MIRACL (standard public retrieval benchmarks), and a 10Q test scored 9/10 — but 10Q noise is roughly ±20%,¹ so 9/10 proves nothing against Cohere's and Nemotron's verified 92%. We will not credit a quality claim we haven't measured at sufficient sample size.

Second, **the precompute economics don't fit the benchmark.** KaLM wins big when documents are *reused* across many queries — break-even is around 1–2 queries per document. But LongMemEval queries each haystack exactly once. Ingest-encoding 250k documents would take ~50 minutes to save ~7 minutes across 500 questions: a net loss. For a *persistent personal-memory service*, where the same memories get queried for months, KaLM is a clear multi-month win. For this benchmark, it's a production-scale play, not a benchmark lever.

So Nemotron stays the pick: proven 92%, zero ingest-side changes, no unverified claims. KaLM is the thing we built for the future and are honest about not having earned yet.

> ¹ Our own notes disagree on the 10Q noise figure — one brief calls it ±10%, another ±20%. We quote the more conservative ±20% here; either way, 10 questions is far too few to validate against a 92% baseline.

---

## The final numbers

After all the dead ends, here's where local reranking landed.

**Two models appear in these tables, and it's worth keeping them straight: Nemotron is what we ship** (proven 92% quality), **and KaLM is the speed ceiling we're reaching for** (faster, but quality unverified). Nemotron is the answer; KaLM is the future.

**Quality** (50Q stratified, answer-only — restated from earlier for the full picture):

| Model | Score |
|---|---|
| Cohere rerank-4-fast | 92% |
| Nemotron-1B MLX | 92% |
| No rerank | 88% |
| bge | 86% |
| jina | 84% |

**Speed** (100-doc real payload):

| Approach | Latency |
|---|---|
| Cohere remote | 0.3s |
| KaLM MLX (optimized, end-to-end w/ HTTP) | ~0.52s |
| KaLM MLX (optimal split) | ~0.61s |
| bge llama.cpp Metal | 0.7s (but only 86% quality) |
| Nemotron MLX (sorted B=25) | ~1.9s |
| KaLM PyTorch/MPS (end-to-end, incl. HTTP) | 2.9s |
| Nemotron PyTorch/MPS | 5.5s |
| ONNX int8 / WebGPU | 16–18s |

*MPS = PyTorch's Metal backend; ONNX and WebGPU are portable runtimes we also tried.*

The two headline ratios — both about **KaLM**, the speed ceiling — come with one honest caveat worth dwelling on, because it's the whole ethos of this post.

The *best case* — one fixed payload, the graph warmed — is **~520ms end-to-end, 5.5x faster than KaLM in PyTorch (2882ms)**, torch-free. But that number is a trap, and our own pipeline caught it. Run the server against the *live* benchmark — where every question brings a different set of candidate docs, and therefore a different tensor shape — and latency ballooned to **1.3–2.9s**. The culprit was `mx.compile`: it traces a fused graph *per input shape*, so on a stream of varying real payloads it re-traces constantly. Turning compile **off** (eager, no tracing) fixed it: across 8 different real query payloads the server runs **637–1007ms, median ~0.75s**, scaling with each query's doc lengths.

So the honest production numbers:

- **KaLM MLX in production (compile off, varying real payloads): ~0.75s, ~4–4.5x faster than PyTorch.** The 520ms/5.5x figure was the lightest payload with the graph warmed — a best case, not a headline.
- **KaLM MLX is 3.7x faster than Nemotron MLX** — ~612ms vs 1925ms on the fixed benchmark payload.

The lesson is the one this whole post is about: a `compile` flag that *helps* a fixed micro-benchmark can *hurt* in production, and only running the real pipeline reveals it. **Compile off is the production default.**

Per-stage against PyTorch on the identical 100-doc payload, KaLM MLX runs encode 1.8x faster (758ms vs 1348ms), decode 3.0x faster (421ms vs 1257ms), and the full split 2.2x faster (1179ms vs 2605ms).

The honest framing: local is still slower than Cohere's API. Nemotron MLX at 1.9s is ~6x slower than Cohere's 0.3s; KaLM at ~0.75s is ~2.5x slower. But the speedup *over PyTorch* is what makes local viable at all, and a 1–2s rerank latency is acceptable for nearly every memory workflow. You trade a little latency for zero per-query cost, full offline capability, and a model you can actually tune.

---

## Torch-free deployment, and the server

A reranker you can't deploy cleanly isn't a win, so the last mile was making Nemotron run **without PyTorch in the process at all** — inference-only.

PyTorch is a multi-gigabyte dependency that exists, in an inference server, mostly to load weights and tokenize. We removed both reasons:

- **Weights:** `mx.load()` reads the bf16 safetensors directly. No torch load.
- **Tokenizer:** `tokenizers.Tokenizer.from_file(tokenizer.json)` — verified to produce token ids identical to `transformers.AutoTokenizer`. This matters because `AutoTokenizer` transitively imports torch; the standalone `tokenizers` library doesn't.
- **Inference:** 100% MLX, no torch ops anywhere.

The payoff: resident memory (RSS) dropped from ~5GB (with the torch model loaded) to **~3GB**. The server itself exposes a **Cohere-format `/rerank` endpoint** — the same request/response shape as the Cohere API — which is the whole point of the modular design. The backend behind that endpoint can be Cohere remote, Nemotron MLX, or KaLM MLX, and nothing upstream changes. (One sharp edge: it has to be a single-threaded `HTTPServer`. The MLX GPU stream is thread-local, and `ThreadingHTTPServer` crashed on the first request.)

---

## The philosophy, in one paragraph

Everything above follows from one stance: **fact-and-benchmark-driven, no golden stuff, modular by default.** No gold-string matching, no per-question special-casing, no answer-peeking — every lever has to be a generic improvement that would work on data we've never seen. Every number gets re-measured in isolation before it's trusted, because a phantom 20–30% speedup is always one background process away. And the reranker stays swappable: a Cohere-format endpoint with interchangeable remote and local backends, so you can run the bleeding-edge API when you want it and your own hand-built MLX model when you'd rather not pay or phone home. The +3.34pp was the lever worth all of this. Owning it — cheaply, locally, honestly — was the road to here.

---

## So why is the system stuck at ~89%?

Here's the part that reframes everything above. Reranking is owned and retrieval is solved at 99.8% recall — and the system *still* tops out around **89.1%** (N=3 mean of our best clean config). The full ladder of what got us there:

| Stage | Accuracy (500Q) | Lever |
|---|---|---|
| Baseline | 79.0% | — |
| | 84.2% | early retrieval/answer fixes |
| | 86.2–87.4% | candidate pool + tuning |
| **Reranking ON** | **89.8%** | the one proven lever |
| Best clean (N=3 mean) | 89.1% | rerank + evidence-dedup + low-confidence drop |

Reranking is the single biggest rung on that ladder — but the ceiling above it isn't in the evidence pipeline at all. When we audited every miss, the breakdown was sobering: of the rerank-off misses, **41% were fixable answerer (reader) bugs, 43% were benchmark noise** (broken or ambiguous gold answers, format-strict judge dings), **and only 16% were genuine retrieval/extraction misses.** Subtract the unwinnable 43% and the *effective* clean ceiling is roughly **93.6%**, not 100% — and getting there is about the reader's reasoning, not the evidence it's handed.

And the wall the reader keeps hitting is specific: **multi-session counting.** Counting events that span many sessions holds at **79–87% across every system and every reader model we surveyed** — including frontier readers like Claude Opus. It's structurally reader-capped: a bigger model or a better prompt doesn't move it; only a deterministic aggregation aid does. For context, the published "leaders" on this benchmark (jordanmccann 96.2%, OMEGA 95.4%, Mastra 94.87%, the LongMemEval paper's 92.4%) all win on a stronger reader, an oracle retriever, or self-grading and benchmark hacks — none of them is a clean, comparable result, and our DeepSeek-Flash 89.1% **leads its reader tier**.

*Next thread: the reader. Why a frontier model buys ~10pp it mostly spends on temporal and latest-value questions, why the counting wall holds for everyone, and why "solve retrieval" was never going to get us to 95%. That's where the real ceiling lives.*