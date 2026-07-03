---
name: oracle-test
description: Run an oracle-gold answerer/prompt A/B on LongMemEval-500 — feeds gold evidence straight to a chosen reader model + answer prompt, grades with the official judge, then prints accuracy-by-type and canonical cost. Use whenever the user wants to test a reader model or an answer prompt in isolation (the "reader ceiling" method), or compare answerers/prompts. Each run is a paid OpenRouter call (~$0.4-2.7).
---

# Oracle answerer/prompt test (reader-ceiling isolation)

Holds retrieval perfect (gold-only evidence via `--oracle-gold`) so the only variables are the
**reader model** and the **answer prompt**. No ingest, so it's cheap and fast (~8 min, $0.4-2.7).

```
scripts/oracle-test.sh <operator/model> [prompt-dir] [run-name] [baseline-run]
```

- `<operator/model>` — OpenRouter id (operator forced to `openrouter`): `qwen/qwen3.7-plus`,
  `google/gemini-3.5-flash`, `qwen/qwen3.6-35b-a3b`, …
- `[prompt-dir]` — answer-prompt dir. Default `/tmp/prompts-v3` (production full prompt). Pass
  `/tmp/prompts-min` (or any dir with an `answer.yaml`) to test a different prompt.
- `[run-name]` — defaults to `<model-slug>-<prompt-slug>-500`.
- `[baseline-run]` — optional existing run to print alongside for the A/B.

It runs membench (answer-only, oracle-gold, official judge), then calls `score-run.sh` to print
**by-type accuracy** and **canonical cost** for the new run (and the baseline if given).

## Examples

```
# Test a minimal answer prompt on qwen3.7-plus vs the full-prompt baseline:
scripts/oracle-test.sh qwen/qwen3.7-plus /tmp/prompts-min qwen37p-min-500 qwen37p-500

# Same prompt, cheaper reader — does the cheap model hold up?
scripts/oracle-test.sh qwen/qwen3.6-35b-a3b /tmp/prompts-min
```

## Notes

- **Paid + serialized.** One membench paid run at a time (the binary enforces the lock); don't
  launch two oracle-tests concurrently. Needs `OPENROUTER_API_KEY` in `./.env.test.local`.
- **Cost is canonical.** The reported cost is the `cost.rs` rollup priced from
  `config/pricing/openrouter-pricing.json` (OpenRouter catalog) + the native static table — every
  model, with prompt-cache discount. Refresh prices with `scripts/refresh-pricing.sh`.
- **Per-token cheap ≠ per-run cheap.** Thinking-on models emit very different output volumes
  (qwen3.6-35b ran 3.2× the tokens of qwen3.7-plus for the same input → pricier per run despite a
  lower rate). The per-run cost is the one that matters; this skill reports it.
- **Default thinking is on** (`MEMBENCH_ANSWER_THINKING=on`); override via env. Override `VAULT`/`DS`
  via env to point at a different oracle bed.
- Reuses `score-run` for the by-type/cost readout, so the two stay consistent.
