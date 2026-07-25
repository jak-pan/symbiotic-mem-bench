# PR #4 body — source of truth

Update the GitHub description of PR #4 (`codex/membench-v2-text-lane-20260725`) from this file.
Two hard rules for that description: it must never use a closing keyword for issue #2 (nor claim
to partially close it — #2 is the v1 OSS release issue and is already closed on its own terms),
and it must never present this lane as official LongMemEval-V2.

---

Related to #2; this PR does not close it, in whole or in part.

Adds `longmemeval-v2-text`, an **experimental text projection of LongMemEval-V2 — never an
official LongMemEval-V2 tier, and its numbers must never be reported as official LongMemEval-V2
scores**. It exists so the current text-only Symbiotic Memory adapter can exercise the released
trajectory corpus safely while the multimodal adapter boundary is built.

What it adds: official deterministic evaluator semantics, strict fail-closed schema and evaluator
preflight (unsupported official LLM checker rows are rejected before any provider work or run-root
mutation, so an existing score bundle survives a rejected `--score`/`--rejudge` launch), stable
resumable corpus identity, cohort/cap provenance, hash-bound score artifacts, CI gates, and
upstream dataset instructions.

Limits, stated plainly:

- excludes the 29 image-query questions; trajectory screenshots stay locator-only;
- does not implement the official memory-context-to-fixed-reader protocol or latency/LAFS metrics;
- run parameters record `official_equivalent: false` and `leaderboard_eligible: false`, and the
  ranking gate rejects these records even with a review attestation;
- `--no-score` skips scoring only — ingest, distillation, embedding, and answering still run
  through paid providers by default (use `--smoke` for a local no-network check);
- the score bundle (`verdicts.jsonl`, `scored.json`, `score-summary.json`) is staged and then
  renamed file by file with the hash-binding summary last; it is **not** one atomic unit — an
  interrupted publish is instead rejected by the eligibility gates (missing/empty artifact,
  score-summary hash mismatch), and v2-text records are categorically leaderboard-ineligible.

Verified: default lib suite; adapter-enabled lib/bin unit suites and the `benchmark_v2` contract
tests (now enforced in the mandatory `rust` CI job, not only the key-gated `adapter-build` job);
real 451-question dataset validation; bounded no-provider smoke; strict clippy and the adapter
build gate.
