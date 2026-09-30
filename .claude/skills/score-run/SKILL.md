---
name: score-run
description: Score/measure a membench LongMemEval run (or several) — overall accuracy, per-question-type breakdown, reasoning-fired count, cost, and the hard/control split. Use whenever the user asks to score, measure, re-measure, or compare runs by name or path.
---

# Read a Run's Scores

Shared collaboration, Git, verification, and security rules: House Rules (`~/p/house-rules/AGENTS.md`).

From the repository root, with `jq` and `awk` available:

```bash
scripts/score-run.sh <run-name-or-path> [<run> ...]
```

Names resolve under `runs/symbiotic-memory/long-mem-eval/<limit>/<name>`; paths must contain
`artifacts/verdicts.jsonl` with native `autoeval_label.label` booleans. Imported/canary verdicts
using other label fields are unsupported and can print incorrect zero counts; inspect those
through `membench explore` instead. Multiple runs print side by side. This is read-only: it reports stored
artifacts and never invokes an answerer or judge.

- `acc`: correct/total from `autoeval_label.label` in verdicts.
- `by-type`: LongMemEval question-type counts (`ms`, `tr`, `ku`, `ss-user`, `ss-asst`, `ss-pref`).
- `reasoned`: answerer calls with a reasoning trace, or `n/a` without per-question debug.
  Check this before interpreting an effort/model trial.
- `cost`: stored `benchmark-report.json`'s `metrics.cost_micro_usd`; it does not refresh pricing
  or recompute an old report. Missing cost prints zero, which does not prove the run was free.
  Use [membench cost diagnostics](../../../skills/membench/SKILL.md#cost-interpretation) for
  live rollups and per-model usage.
- `hard / control`: optional local question sets under `runs/inputs/longmemeval-hard/`
  (`hard-tier2-cluster31.json`, `control-easy30.json`), only when present and covered by the run.
  Those sets are absent from a clean checkout.

To judge new hypotheses, use the [native benchmark workflow](../../../skills/membench/references/membench-commands.md);
this helper only reads its result.
