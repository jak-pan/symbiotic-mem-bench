# LongMemEval Tuning — Investigation Report

Consolidated findings for the symbiotic-memory pipeline on LongMemEval-S 500Q (qwen3-embedding-8b@1024d
+ DeepSeek-Flash answerer). Companion to the chronological ledger [PUSH-TO-95.md](PUSH-TO-95.md) and the
re-read bible in [AGENTS.md](AGENTS.md). Methodology rule throughout: **verify a lever actually FIRED
(changed the prompt/evidence — measure it) before trusting any score delta**; run-to-run answerer
nondeterminism is ~±1.5pp and fakes "wins" on levers that didn't fire.

## 1. Answer-prompt shapes tested

The answerer system prompt is a hardcoded literal in `prompt_policy.rs`, overridden by `--prompt-dir
<dir>/answer.yaml`. Each variant = the default + a specific change. Results are answer-only N≥2 vs the
~88.3–89.6 baseline band.

| variant | chars | change from default | result | verdict |
|---|---|---|---|---|
| **default** | 8229 | 7 sections: Core/Evidence/Temporal/Current-state/Exactness/Counts/Recommendations | ~88.3–88.5 | **BASELINE (best)** |
| short | 1202 | stripped to 5 one-liners (removed all discipline) | 84.0/84.2 | ✗ −4.4 (fired, hurt) |
| surgical | 18083* | + "Answer-vs-abstain discipline" (prefer-to-answer, yes/no, name-when-asked) | 87.8 | 〜 noise (in-band) |
| con (Chain-of-Note) | 18220* | + "Reasoning procedure" (note each memory→enumerate→reason→answer) | 88.0/87.6 | 〜 noise; +count/temporal, −7 preference |
| condcon | 18560* | con but conditional (enumerate for count/temporal, holistic for preference) | 88.8/88.6 | 〜 noise — **collapsed under replication** (con-r2 already fixed condcon's "fixes"; con-r1 was a bad-luck draw) |
| count-list lean (split) | 1397 | count/list questions only: stripped to counting-only (Method+Answer) | count/list +0.0 | 〜 noise (fired, zero effect) |

(*surgical/con/condcon char counts include the yaml's duplicated `cacheable_prefix`.)

**Takeaway:** prompt *content* is not a lever. The default (most rules) is best; shortening hurts (−4.4);
adding/restructuring rules is in-band noise. The one apparent win (condcon) evaporated under replication.

### Forensic diff: lean count prompt vs default (all consistent flips on the count subset)
Of the count/list questions where the two prompts consistently disagree: **3 fixes, ~10 breaks**. The lean
prompt is one trait in two directions — *takes predicate/scope literally, loses the default's soft-inclusion
machinery.* FIXES (narrow): dedup ("American, American"→"American"), a correct explicit 0 on a bounded
timeframe (museums-in-December, where the default over-abstained), one entailment win. BREAKS, two opposite
modes, each from a REMOVED default rule: **(1) over-exclusion / under-count** — the lone "exclude near misses"
over-fires without "count source-backed entailments" + "each distinct pending obligation" + the temporal
section → drops the dry-cleaning pickup (3→1), Sunday yoga (5→4), the mattress (4→3), ride instances (10→9),
a date-boundary aggregate (23→1); **(2) bare-zero / wrong-abstain on category miss** — without "no exact
match → unavailable, do not answer 0," it answers 0 where the category is absent (Italian restaurants, egg
tarts; gold = "not enough info") and over-pedants on near-matches the default accepts (5-day-trip shirts,
album-vs-poster). CONCLUSION: the default's verbose counting rules (entailment, pending-obligation, temporal,
and "no exact match → unavailable not 0") are LOAD-BEARING; the lean prompt kept only the subtractive rule,
so it nets negative. The default is at a local optimum — can't lean OR pad it into a win. Caveat: break
tally mildly inflated by 3-default vs 2-count run asymmetry (e.g. gpt4_d6585ce8 is a mislabel).

### The count prompt vs the original — and the thinking change
The lean count prompt is the original **stripped to only the counting logic** (2 sections vs 7; 1397 vs
8229 chars; temporal/preference/exactness/recommendation rules removed). It **fired** (applied to 119/500
count/list questions exclusively; the 8229-char default elsewhere) and **changed the thinking**: count-question
reasoning ran **+28% longer** (median 1011→1295 chars), more enumeration-focused. Concrete flip — *"How many
plants did I acquire in the last month?"* (gold 3): the default applied a strict 30-day window and answered
**2** (excluded a ~Apr-25 plant); the lean prompt enumerated more inclusively and answered **3**. BUT the net
is **+0.0 (symmetric churn)** — for each question it fixes it breaks another, because the answerer's
predicate/date judgment varies per question; the prompt only reshuffles which way it errs.

## 2. Forensic tune audit (did each lever FIRE? real or noise?)

9-agent forensic over ~86 runs. **Exactly one lever is real.**

| lever | fired? | verdict | evidence |
|---|---|---|---|
| **reranker (ON vs OFF)** | YES, strong | **PROVEN +3.34pp** | ~63% of top-20 changes; ON-min 88.20 > OFF-max 86.00 (ranges separate); causal: a Q where OFF buried the fact→"unavailable", ON surfaced it→correct |
| lowconf@0.7 | **NO (0.0%)** | NO-OP | min confidence IS 0.70 → removes 0 facts; byte-identical evidence |
| dedup (exact) | barely (0.43%) | NO-OP | then the 20-cap backfills → net unchanged |
| conflict (slot-collapse) | barely (0.74%) | NO-OP | backfilled |
| **stack (dedup+lowconf)** | NO net change | NO-OP | the "+0.8 / 89.1 best config" was variance with no mechanism |
| lc80@0.8 | YES (17.6%) | NEUTRAL | fired but count/list 82.8=82.8 → low-conf facts didn't matter |
| relevance-cutoff@0.5 | YES (12%) | COST/LATENCY only | accuracy-neutral |
| reasoning=max | YES (4× tokens) | NOISE | +0.5pp overlapping; 90.8 was the high tail of a 90.8/86.8 band |
| count specialist (expand-k 40 + semantic-dedup + lean + max) | YES, strong (16→32 facts) | **WORSE −1.2** on count/list | expanding evidence backfires for counting |
| smaller-k (k12/k8) | YES | WORSE | monotone k20>k12>k8 |
| rrk candidates 200 / 50 / 150 | YES (weak) | NOISE/WORSE | 200 below band; 50 worse; 150 one-run noise |
| follow-roots | YES (+1640 blocks) | NOISE | churn 16 gained/17 lost |
| event-date re-ingest | YES (17.1% re-dated) | ZERO | 50Q 94.0=94.0 |
| briefs-off | YES | **WORSE −3.35** | briefs are load-bearing |
| ledger-retrieval | YES | WORSE −3.3 | unreliable Totals mislead |

**Method bugs caught:** `reason-off` actually runs thinking=ON (a prompt+thinking confound, not a clean
off-control); `pro-answer` silently emitted 0% reasoning; env vars aren't persisted in run-params.

## 3. Pipeline integrity (is the answerer actually using the evidence?)

Tested because "drastic knobs → ~0 effect" looked like it could be a pipeline bug. It is NOT:
- **Knobs DO change the answerer's evidence** (top-20 fact-id Jaccard vs the 0.936 same-config variance
  floor): k8 changed 62%, rerank 60%, lc80 20%, dedup 0.95 (≈variance, confirmed near-no-op).
- **The answerer DOES use the evidence** (ablation — remove the answer-bearing fact, replay): "Veja"→
  "unavailable", "Pennsylvania"→"unavailable" (2/3 broke; the 3rd had a redundant copy).
- **Why most knobs do little:** the few *load-bearing* facts are high-relevance so rerank keeps them in
  the top regardless, and evidence is redundant. Only changes to *which fact is load-bearing-and-present*
  move the score: rerank (relevance reorder) +3.34; briefs-off (deletes load-bearing) −3.35; short prompt
  (removes discipline) −4.4. It's *relevance*, not *volume* — k8 and rerank both change ~60% of the
  evidence, opposite outcomes.

## 4. Bottom line

**The reranker is the only real lever. Honest best clean config = rerank-ON ≈ 88.3–88.5%, no hacks** (+
baseline prompt, k=20/10, candidates=100, briefs ON, reasoning=high). The counting weak class (~82.8%) is
a **reasoning ceiling over the right ~20 facts** — not retrieval, not evidence-coverage, not prompt-content
(every count lever that fired was neutral-or-worse). 95% is not reachable on this stack; no public 94–96%
is clean (all use a stronger reader / oracle / self-grading / hacks). Every experimental lever is gated
off by default; the tree is clean and 155 symem tests pass.
