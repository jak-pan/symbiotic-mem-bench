---
name: score-run
description: Score/measure a membench LongMemEval run (or several) — overall accuracy, per-question-type breakdown, reasoning-fired count, cost, and the hard/control split. Use whenever the user asks to score, measure, re-measure, or compare runs by name or path.
---

# Score a membench run

Run the scorer with one or more run names or paths:

```
scripts/score-run.sh <run-name-or-path> [<run> ...]
```

A run name resolves under `runs/symbiotic-memory/long-mem-eval/<limit>/<name>` (a full path also works). Pass several to compare them side by side.

## What it prints, per run

- **acc** — overall correct/total from `artifacts/verdicts.jsonl`.
- **by-type** — accuracy per LongMemEval question type: `ms` (multi-session), `tr` (temporal), `ku` (knowledge-update), `ss-user` / `ss-asst` / `ss-pref` (single-session).
- **reasoned** — how many answerer calls emitted a reasoning trace (`n/a` if the run has no per-question debug). Use this to confirm thinking actually fired — opt-in models (e.g. gemma-4) need `MEMBENCH_ANSWER_REASONING_EFFORT=high`, not just `THINKING=on`. See `MODEL-REASONING-DEFAULTS.md`.
- **cost** — total run cost from the report (the `cost.rs` rollup, priced from the OpenRouter `/models` catalog in `config/pricing/openrouter-pricing.json` + the native static table, with prompt-cache discount). Canonical for runs scored after the pricing-catalog change; re-score older runs to refresh. Per-model split is in the dashboard / `/api/run`. Refresh prices: `scripts/refresh-pricing.sh`.
- **hard / control** — the tier2 (31) and control (30) split, shown only when the run covers those question sets (the 61-question baseline). Qids come from `runs/inputs/longmemeval-hard/{hard-tier2-cluster31,control-easy30}.json` — canonical, not `/tmp`.

## Notes

- Scoring is read-only over already-written artifacts; it never re-runs the model.
- All counting is jq/awk (no Python), matching repo conventions.
- To score a fresh run, first produce it with `membench ... --score --run-name <name>`, then `scripts/score-run.sh <name>`.
