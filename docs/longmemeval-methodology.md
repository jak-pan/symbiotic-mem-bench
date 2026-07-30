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

The bundled leaderboard currently publishes **one verified ranked result**:

- **87.4% accuracy (437/500)** on LongMemEval-S for
  `factconsol-thinkon-500-20260624`, with 88.80% task-averaged accuracy and 25/48
  abstention accuracy.
- The run used the Symbiotic Memory stack with DeepSeek Flash for
  distillation/query planning/answering, OpenRouter `qwen/qwen3-embedding-8b` at 1024
  dimensions, answer thinking enabled, and DeepSeek Flash judging in
  `semantic-shared-compact` mode.
- Its cohort is tied to dataset fingerprint
  `d8a772212d40150b75664719d3d078268bd665ce0988ca894c7064de97465c4e`; the portable
  record includes all question-level scoring artifacts and trace-backed provider provenance,
  and passed the independent review gate below.

Separate local full-500Q experiments measured **≈88.5% accuracy** (N=3: 88.6 / 88.2 /
88.6, spread 0.2pp) with a related rerank-ON configuration. Those runs remain useful tuning
evidence, but they have not been promoted and reviewed as canonical public records and
therefore are not leaderboard claims.

Additional interpretation from those local experiments:

- Run-to-run variance with the non-deterministic answerer is material (rerank-OFF spread
  ±0.7pp; earlier same-config runs ranged 87.2–89.6): single-run deltas under ~2pp are noise.
- Reranking is the one proven lever: +3.4pp over rerank-OFF (85.1% mean), with
  non-overlapping ranges.
- Context for the ceiling: feeding perfect gold evidence to the same reader caps at ≈94%
  (the reader wall), and ~15/500 questions have broken or ambiguous golds. Scores near 100%
  on this benchmark should be treated with suspicion, not aspiration.

Detailed running evidence: `TUNING-REPORT.md`, `PUSH-TO-95.md` (historical log).

## Comparability: what a cohort is

A leaderboard cohort is one *comparability class*, not one size class. Two runs share a cohort
only when **benchmark, question count, question-set fingerprint, judge model and judge prompt
mode** all match; that tuple is the cohort id
(`long-mem-eval::500::ds:<fingerprint>::judge:<model>::mode:<prompt_mode>`), and it is also
recorded on every run. Runs judged by different models, or scored over different question sets,
are different boards — their accuracies are not comparable, so the tooling will not place them
in one table. The live `/api/leaderboard` and the static export share this partition.

## Review gate for a canonical ranked record

A record may be ranked in a published leaderboard cohort only if all of the following hold.
This gate is what separates "verified" from "unverified" on the landing page. Conditions 1–4 and
the artifact half of 5–6 are **enforced in code** (`src/eligibility.rs`) against bytes on disk —
a record's own `artifact_manifest` is not evidence — and a record that fails any of them is
listed as unranked with the gate it failed, never ranked.

1. **Full-scale, fresh run.** The complete benchmark question set for its cohort (500Q for
   LongMemEval-S), executed through the full pipeline. Answer-only reruns are acceptable only
   when the ingested substrate they reuse is itself part of the record's provenance.
   *Enforced as `full-scale`: the scored question count must equal the cohort's declared size,
   so a 50-question subset can never appear on the 500-question board.*
2. **Complete scoring artifacts.** `hypotheses`, `verdicts`, and `scored` present in the
   record, plus `run-params.json` and `benchmark-report.json` with `dataset_fingerprint`,
   `judge_model`, and `judge_prompt_mode` recorded.
   *Enforced as `scoring-artifacts` (each file must exist on disk and be non-empty — a manifest
   entry is not evidence), `cohort-identity`, and `score-summary-hashes`: when the scorer wrote
   `artifacts/score-summary.json`, the hashes it recorded for the artifacts it judged must still
   match those files. That chain involves no reviewer, so post-scoring edits are caught on any
   record, reviewed or not.*
3. **Provenance, not intent.** Provider usage proven by model/provider traces
   (`model_traces` or provider-queue traces), with cost rollup derivable. `configured_models`
   alone proves nothing. *Enforced as `provenance-traces`.*
4. **Clean flags.** Not `oracle_gold`, not `TRIAL`-flagged, no experimental gate enabled that
   has not been disclosed in the record's config label. *Enforced as `clean-flags` (meta
   records, oracle-gold runs, trial-flagged runs, non-promotable protocol identities, and
   contradictory report/parameter/registry-path benchmark identities are rejected); the
   remaining disclosure judgement is human.*
5. **No-cheating review.** A second reviewer (human or independent agent) samples verdicts
   against raw artifacts and confirms the pipeline rules above; misses were not "fixed" by
   tuning to broken golds. *Enforced as `independent-review` + `artifact-hashes`: the record
   must carry a `review.json` attestation (below) whose recorded SHA-256s still match the
   scoring artifacts. Editing any artifact after review invalidates the attestation rather
   than silently inheriting it. Whether the reviewer did a real review is not machine-checkable
   — the attestation names who is accountable for it.*
6. **Hygiene.** No secrets, raw provider payloads, or absolute local paths in the record;
   oversized native state stored externally per `docs/canonical-record-storage-task.md` with
   hashes and restore instructions. *Attested in `review.json`; not machine-checked.*

### The review attestation (`review.json`)

Written next to `benchmark-report.json` in the record directory:

```json
{
  "schema": "membench.record_review.v1",
  "reviewer": "name of the person or independent agent accountable for the review",
  "reviewed_at": "2026-07-24",
  "reviewed_commit": "7e416c4",
  "verdict": "pass",
  "artifact_sha256": {
    "hypotheses": "<sha256 of artifacts/hypotheses.jsonl>",
    "verdicts": "<sha256 of artifacts/verdicts.jsonl>",
    "scored": "<sha256 of artifacts/scored.json>"
  },
  "notes": "what was sampled and checked"
}
```

`verdict` must be `pass`; any other value (or a missing/renamed schema, reviewer or date) keeps
the record unranked. Hashes are plain `sha256sum` of the files as reviewed.

Promotion is `membench save-record` into `records/{system}/{benchmark}/{limit}/{run_name}/`,
adding `review.json` after the independent review, then regenerating the bundled leaderboard
snapshot (`scripts/export-leaderboard-snapshot.sh`; CI re-checks it against `records/` via
`scripts/check-leaderboard-snapshot.sh`). The first canonical 500-question promotion is
`factconsol-thinkon-500-20260624`; its 437/500 result is the verified rank-1 row described
above. The 2026-07-24 opposite-model/K3 review independently checked the promoted record and the
then-pinned c22 kit revision. That historical attestation does not certify the later divergent f6
squash-port and packaging/build-graph upgrade. The f6 recovery evidence is tracked separately in
`docs/oss-release-handoff.md`, and a fresh protected CI run is required for its release head.
