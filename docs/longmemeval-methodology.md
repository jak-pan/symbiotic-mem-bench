# LongMemEval Methodology and Review Gate

This is the methodology document referenced by every `membench.leaderboard.v1` export. It
states how LongMemEval scores are produced, what the honest current result is, and the review
gate a record must pass before it may be ranked on the leaderboard.

## Benchmark

- **Dataset:** LongMemEval-S, 500 questions, using the community-cleaned dataset
  (`xiaowu0162/longmemeval-cleaned`; gold answers in `answer`, gold sessions in
  `answer_session_ids`). The exact question set of a run is captured as
  `cohort.dataset_fingerprint` — a SHA-256 over the sorted question-id set — so "same
  questions" is verifiable, not assumed.
- **Question types:** single-session-user, single-session-assistant,
  single-session-preference, multi-session, temporal-reasoning, knowledge-update.
- **Judging:** LLM judge (recorded per run in `scored.json` as `judge_model`), default prompt
  mode `official` — the per-question-type paper grader (see `JUDGE.md`). Verdicts across
  judge prompt modes are not comparable; the cohort identity includes the judge model and
  prompt mode, and the dashboard flags mixed cohorts as not strictly comparable.

## Pipeline rules (what "clean" means)

- The full memory pipeline runs end to end: ingestion, distillation, embedding, retrieval,
  optional reranking, answering. No stage may read the answer key.
- No gold-string matching, no per-question special-casing, no dataset-targeted heuristics.
- Experimental levers are gated flags, off by default; a lever only counts as real when it is
  proven to have fired and its effect clears the variance floor on N≥2 runs.
- Diagnostic runs over question subsets are `TRIAL`-flagged and are never benchmark claims.

## Honest current result

There is currently **no ranked score published**: the tracked `records/` tree contains only
meta records (timing/transport evidence without question-level scoring artifacts), so every
leaderboard cohort is empty and all records appear as unranked with their exclusion reason.

The best **clean, measured** result to date, from local full-500Q runs pending promotion:

- **≈ 88.5% accuracy** on LongMemEval-S 500Q (N=3: 88.6 / 88.2 / 88.6, spread 0.2pp) with
  the Symbiotic Memory stack: DeepSeek Flash distill/answer (thinking on), OpenRouter
  `qwen/qwen3-embedding-8b` @1024d, cross-encoder reranking ON, judged by DeepSeek Flash.
- Run-to-run variance with the non-deterministic answerer is material (rerank-OFF spread
  ±0.7pp; earlier same-config runs ranged 87.2–89.6): single-run deltas under ~2pp are noise.
- Reranking is the one proven lever: +3.4pp over rerank-OFF (85.1% mean), with
  non-overlapping ranges.
- Context for the ceiling: feeding perfect gold evidence to the same reader caps at ≈94%
  (the reader wall), and ~15/500 questions have broken or ambiguous golds. Scores near 100%
  on this benchmark should be treated with suspicion, not aspiration.

Detailed running evidence: `TUNING-REPORT.md`, `PUSH-TO-95.md` (historical log).

## Review gate for a canonical ranked record

A record may be ranked in a published leaderboard cohort only if all of the following hold.
This gate is what separates "verified" from "unverified" on the landing page.

1. **Full-scale, fresh run.** The complete benchmark question set for its cohort (500Q for
   LongMemEval-S), executed through the full pipeline. Answer-only reruns are acceptable only
   when the ingested substrate they reuse is itself part of the record's provenance.
2. **Complete scoring artifacts.** `hypotheses`, `verdicts`, and `scored` present in the
   record (export verification level `full`), plus `run-params.json` and
   `benchmark-report.json` with `dataset_fingerprint`, `judge_model`, and
   `judge_prompt_mode` recorded.
3. **Provenance, not intent.** Provider usage proven by model/provider traces
   (`model_traces` or provider-queue traces), with cost rollup derivable. `configured_models`
   alone proves nothing.
4. **Clean flags.** Not `oracle_gold`, not `TRIAL`-flagged, no experimental gate enabled that
   has not been disclosed in the record's config label.
5. **No-cheating review.** A second reviewer (human or independent agent) samples verdicts
   against raw artifacts and confirms the pipeline rules above; misses were not "fixed" by
   tuning to broken golds.
6. **Hygiene.** No secrets, raw provider payloads, or absolute local paths in the record;
   oversized native state stored externally per `docs/canonical-record-storage-task.md` with
   hashes and restore instructions.

Promotion is `membench save-record` into `records/{system}/{benchmark}/{limit}/{run_name}/`,
followed by regenerating the bundled leaderboard snapshot
(`scripts/export-leaderboard-snapshot.sh`). The selection and promotion of the first
canonical 500-question record is tracked in `docs/canonical-record-storage-task.md` and has
not happened yet — which is why the published leaderboard is honestly empty.
