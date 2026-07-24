# Leaderboard Export Canary

This directory is the end-to-end contract smoke test for the static leaderboard
export (`membench.leaderboard.v1`). CI rebuilds the export from the fixture
records here and diffs it against the committed `expected-leaderboard.json`;
any drift in the export contract fails the `leaderboard-contract` job.

## Contents

- `records/symbiotic-memory/long-mem-eval/5/canary-alpha/` and `canary-beta/` —
  two **synthetic fixture records** in the standard
  `{system}/{benchmark}/{limit}/{run_name}` shape. They are deliberately
  minimal (`run-params.json` + `benchmark-report.json` only), carry
  `"run_kind": "synthetic-fixture"` and `"fixture": true`, and must never be
  presented as real benchmark results. Alpha scores 0.8 (4/5) with all scoring
  artifacts listed available (verification level `full`); beta scores 0.6 (3/5)
  and lists `scored` as missing (verification level `partial`).
- `records/symbiotic-memory/long-mem-eval/5/canary-gamma/` — a synthetic
  **meta-record** fixture (`meta_record` present, accuracy 0.4): it must land
  in `unranked` with reason `meta-record` while keeping its accuracy fields.
- `expected-leaderboard.json` — the exact deterministic export over those
  fixtures (formatted with `python3 -m json.tool`).

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
