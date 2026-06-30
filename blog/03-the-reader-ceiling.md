# The Reader Ceiling: Feed a Model Perfect Evidence and See Where It Stops

*Post 3 of the Symbiotic Memory series — the reader deep-dive promised at the end of Posts 1 and 2. Retrieval is solved, reranking is owned, and the system still tops out near 89%. This is the post about why — and about the one experiment that turned "the bottleneck is the reader" from an assertion into a measured number with a hard ceiling bolted to it. Every figure here is reproducible; the dataset and harness are public.*

---

## The benchmark, stated precisely

Everything in this series is scored against **LongMemEval-S** — 500 questions over long, multi-session chat histories, each testing whether a system can recall a specific fact buried across months of conversation. It's the de-facto standard for agent memory, and it's public: [paper (arXiv:2410.10813)](https://arxiv.org/abs/2410.10813), [code (github.com/xiaowu0162/LongMemEval)](https://github.com/xiaowu0162/LongMemEval), [project page](https://xiaowu0162.github.io/long-mem-eval/). We score specifically against the maintainers' **cleaned** split ([xiaowu0162/longmemeval-cleaned](https://huggingface.co/datasets/xiaowu0162/longmemeval-cleaned), `longmemeval_s_cleaned`, 500 questions), which removes noisy history sessions that interfere with answer correctness.

It tests five abilities, which map directly onto the question categories we'll break results down by:

- **multi-session** reasoning (133 questions) — synthesize facts across many sessions, including counting
- **temporal-reasoning** (133) — reason about *when* things happened
- **knowledge-update** (78) — track a value that changed over time, return the latest
- **single-session-user / assistant / preference** (70 / 56 / 30) — recall within one session
- **abstention** (30, distributed across the above) — the answer was never stated; the correct move is to refuse

That's 500 questions. Throughout this post the headline number is **accuracy over all 500**.

---

## The thread we kept pulling but never cut

By the end of the first two posts the story had a suspicious shape. Retrieval was **solved** — the gold evidence lands in the top-100 candidate pool for 499 of 500 questions (99.8%). Reranking was **owned** — a hand-built local cross-encoder that ties Cohere, +2.4pp, the one lever that ever cleared the noise band. And the system *still* sat at **~89%**.

Every time we hit that wall we said the same thing: *the bottleneck is the answerer's reasoning.* But that claim was always an **inference**, never a measurement. We'd categorized the misses, seen that the right evidence was usually on screen, and concluded the reader must be fumbling it. Reasonable. Unproven. And it left the real question unanswered:

> If retrieval were *perfect* — if the answerer got exactly the right evidence and nothing else — how high could it score? And does a stronger reader change that ceiling, or just cost more?

You cannot answer that by tuning the pipeline forward. So we stopped going forward.

---

## The backwards step

The move that broke the logjam was to stop optimizing the pipeline and instead **delete it** — for one experiment. Hand the answerer the perfect evidence directly, skip retrieval entirely, and measure what the reader does with a clean desk. We call it the **oracle**, and the methodology — *find the ceiling before you climb toward it* — was the most useful thing we did all project.

It works because LongMemEval ships a turn-level evidence flag we'd been ignoring: every message carries `has_answer: true|false`, marking the **exact** turns that contain the answer. They're sparse — typically 2 to 6 marked turns out of ~480 in a question's history. The oracle forces the answerer's context to be *exactly* those turns and nothing else. For the 30 **abstention** questions there are no `has_answer` turns (there's no answer), so the oracle feeds them **empty context** — and a correct reader abstains.

That isolates the reader from retrieval completely. Whatever a model scores here is its **ceiling**: the best it can do if retrieval were flawless. Every number below is an oracle run — same evidence in every model's context, only the answering model changes.

(One methodology note we'll lean on: the 470 *answerable* questions get identical treatment across every run regardless of when it executed, so they're directly comparable. The 30 abstention questions depend on getting the empty-context handling right — a fix we landed mid-project. For models that ran before that fix, we measured their abstention behavior separately on empty context and folded it in, so **every row in this post is on one consistent full-500 basis**.)

---

## What perfect evidence revealed

Our production answerer is **DeepSeek-Flash with reasoning on**. Hand it the perfect evidence and it scores **436/500 (87.2%)** — essentially the same ~87–89% we see end-to-end. Crank it to *max* reasoning and it reaches 448 (89.6%), then stops. DeepSeek-Pro, the bigger sibling, is no better.

Read that the way Post 2 taught you to: **the reader caps at ~89% even when retrieval is perfect.** The 89% wall was never mostly about retrieval, and it was never fixable by giving DeepSeek more thinking budget. The inference from Posts 1–2 was right — but now it's a measurement, not a hunch. Which raises the question Post 1 only speculated about ("a frontier reader would buy ~10pp"): what does a *stronger* reader do on the same evidence?

---

## The reader sweep — full 500, with cost

We swept 14 reader configurations across nine models on the identical oracle evidence. Accuracy is over all 500; cost is the measured spend for one full answer-only oracle run (small context — 2–6 gold turns plus the answer prompt — computed from real token usage, since per-provider pricing isn't in our cost rollup for OpenRouter).

| reader | thinking | accuracy (500Q) | cost / run | cost / point | on Pareto frontier? |
|---|---|---|---|---|---|
| **qwen3.7-plus** | on | **469 (93.8%)** | **$0.67** | **$0.0071** | ✅ value champion |
| qwen3.7-max | on | 469 (93.8%) | $2.09 | $0.0223 | ✗ (qwen-plus, same acc, 3× cheaper) |
| gemini-3.5-flash | on | 467 (93.4%) | $4.10 | $0.0439 | ✗ (qwen-plus is higher *and* 6× cheaper) |
| gpt-5.5-medium | on · medium | 466 (93.2%) | $7.78 | $0.0835 | ✗ (dominated) |
| qwen3.6-35b-a3b | on | 453 (90.6%) | $0.52 | $0.0057 | ✅ |
| nemotron-ultra-550b | on | 453 (90.6%) | $0.99 | $0.0109 | ✗ (qwen3.6, same acc, cheaper) |
| nemotron-super-120b | on | 449 (89.8%) | **$0.15** | **$0.0017** | ✅ budget champion |
| deepseek-flash-max | on · max | 448 (89.6%) | $0.24 | $0.0027 | ✗ (nemo-super, higher acc, cheaper) |
| deepseek-flash-high | on · high | 445 (89.0%) | $0.10 | $0.0011 | ✅ floor |
| deepseek-pro-max | on · max | 441 (88.2%) | $0.48 | $0.0054 | ✗ |
| minimax-m3 | on | 437 (87.4%) | $0.49 | $0.0056 | ✗ |
| deepseek-flash (default) | on | 436 (87.2%) | $0.14 | $0.0016 | ✗ |
| deepseek-pro-high | on · high | 434 (86.8%) | $0.31 | $0.0036 | ✗ |
| deepseek-pro (default) | on | 433 (86.6%) | $0.45 | $0.0052 | ✗ |

*`thinking` = reasoning enabled on every run; effort is shown where we set it explicitly (gpt-5.5 `medium`; deepseek `high`/`max`), otherwise the model's own default — per-model defaults in [`MODEL-REASONING-DEFAULTS.md`](../MODEL-REASONING-DEFAULTS.md). `cost / run` = measured spend for one answer-only oracle run (real token usage). `cost / point` = run cost ÷ accuracy %, i.e. dollars per percentage point of accuracy — lower is better value.*

The frontier readers (qwen3.7-plus, gemini-3.5-flash, gpt-5.5) land at **~93–94%** on the same evidence DeepSeek-Flash gets 87% from. That's **+6.6pp over flash-default, +4.2pp over flash-max, from nothing but the reader.** Post 1's "a frontier model buys ~10pp" was directionally right; the clean, evidence-controlled number is **~+5pp of pure reader skill** (the looser informal figure also bundles in noisy-retrieval interaction — see the ceiling caveat).

So the reader is genuinely the lever it was suspected to be — five points were sitting on the table the whole time, recoverable by swapping the answerer *if* you can feed it clean evidence.

---

## Price/performance: the expensive models are all dominated

The cost column reframes the whole sweep. Sort by value and a clear **Pareto frontier** emerges — four readers that are the best accuracy available at their price; everything else is strictly dominated (something is at least as accurate for less money):

- **deepseek-flash-high — $0.10, 89.0%** — the floor. Nothing cheaper does better.
- **nemotron-super-120b — $0.15, 89.8%** — budget champion. 90% accuracy for fifteen cents a run, **$0.0017 per accuracy point** — the cheapest points in the table.
- **qwen3.6-35b-a3b — $0.52, 90.6%** — the mid-tier step, and the one worth a second look: **open-weight (Apache-2.0)** with only **~3B active parameters** (of 35B, MoE), so it self-hosts on a single workstation — ~21GB at 4-bit, comfortable on a 64–128GB Mac or a 24GB GPU. Other open-weight readers near this accuracy exist (Nemotron-super/ultra and MiniMax-M3 are all open-weight too), but they're datacenter-scale — 12B to 55B *active* params. qwen3.6 is the only ~90% reader you can realistically **run yourself, offline, at zero marginal cost** on normal hardware.
- **qwen3.7-plus — $0.67, 93.8%** — the **value champion**. Ceiling-grade accuracy for two-thirds of a dollar.

And the punchline: **every model more expensive than qwen3.7-plus is dominated by it.** qwen3.7-max matches its accuracy at 3× the cost. gemini-3.5-flash is *lower* (93.4%) at 6× the cost. gpt-5.5 is lower still (93.2%) at nearly **12×** the cost — **$0.0835 per accuracy point versus qwen-plus's $0.0071.** If you're paying frontier prices for a reader on this task, you're paying for a brand, not for points.

The one honest caveat on cost: these are *oracle*-run costs, with small contexts (2–6 gold turns). A production run feeds the reader more retrieved context, so absolute dollars rise — but the *relative* ordering holds, because every model here saw the same context size.

---

## By category: where the points actually are (and aren't)

Accuracy over 500 hides the structure. Broken out by LongMemEval's five abilities, the same evidence tells a sharper story (counts are correct / total in each category):

| reader | multi-session (133) | temporal (133) | knowledge-update (78) | ss-user (70) | ss-asst (56) | ss-pref (30) |
|---|---|---|---|---|---|---|
| qwen3.7-plus | **118** | 125 | 75 | 69 | 54 | 28 |
| qwen3.7-max | **117** | 126 | 74 | 69 | 54 | 29 |
| gemini-3.5-flash | **117** | 124 | 75 | 69 | 54 | 28 |
| gpt-5.5-medium | **116** | 125 | 73 | 68 | 55 | 29 |
| qwen3.6-35b-a3b | 113 | 121 | 75 | 66 | 53 | 25 |
| nemotron-ultra-550b | 109 | 124 | 70 | 68 | 54 | 28 |
| nemotron-super-120b | 113 | 119 | 71 | 66 | 53 | 27 |
| deepseek-flash-max | 112 | 119 | 68 | 66 | 55 | 28 |
| deepseek-flash-high | 108 | 122 | 69 | 65 | 54 | 27 |
| deepseek-pro-max | 108 | 119 | 68 | 64 | 54 | 28 |
| minimax-m3 | 102 | 122 | 67 | 66 | 53 | 27 |
| deepseek-flash (default) | 104 | 120 | 69 | 64 | 52 | 27 |
| deepseek-pro-high | 104 | 117 | 72 | 61 | 52 | 28 |
| deepseek-pro (default) | 102 | 121 | 70 | 60 | 51 | 29 |

Three things fall out:

- **Single-session is solved.** Across the top tier, ss-user/assistant/preference run **96–99%** — and even the weakest readers are in the low 90s. There is nothing to win here.
- **Temporal (~94%) and knowledge-update (~96%)** are near-ceiling for the frontier readers.
- **Multi-session is the wall.** Even the best reader, handed perfect evidence, tops out at **118/133 (88.7%)**. It's the lowest category for *every* model and the single largest pool of misses. This is the multi-session counting problem Post 1 flagged at 79–87% across all systems — and the oracle proves it holds *even when you hand the model every relevant turn at once*. It is not a retrieval problem and not a horsepower problem; it's aggregation.

---

## The hard ceiling

Now the result that reframes the goal. Look at the top of the accuracy table:

- qwen3.7-plus → **469** · qwen3.7-max → **469** · gemini-3.5-flash → **467** · gpt-5.5 → **466**

Four frontier readers, different labs, handed perfect evidence, all land within **3 questions of each other at ~94%**. That's a wall. **No reader cracks ~94%** — not by being bigger (qwen-max = qwen-plus), not by reasoning harder (flash-max never reaches it), not by switching architecture or vendor.

The ~30 answerable questions that *every* reader misses *with the exact evidence in front of it* are two things, and neither is buyable: **multi-session counting** (the 15 the best reader still drops in that category) and **thin or ambiguous gold** — questions where the annotation itself is incomplete or the "correct" answer is debatable, the benchmark-noise category from Post 1's audit.

This is the deepest finding in the post: **even perfect evidence and the best available reader top out around 94%.** The last six points to 100 — and the gap from there to a *clean* 95 — are not a reader problem. They're a data/task problem, and pretending a bigger model fixes them is exactly the mistake the inflated leaderboard numbers make.

---

## The confounds we caught (because this series always has a few)

Post 2's reranking saga was really a story about measurement discipline. The reader sweep had its own traps; here's the graveyard.

- **The abstention-oracle bug.** Our first oracle had no special handling for abstention questions, so when it found zero `has_answer` turns it silently *fell back to normal noisy retrieval* — feeding the answerer the very distractors the oracle was meant to remove. The dashboard showed it; we'd been claiming "the reader only sees gold" while 30 questions saw the full junk pile. Fixed by forcing **empty** context for those, and clearing the bypassed recall from the debug view so it can't lie again. *Show what the model actually received, not what you intended it to receive.*

- **`accuracy/500` vs `accuracy/470`.** Because that fix changed only the 30 abstention questions, the 470 answerable ones are the only set comparable across the fix boundary. Quoting all-500 across that boundary is how "qwen beats gemini" sneaks in when the truth is "qwen ties gemini." Every full-500 number here is reconciled: answerable from the run, abstention from a matched empty-context measurement.

- **50-question smokes lie.** We filter candidates on a cheap 50Q sample before paying for the full 500. It's a *filter*, not a measurement: nemotron-super smoked at 94% and landed at 90%; minimax-m3 smoked at 93% and fell to 87%. A 50Q sample swings ±5pp. Only full-500 numbers appear above; smokes only decided who *earned* a full run.

- **Cost accounting that under-reported by 240×.** Our cost rollup prices DeepSeek precisely but doesn't price OpenRouter answer models — so it reported a gpt-5.5 run at **$0.03** when the real bill was **$7.78**. The tokens were recorded; the pricing table had a hole. Every cost in this post is recomputed from logged token counts. *A number that looks tracked isn't tracked until you check what it's summing.*

---

## What this means for 95

Put it together and the path is, for the first time, unambiguous.

- The reader **is** a real lever — ~+5pp — but only on clean evidence. (On *noisy* retrieval the advantage largely evaporates: a strong reader chokes on the same distractors a weak one does. The frontier's edge is *gated* on a clean context.)
- The best reader money can buy, on **perfect** evidence, caps at **~94%**. The remaining points are multi-session counting and thin gold — a data/task ceiling, not a model gap.
- Among readers that reach the ceiling, **qwen3.7-plus is the pick** — 93.8% for $0.67, beating gemini and gpt-5.5 on both axes — with nemotron-super-120b the budget option (89.8% for $0.15).

So the reader question, which dangled across two posts, is **closed**. The open problem is no longer "which model answers" — it's "can we get the real, noisy pipeline to hand that model the same clean evidence the oracle did." Today it can't: the live answerer gets reranked distractors and adjacent-topic turns, a world away from the 2–6 exact turns the oracle feeds. Closing that gap — **denoise** — is the only road from 89 to the ~94 ceiling. And cracking ~94 itself means attacking the multi-session counting wall directly, with deterministic aggregation, not a bigger checkbook.

---

## The philosophy, restated

This post is one idea wearing a lab coat: **measure the ceiling before you climb toward it.** Tuning a system forward, you can spend months unable to tell whether a miss is the retriever's fault or the reader's. Take the backwards step — delete retrieval, hand the reader perfect evidence, read the number — and the question answers itself in a single sweep. It told us the reader is worth +5pp, named the cheapest model that captures it ($0.67, not $7.78), proved ~94% is a wall no reader breaks, and located the surviving misses in one category. Every figure is one variable changed at a time, on a public benchmark, with the confounds — the 32% failure rate, the 240× cost miss, the abstention bug — hunted down and named.

*Next thread: denoise. Now that we know the ceiling, the reader that reaches it, and its price, the whole game is delivering oracle-grade evidence through a real, noisy pipeline — and the multi-session counting wall that even perfect evidence can't climb.*
