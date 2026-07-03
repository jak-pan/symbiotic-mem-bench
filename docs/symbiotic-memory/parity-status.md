# Parity Status

Status: in progress.

This repo now has the task 152 parity path wired as reusable library behavior, not only as a
benchmark script:

- default production Reduce prompt in `prompts/distill.yaml`;
- measured Reduce variants in `prompts/symbiotic-memory/mem0.yaml`,
`prompts/symbiotic-memory/durable.yaml`, `prompts/symbiotic-memory/supersession.yaml`, and
`prompts/symbiotic-memory/exhaustive.yaml`;
- Gemini embedding provider with retries and request timeouts;
- DeepSeek-compatible distill and answer providers split by model;
- clean greedy recall router with provenance fields;
- raw-turn semantic + keyword fusion with chronological restoration;
- per-question SQLite vault reuse;
- partial vault resume when raw turns exist but facts are missing;
- bounded chat/embedding retries;
- optional distill-window cache through the kit's `distill.cache_dir` config field
  (`SYMBIOTIC_MEMORY__DISTILL__CACHE_DIR`);
- foundation SQLite workflow queue for LongMemEval row leases and completion state;
- foundation model queue response-cache traces for provider-backed adapters;
- foundation model queue transition traces under `provider-queue/model-queue-traces.jsonl`;
- historical score artifact recording, since moved to `membench`;
- answer-only reruns over complete SQLite vaults for cheap answer/router experiments;
- original source artifact preservation under each SQLite vault's `archive/artifacts/` directory with
digest and metadata recorded in Capture stage metrics;
- conservative current-state lifecycle consolidation using explicit `slot_key` values, with additive
history left append-only unless a fact declares `supersedes`.

## Latest Probe

Historical command details are intentionally omitted here because benchmark ownership has moved to
`../symbiotic-mem-bench`; use that repo for runnable LongMemEval commands.

Score:

```text
LongMemEval QA score: 9/10 = 0.9000
```

Run notes:

- A capped distill probe failed with `finish_reason=length`; production distill runs must not set
artificial output caps.
- Earlier probes used a whole-row timeout and showed why that was the wrong control point: slow rows
can be legitimate when work is waiting in durable model queues. Normal runs now leave
`MEMBENCH_QUESTION_TIMEOUT_SECS=0` and rely on per-step/model timeouts instead.
- Hypothesis JSONL had 10 rows and zero forbidden scoring fields.
- the historical score recorder updated all 10 vault manifests.
- Provider response cache contained 342 chat responses and 17,168 embedding responses after the
run/resume.
- Queue traces showed transient provider failures but successful recovery: Gemini had 212 failed
embedding attempts before 17,168 successes; DeepSeek Flash had 2 dead decode items and 330
successful distill calls through the logical retry/cache path.

The only miss was:

```text
question: Where did I buy my new tennis racket from?
expected: the sports store downtown
actual: a sports store downtown
```

Do not overfit this miss. It is a wording strictness case, not evidence absence.

Answer-only follow-up:

- `hyp-answer-only-v2.jsonl` reran the same 10 completed vaults with a generic source-phrase fidelity
prompt rule for short who/what/where/which answers.
- Score remained `9/10 = 0.9000`; the miss stayed `a sports store downtown`.
- The original raw turn says the racket was "got from a sports store downtown", while the oracle uses
"the sports store downtown"; no article-normalization patch was promoted.

## Full-500 Parity Anchor

Small slices are now infrastructure smoke only. They are useful for checking that credentials,
queues, retries, caches, manifests, and output files work, but they are not a tuning target and must
not be used to promote quality changes.

The current 94+ anchor is the frozen task-152 full-500 parity artifact:


| artifact                                   | result                                  | interpretation                                                              |
| ------------------------------------------ | --------------------------------------- | --------------------------------------------------------------------------- |
| `route-residual-phrase-v12b-generic-clean` | `471/500 = 0.942`                       | honest generic post-strip selector screen, with no qid/gold/verdict routing |
| `route-greedy5-howmany-m1`                 | `453/500 = 0.906` and `452/500 = 0.904` | cleaner deployable routed baseline over existing full-500 arms              |


The historical reproduction emitted a 500-row hypothesis file and a 500-row provenance file that
were byte-for-byte identical to the frozen task-152 `v12b-generic-clean` artifacts. The existing
scored artifact reports `overall_accuracy = 0.942`, `task_averaged_accuracy = 0.9375`, and
abstention accuracy `26/30 = 0.8667`. This repository does not package the historical Python
selector wrapper as a runnable memory path; current runs go through the parameterized `membench`
system and benchmark flags. Existing artifacts can be attached to the new report format with
`membench --system symbiotic-memory --benchmark long-mem-eval --import-report --hypotheses ... --scored ...`.

This is a parity target, not a product claim for this standalone crate yet. New quality claims from
`symbiotic-memory` require generated hypotheses from this crate on the full cleaned 500 set.

## Next Required Work

Before a full 500 quality run:

1. Enable the kit's `distill.cache_dir` (`SYMBIOTIC_MEMORY__DISTILL__CACHE_DIR`) for all paid runs.
2. Use `model-traces.jsonl` to verify cache-hit/miss token accounting.
3. Verify all DeepSeek and Gemini calls route through the foundation model queues; do not add
  separate stage-specific semaphores.
4. Use `--resume` for timeout recovery; missing terminal workflow rows are force-reenqueued while
  normal runs still reject stale terminal duplicates.
5. Close the retrieval implementation gate in `docs/symbiotic-memory/retrieval-implementation-gate.md`. In
  particular, do not spend a full run to rediscover that dense vectors plus token-hit scoring miss
   paraphrases, rare terms, lifecycle state, and compact derived evidence.
6. Run answer-only variants over a completed run only if the issue is answer shaping or routing.
7. Run full 500 for quality validation after retrieval parity gates pass. Use smaller slices only to
  prove the pipeline functions or to debug a failure mechanism, not to choose answer/routing
   changes.

Current clean baseline target remains full-500 parity against task 152. The 94.2 selector screen is
the near-term behavior target; the 90.5 router is the cleaner deployment baseline until the generic
selector behaviors are implemented as memory/evidence mechanisms.