# Push to 95+ on LongMemEval-S (500Q), clean pipeline — plan + running log

> **HISTORICAL LOG.** The `SYMEM_*` env vars named below are the pre-rename spellings from when
> these sweeps ran; that prefix is dead. Harness levers are now `MEMBENCH_*`, engine gates are
> typed `SYMBIOTIC_MEMORY__*` config keys (docs/environment.md), and several one-off levers
> (`SYMEM_DEDUP_EVIDENCE`, `SYMEM_EXCLUDE_BRIEFS`, `SYMEM_LEDGER_RETRIEVAL`,
> `SYMEM_DETERMINISTIC_COUNT`, `SYMEM_RERANK_RESERVE`) were removed in the config triage.

**Goal:** 95%+ on LongMemEval-S 500Q, clean pipeline, NO LongMemEval-targeted hacks
(no per-question special-casing, no gold-string matching). Two products ship on this.

**Started:** 2026-06-25. Stack settled: qwen3-embedding-8b @1024d (OpenRouter) + DeepSeek-Flash
distill/answer/judge. Source vault: `runs/symbiotic-memory/long-mem-eval/500/factconsol-thinkon-500-20260624/vaults`.

## Baseline (measured, N-run, this session)
| config | mean | runs | spread |
|---|---|---|---|
| rerank-OFF | 85.1% | 84.6 / 86.0 / 84.8 | 0.7pp |
| **rerank-ON (working baseline)** | **88.5%** | 88.6 / 88.2 / 88.6 | **0.2pp (tight)** |

Reranker = +3.4pp, ranges don't overlap → the one proven lever. rerank-ON variance is *tight*
(±0.2), so on this baseline a single 500Q run is already fairly trustworthy; still prefer N=2–3 for verdicts.

**Gap to 95: +6.5pp = ~33 questions.**

## Methodology (paranoid rules)
1. **Variance floor:** think-on answerer is non-deterministic. rerank-OFF ±0.7pp, rerank-ON ±0.2pp.
   Trust a lever only when N≥2 ranges clear the baseline band.
2. **Cheap-first:** iterate on the ~57 rerank-ON misses with `/tmp/ask_one.py` (replays exact
   prompt to flash, ~10s, ~$0) BEFORE any 500Q run. Promote to 500Q N=3 only when the probe wins.
3. **Cross-check every "miss":** the audit showed ~43% of misses are benchmark noise (broken data /
   shaky gold / judge-strict). Verify against the raw source before "fixing" — never tune to match a
   broken gold (that's a hack).
4. **One lever at a time.** Log $ after every paid run. Budget ~$120; answer-only ≈ $0.33, 500Q
   re-ingest ≈ $13, 50Q re-ingest ≈ $2–3.

## The target set (rerank-ON ~57 misses) — audit split (from rerank-OFF 74-miss audit, being re-run on rerank-ON)
- **~32 benchmark noise (unwinnable as-scored):** 18 broken/ambiguous data, 8 shaky gold, 6 judge-strict.
- **~25 genuinely fixable:** flash_bugs (counting, temporal date-arithmetic, latest-value, over-strict
  windows) + retrieval misses (answer-bearing chunk never surfaced).
- **Path to 95:** fix the ~25 fixable + cleanly reclaim the 6 judge-strict (format/precision instruction,
  NOT gold-matching) → ~94–95%. Beyond that needs benchmark gold fixes (out of scope).

## Knob map (every tweakable; wired status; interactions)

### Retrieval — answer-only, cheap (~$0.33/run)
| knob | current | wired | notes / interaction |
|---|---|---|---|
| `SYMEM_RERANK` | on | ✅ | +3.4pp. KEEP. |
| `SYMEM_RERANK_MODEL` | cohere/rerank-4-fast | ✅ | pro worse. fast best. |
| `SYMEM_RERANK_CANDIDATES` | 100 | ✅ | **UNTESTED 150/200** — more golds into rerankable set; ×top-k |
| `SYMEM_RERANK_RESERVE` | 0 | ✅ | reserve-3 blend worse. 0 best. |
| `fact_top_k`/`raw_turn_top_k` | config yaml | ✅ | 40/50/200 worse (noise). rerank picks top-k from candidates. |
| query planner / multi-query | ? | ? | **UNTESTED** — decomposition for multi-session/retrieval misses |
| `SYMEM_TEMPORAL_FILTER` | off | ✅ | hurt −0.8. OFF. |
| `SYMEM_EXCLUDE_BRIEFS` | off | ✅ | net-neutral. |
| `SYMEM_LEDGER_RETRIEVAL` | off | ✅ | surfacing unreliable Totals misleads. OFF. |

### Answer — answer-only, cheap
| knob | current | wired | notes |
|---|---|---|---|
| `SYMEM_ANSWER_THINKING` | on | ✅ | +5pp. KEEP. |
| answer prompt (`--prompt-dir`) | **long 140-line** | ✅ (since no-op fix 6/25) | **SHORT 6-line UNTESTED** (was no-op before). length×abstention. |
| answerer model | deepseek-v4-flash | ✅ | flash best (pro/mistral/gemini/glm all worse). |
| format/precision rule | — | via prompt | **UNTESTED** — judge-strict reclaim (clean) |

### Distill / ingest — forces re-ingest (~$3 50Q / ~$13 500Q)
| knob | current | wired | notes |
|---|---|---|---|
| `SYMEM_DISTILL_THINKING` | off | ✅ | on hurt. OFF. |
| distill prompt | v4 | ✅ | **event-date extraction → event_time bug; granular subjects.** RE-INGEST lever. |
| `event_time` | =turn timestamp (BUG) | bug | should be EXTRACTED event date; answer temporal rules misfire. |
| reweave/consolidate | factconsol thinkon | ✅ | count-ledger Totals unreliable. |
| embed model/dims | qwen 1024 | ✅ | bigger = more golds in candidate set (expensive re-ingest). |

## Interaction map ("everything interacts")
- **rerank candidates × top-k:** more candidates only helps if rerank promotes the right ones into the
  answer top-k. Test together, not separately.
- **distill date-extraction × answer temporal rules:** real `event_time` would make the temporal rules
  fire correctly; today they misfire on uniform message-dates. NOTE: surfacing dates in *prose* (the
  enrich experiment) HURT via over-filtering — dates belong in zvec date-range FILTER, not text.
- **ledger Totals × answer counting rules:** unreliable Totals mislead the counting rules. Fix Totals
  (deterministic count) or don't surface them — don't half-trust.
- **answer-prompt length × abstention:** the long prompt's many "say unavailable" clauses drive
  over-abstention (seen in enrich regressions: correct answers → "unavailable"). Simplify may help.

## Lever queue (cheap → expensive) — the tackle order
1. **[RUNNING]** rerank-ON reasoning baseline → re-categorize the 57 misses (the real target set). ~$0.5
2. **Simplify answer prompt** (short vs long), probe on misses via ask_one.py → 500Q N=3. ~$1
3. **Judge-strict format/precision rule** (clean granularity instruction) → N=3. ~$1
4. **Rerank candidates 150/200** (+ top-k co-tune) → N=3. ~$2
5. **Query planning / decomposition** for multi-session + retrieval misses (investigate→build). ~$2
6. **Extraction re-ingest**: `event_time` = real event date + granular subjects + zvec date-filter.
   50Q diverse first (~$3) → 500Q (~$13) only if it wins.
7. **Deterministic counting / reliable ledgers** for count misses (build + re-ingest).
8. **Stronger reasoner** for residual count/temporal arithmetic (last resort).

## Budget tracker
| run | purpose | $ | cumulative |
|---|---|---|---|
| base95-rrkon-reason | reasoning baseline + 57 misses | ~0.5 | 0.5 |

## Running log
- **2026-06-25 start:** baseline confirmed 88.5% rerank-ON (tight ±0.2). Long answer prompt is the
  baseline. Source vault factconsol-thinkon intact. Launched base95-rrkon-reason (reasoning capture)
  to get the target set. Plan written.
- **base95-rrkon-reason = 89.6% (448/500), $0.24, reasoning captured.** 52 misses. To 95 = +27.
  By type: multi-session 20, temporal 15, knowledge-update 8, ss-preference 4, ss-user 3, ss-assistant 2.
- **RETRIEVAL IS SOLVED on rerank-ON: 51/52 misses have the gold in the candidate pool.** So the
  misses are ~all reasoning errors + benchmark noise, NOT recall. The rerank-OFF audit's "12 retrieval
  misses" collapse to ~1 with rerank ON. Implication: more rerank candidates / hybrid search has little
  headroom here; the lever is REASONING (counting, date-arithmetic, latest-value) + the noise floor.
- **Retrieval-flow map (Explore):** planner is LLM (Flash mode), emits canonical + dense_queries[1-4]
  + sparse_terms + time_window, all embedded together in ONE search. `sparse_terms` are collected but
  only embedded — NO keyword/BM25 lane exists (a real but lower-priority gap given 51/52 gold-in-pool).
  Multi-hop 2nd round gated OFF (within-noise). Cheap answer-only levers: rerank candidates, multi-hop-all,
  context budget, planner mode.
- **Launched (background):** fix-map workflow (categorize 52 → mechanism + fix lever); cheap sweep
  (short-prompt N=2, rerank-200 N=2); competition research (mem0/Zep/MemoryOS/etc — comparability +
  applicable counting/temporal techniques + reader-model gap).

### FIX-MAP (52 rerank-ON misses) — `/tmp/fixmap.json`
- flash_bug 32, broken/ambiguous-data 8, shaky_gold 7, judge_strict 3, retrieval_miss 2.
- fixable_by: **answer_prompt 27**, none(noise) 15, deterministic_count 4, rerank/retrieval 2, format 2,
  **reingest_event_dates 1, reingest_subjects 1**. → the expensive re-ingest fixes only 2 questions. DEPRIORITIZED.
- mechanisms (flash_bug): over_abstain 9, wrong_item 9, miscount 8, stale_value 4, date_arithmetic 2.
- Noise floor 15 → effective ceiling ~97% on this run. 95 is inside the noise ceiling.

### PROMPT LEVERS ALL FAILED (answer-only, vs 89.6 baseline) — the answer-prompt is EXHAUSTED
- short (6-line simplify): **84.0 / 84.2** — much worse. Rules do real work; don't remove them.
- rerank candidates 200: **87.0 / 87.6** — worse. 100 is better (51/52 gold already in pool).
- surgical (long + "prefer-to-answer" discipline): **87.8** — worse. The abstention tension is real:
  cutting abstention fixes over_abstain but creates more false-positives. Net-negative, as the old plan warned.
- → The "27 answer_prompt-fixable" is an ILLUSION; fixing them via prompt breaks others equally. Confirmed 5×.

### COMPETITION VERDICT (workflow wvl1dp56n) — decisive
- **NO clean comparable 95%+ exists.** Every 94-96% headline disqualified: jordanmccann 96.2% (Claude
  Opus + 46-iter overfit + explicit hacks), OMEGA 95.4% (macro-avg inflation; micro 93.2%; generator==judge),
  Chronos-High 95.6% (Claude Opus 4.6 reader; GPT-4o version = 92.6%), Mastra 94.87% (gpt-5-mini; its
  gpt-4o = 84.23%, below us), mem0 94.4% (self-report, gpt-4o answerer+judge), paper 92.4% (ORACLE + GPT-4o).
- **Our 89.6% (DeepSeek-Flash, no hacks) is LEADING at its reader tier.** Honest peer tier (gpt-4o-mini
  readers: EverMemOS/MemReader/MemOS/MAGMA) = 77.8-83.0%. MemCoT (weak reader + reasoning) = 88.0%. All below us.
- **Reader gap:** a frontier reader buys ~+10pp but concentrated in TEMPORAL + LATEST-VALUE/KU; PLATEAUS on
  multi-session COUNTING (79-87% across every system AND every reader — structurally reader-capped).
- **We already score on the cleaned dataset** (longmemeval-cleaned) — yet 15 noise misses remain (cleaned set still imperfect).
- **Ranked no-hack levers:** (1) Chain-of-Note JSON reading [running], (2) deterministic counting over
  deduped (subject,event_date) — the ONLY structural fix for the counting wall, (3) event-time extraction
  +(4) date-deltas-in-code (~+6pp temporal subset), (5) append-only latest-value. AVOID: override prompt
  rules (hacks), graph rewrite (regressed elsewhere; retrieval already solved).
- **Reader-swap diagnostic [queued]:** gpt-5-mini / deepseek-r1 / gemini-3.1-pro answer-only, to bound
  how much of our 52 misses is reader-capped vs technique-fixable.

### ITERATIVE TUNING (user: "one try and quit is not tuning" — inspect every run in detail)
- **Baseline recalibrated:** true baseline ≈ **88.5% (rerank-ON N=3 mean 88.6/88.2/88.6)**. The single
  base95-rrkon-reason=89.6 was a HIGH OUTLIER (above the N=3 max). Compare levers to the 88.5±0.2 band.
- **Manual inspection of Chain-of-Note (87.8) revealed it's CONDITIONAL:** +2 multi-session +1 temporal
  GAINS, −7 single-session-PREFERENCE (enumeration makes preference answers terse/abstaining).
- **condcon (conditional CoN: enumerate for count/temporal, holistic for preference) = 88.7/88.6** —
  the conditional scoping cut preference regressions 7→3, but multi-session both gains AND regresses
  (+4/−7). In-prompt enumeration is unreliable for counting → the real fix is counting IN CODE.
- **Reasoning levels (user-requested):** flashmax (effort=max) + promax (deepseek-v4-pro) running.
  (`on`=high is the baseline; `max` is the only untested level.)
- **DETERMINISTIC COUNTING BUILT + unit-tested (7/7 pass):** `SYMEM_DETERMINISTIC_COUNT` (off by default).
  Reuses the existing evidence_ledger emission (supported_items JSON), counts distinct (label,date) in
  Rust, overrides the prose answer ONLY when code-count disagrees with the model's number AND the set is
  ≥2 items AND not "unavailable" → strictly additive on the agree-path (zero regression there).
  Files: symem `evidence_ledger.rs` (parse_evidence_ledger/dedup_count), `engine.rs` (run_deterministic_count
  + hook + count_intent + extract_prose_number). Pending: rebuild harness + answer-only A/B vs baseline.

### REASONING LEVELS (user-requested) — within-deepseek, settled
- flash `on`(=high)=baseline ~88.5. flash `max` = **90.8 / 86.8** (huge variance — raises ceiling but
  unstable). deepseek-v4-pro `max` = **87.0** (worse). → flash-high is the right reader; pro is worse;
  max is a high-variance gamble. Confirms user's "model changes are noise" read. Reader settled = flash.

### NEW DIRECTION (user 2026-06-25): cleaner/tighter/task-split INPUTS, NOT count-in-code
User redirected: "Counting is not what we should do in code... prompt splitting for specific tasks,
specifically for multi, removing ambiguous stuff (worked before), smaller k, better recall focus, dedupe."
- **Per-task prompt splitting** [BUILT, gated SYMEM_SPLIT_PROMPTS]: count/list → a lean 12-line counting
  prompt instead of the 140-line everything-prompt (no temporal/preference dilution). symem prompt_policy.rs.
- **Smaller k** [staged]: k12/k8/k6 configs (fewer, higher-precision evidence). Prior plan only tried LARGER
  k (all worse); smaller is untested.
- **Evidence dedup + remove-ambiguous** [TODO]: content-dedup (today only memory_id dedup) + drop ambiguous/
  low-conf evidence. Confirm what "ambiguous" means with user.
- **tune2 running:** rebuild + baseN(new-binary ref) + split N=2 + k12 N=2 + k8. Deterministic-count SHELVED
  (gated off, user said no to count-in-code).

### tune2/tune3 — SUBSET ISOLATION is now the method (aggregate variance ~±1.5pp swamps levers)
- 6 baseline-ish runs span **87.2-89.6 (mean ~88.6, std ~0.8)** → aggregate single/N=2 comparisons are
  unreliable. Detect levers by their DETERMINISTIC subset effect, not the noisy total.
- **split-prompts (count/list → lean): DEAD.** count/list subset 82.4%→82.4% (+0.0); other −0.1 (noise
  check valid). Prompt CONTENT is not the counting lever. count/list = the weak class (82.4 vs 90.1 other).
- **smaller k (k12/k8): within noise** (87.4-88.2). 
- **tune3 [running]:** the 4 "remove ambiguous + dedupe" evidence filters individually N=2 —
  SYMEM_DEDUP_EVIDENCE / DROP_LOWCONF(0.7) / DROP_CONFLICTING / RELEVANCE_CUTOFF(0.5). Built in symem
  engine.rs (gated, off). Will isolate each via per-class/changed-question subset analysis.
- **PLAN to beat the noise floor:** find which levers are non-negative by subset analysis, then STACK the
  non-hurting ones and test the full stack N=3 vs baseline N=3 — compound many sub-noise wins into a
  detectable mean shift (the only realistic path to +4-5pp given no single lever clears the band).

### COUNTING MISS MECHANISM MAP (deep inspection of the 8 miscount misses, 2026-06-25)
The weak class (count/list 82.4%) fails FOUR different ways — not one-lever-fixable:
1. **Over-strict temporal window** (plants 3→2): excluded an item dated just outside flash's calendar
   reading of "last month". Often a genuinely ambiguous question boundary.
2. **Over-LOOSE predicate** (projects 2→4): counted "planning a project", "solo project", class
   projects as "led" despite the prompt saying "only explicitly led". Flash ignores the rule.
3. **DUPLICATE-EVENT double-count** (baking 4→8): the SAME "baguette last Saturday" event was distilled
   into TWO facts with different resolved dates (05-27 and 05-20) → flash counts both. An EVIDENCE/
   distillation artifact — content-dedup won't catch it (dates differ); needs event-level dedup.
4. **Over-STRICT predicate-entailment** (albums 3→1): excluded "got a vinyl" because it said "got" not
   "bought". Flash under-counts on entailment.
→ Implication: evidence-cleaning (dedup/conflict) can only touch #3-type; #2/#4 are predicate-judgment
   the prompt already covers and flash ignores; #1 is often ambiguity/noise. Counting is genuinely diffuse.
   When tune3 lands, check its effect on THESE specific qids, not just the aggregate.

### tune3 EVIDENCE-CLEANING RESULTS (subset isolation on count/list, the target class)
baseline count/list 82.8% | other 90.1%. Deltas (avg of N=2 each vs baseline N=3):
- **lowconf (drop conf<0.7): count/list +2.0, other −0.3** ← best on the weak class, ~no harm. KEEPER.
- **dedup (content): count/list +0.4, other +0.6** ← small positive both, no harm. KEEPER.
- conflict-collapse: count/list +1.2 but other −1.2 (net negative — drops needed multi-value slots). OUT.
- relevance-cutoff(0.5): count/list −0.4 (neutral/negative). OUT.
- Caveat: count/list is 122 q so subset noise is ~±4-5pp; these deltas are marginal. → STACK dedup+lowconf
  N=3 to see if they compound above noise [running].
- Counting misses are NOT evidence-fixable in bulk: the dup-event (baguette 8) is a DISTILLATION artifact
  (same event distilled twice w/ diff resolved dates); predicate-strictness (projects/albums) is
  reader-capped; only confidence-pruning nudged it. The honest answer-side ceiling looks like ~89%.

### MAX-REASONING is a COUNT-routing candidate (subset analysis)
flash reasoning=max: count/list **84.4% (+1.6 vs 82.8)**, other 90.2 (≈baseline) — helps ONLY counting,
but unstable on the subset (87.7/81.1 per run). pro=max is worse (76.2 count/list). → route count
questions to max-effort, others to high (stable): only count absorbs the variance. Stacks with dedup+lowconf.
Needs per-question reasoning-effort plumbing (effort is global today). Candidate for the stack if it pans out.

### MARGINAL-POSITIVE LEVER SET (the only things above zero, all weak-class-targeted, all near noise)
dedup, lowconf(0.7), max-reasoning-on-count. Everything else tested = genuine zero or worse. Path to a
detectable mean shift = STACK these (running dedup+lowconf N=3). Even stacked, realistic ceiling ~89-90;
95 is not reachable clean on this stack (flash reader) — confirmed by competition + exhaustive lever sweep.

### STACK RESULT (dedup + lowconf, N=3) = NEW BEST CONFIG
overall **89.1 (+0.8 vs 88.3 baseline)**, count/list 83.9 (+1.1), other 90.7 (+0.6). Runs 90.0/88.2/89.0.
First config to beat baseline mean AND lift both subsets — compounding the marginal levers works, but the
lift is +0.8 (still within the ±1.5pp band, ranges overlap). Best clean config = rerank-ON + SYMEM_DEDUP_EVIDENCE
+ SYMEM_DROP_LOWCONF(0.7). Realistic answer-side ceiling ~89-90. To go past needs the INGEST frontier.

### ANSWER SIDE — CONCLUSIVELY COMPLETE (2026-06-25)
- evidence-root-following (SYMEM_FOLLOW_ROOTS): NEUTRAL — stack+roots 89.1 (=stack), roots-alone 87.0.
  Adding facts' source turns doesn't help (retrieval solved). OUT.
- search/planner prompt: near-zero BY PROOF — retrieval is solved (51/52 golds in pool), a better search
  prompt recovers ≤1 question. Not built (would buy ~+0.2pp at most). The real gap (sparse_terms never used
  as BM25/keyword lane) buys ~nothing given 51/52 already covered.
- **FINAL TALLY:** ~25 levers tested. 1 big proven (reranker +3.4). 2 marginal keepers (dedup, lowconf →
  stack +0.8). Everything else (≈20 levers incl. all prompt variants, smaller-k, reasoning-max, model-swaps,
  conflict/relevance filters, root-following, deterministic-count, split-prompts) = within ±1.5pp noise or
  worse. BEST CLEAN CONFIG = **89.1%** (rerank + SYMEM_DEDUP_EVIDENCE + SYMEM_DROP_LOWCONF=0.7).
- The answer side is exhausted. 95% is not reachable here; ~89-90 is the validated clean ceiling for the
  flash-reader tier (which we LEAD). The only untested frontier is the INGEST side (re-distill; low EV ~2 per fix-map).

### INGEST FRONTIER (user chose B, 2026-06-25): event-date extraction A/B
IMPLEMENTED: distiller emits `event_date` (resolved absolute YYYY-MM-DD; /tmp/prompts-eventdate/distill.yaml
v5), llm.rs parses → fact.event_time when SYMEM_EXTRACT_EVENT_DATE=on (else turn time, the current bug).
Clean A/B = 2 fresh 50Q ingests (both emit event_date; gate off vs on) so only the event_time SOURCE
differs. Answer step = best config (rerank+dedup+lowconf). Targets the temporal misses (event_time was
uniformly the message date, useless for the answer prompt's temporal rules). Low EV per fix-map (~2) but
it is the one untested frontier the answer side cannot reach. Watch: (1) does event_time actually differ;
(2) temporal-subset delta. If it wins on 50Q → check-in before a full 500Q re-ingest (~$13).

### INGEST FRONTIER RESULT = ZERO (event-date A/B, 50Q)
ingest-base50 **94.0%** = ingest-eventdate50 **94.0%** (47/50 both). Event-date extraction has NO effect —
and it WORKED mechanically (verified: eventdate event_times shifted to 22 distinct dates vs base's 15, not
a no-op). The real event dates just don't change answers. Confirms fix-map (~2 = within noise). DO NOT do
the full 500Q re-ingest. (50Q score 94% is a higher/easier stratified subset, not comparable to 500Q ~89.)

### ============ DEFINITIVE CONCLUSION (2026-06-25) ============
EVERY stone turned: ~25 answer-side levers + the ingest frontier. **BEST CLEAN CONFIG = 89.1% (N=3) =
rerank + SYMEM_DEDUP_EVIDENCE + SYMEM_DROP_LOWCONF(0.7).** Proven movers: reranker +3.4, dedup+lowconf +0.8.
Everything else = within ±1.5pp noise or worse, INCLUDING the event-date re-ingest (the one frontier the
answer side couldn't reach). 95% clean is NOT reachable on this stack and nobody has it clean (every public
94-96% = stronger reader / oracle / self-grading / hacks). 89.1% with DeepSeek-Flash and zero hacks is the
honest, LEADING result for its reader tier. → LOCK IN. Commit reranker+dedup+lowconf; revert dead gates.

### COUNT SPECIALIST (expand-k + semantic-dedup + lean-prompt + max-reasoning) — FIRED, HURT
Built count-gated: SYMEM_COUNT_TOP_K=40 + SYMEM_DEDUP_SEMANTIC (post-rerank token-Jaccard, catches
differently-dated near-dups exact-match misses) + SYMEM_SPLIT_PROMPTS + max-reasoning arm. VERIFIED FIRED:
count Qs 16.3→32.3 facts, deduped, lean prompt applied. RESULT: count/list 82.8 → 81.6 (cspec) / 81.6
(cspecmax), Δ −1.2; majority-vote 83.6→77.0/74.6. Noise-check (non-count, gated off) ≈0 → isolation clean.
→ EXPANDING evidence BACKFIRES for counting (more facts = more distractors to over-count); max-reasoning
doesn't rescue it. With smaller-k also monotone-worse, k=20 is the sweet spot. Counting is a REASONING
ceiling over the right ~20 facts, NOT an evidence-coverage problem. Drop the specialist. (OpenRouter hit
−$ mid-session; user refilled; credits verified by a 3/3 probe before the re-run.)

### Strategic read
The realistic CLEAN ceiling for the flash stack is ~90-91% (89.6 now + maybe +1-2 from deterministic
counting/date-deltas). 93-95% needs a FRONTIER READER (a product/cost decision) — and even then 95 isn't
clean. The night's remaining value: (a) deterministic counting [flash-stack, structural]; (b) reader-swap
[bounds the ceiling, informs product decision]; (c) Chain-of-Note [cheap test, tempered].
