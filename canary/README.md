# Leaderboard Export Canary

This directory is the end-to-end contract smoke test for the static leaderboard
export (`membench.leaderboard.v1`). CI rebuilds the export from the fixture
records here and diffs it against the committed `expected-leaderboard.json`;
any drift in the export contract fails the `leaderboard-contract` job.

## Contents

Six **synthetic fixture records** under
`records/symbiotic-memory/long-mem-eval/5/`, in the standard
`{system}/{benchmark}/{limit}/{run_name}` shape. All carry
`"run_kind": "synthetic-fixture"` and `"fixture": true`, and must never be
presented as real benchmark results. Between them they pin every branch of the
ranking gate (`src/eligibility.rs`, `docs/longmemeval-methodology.md`):

| Fixture | Score | What it pins |
|---|---|---|
| `canary-alpha` | 0.8 (4/5) | **Self-attestation is not verification.** Its manifest declares `hypotheses`/`verdicts`/`scored` available while the record holds no artifacts at all — it must stay unranked (`scoring-artifacts`, `provenance-traces`, `independent-review`). |
| `canary-beta` | 0.6 (3/5) | Manifest lists `scored` as missing; unranked for the same reasons. |
| `canary-gamma` | 0.4 (2/5) | A **meta record**: unranked with reason `meta-record`, keeping its accuracy fields. |
| `canary-delta` | 0.8 (4/5) | The **one fully eligible record**: real artifacts on disk, model traces, a scorer hash chain in `artifacts/score-summary.json`, and a `review.json` attestation whose hashes match. Ranks first in its cohort. |
| `canary-epsilon` | 1.0 (5/5) | Complete artifacts, **perfect score, no review attestation** — must never outrank `canary-delta`, or appear at all. This is the regression that keeps an unreviewed record from topping the board. |
| `canary-zeta` | 0.6 (3/5) | Eligible, but judged by `canary-judge-b` — must form its **own cohort**, never share a table with `canary-delta`. |

`expected-leaderboard.json` is the exact deterministic export over those
fixtures (formatted with `python3 -m json.tool`).

Changing any artifact under a reviewed fixture without regenerating both its
`review.json` and `artifacts/score-summary.json` hashes will (correctly) drop it
out of the ranking — that is the `artifact-hashes` / `score-summary-hashes`
gates doing their job, not a broken fixture. Verified by appending one verdict
line to `canary-delta`: it leaves the cohort and lands in `unranked` citing both
gates.

## Regenerating the expected file

From the repository root:

```bash
cargo run --bin membench-leaderboard -- export \
  --records-root canary/records --deterministic | python3 -m json.tool > canary/expected-leaderboard.json
```

`--deterministic` pins `generated_at`, `git_sha`, and per-row `modified_ms` so
the output is stable across machines and fresh checkouts. Regenerate only when
the export contract intentionally changes, and review the diff before
committing.
