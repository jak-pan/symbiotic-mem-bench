# LongMemEval Judging — Official Per-Type Grader

Answers are graded with the **exact LongMemEval per-question-type LLM grader from the paper**
(Wu et al., *LongMemEval*, arXiv:2410.10813, §A.3 / Fig. 10). Verified character-for-character
against the paper (ar5iv HTML, 2026-06). This is the **default** grader.

Code: `judge_prompt()` and `judge_one_longmemeval()` in `src/bin/membench.rs`.

## Per-type prompts

Selection is by `question_type`:

| `question_type` | grader | distinguishing rule |
|---|---|---|
| `temporal-reasoning` | temporal | off-by-one days/weeks/months is OK (predicting 19 when the answer is 18 = correct) |
| `knowledge-update` | KU | a response with previous info **plus** an updated answer is correct as long as the updated answer is present |
| `single-session-preference` | preference | graded against the **rubric**; needn't hit every point; correct if it recalls + uses the user's info |
| `multi-session`, `single-session-user`, `single-session-assistant` | default / "other" | contains the answer (or all intermediate steps) → yes; only a subset → no |

User message format: `Question: …` / `Correct answer: …` (or `Rubric for desired personalized
response:` for preference) / `Response from the model: …` / `Answer yes or no.`

## Abstention

Per the paper, abstention is **not a separate grader**. The 30 `_abs` questions (false-premise
variants drawn from the other types) are graded by the same per-type framework — the gold is "the
information is unavailable" and a correct response abstains. In our code `question_type` drives the
per-type prompt; `is_abstention` is a **reporting-only** flag (the abstention-accuracy breakdown).
LongMemEval-S = **500 questions, 30 abstention**.

## Modes — `SYMEM_JUDGE_PROMPT_MODE`

- unset / `official` / `paper` / `longmemeval` → the per-type paper grader (**default**).
- `semantic` / `semantic-shared-compact` / `legacy` / `generic` → the old generic semantic grader
  (kept for back-compat and A/B comparison).

The mode is recorded per-run in `scored.json` / `benchmark-report.json` and surfaced in the
dashboard (`JUDGE·MODE` in the Overview "Cohort & Models" panel; shows `legacy` for runs predating
the field).

## Judge model

- Default **`deepseek-v4-flash`** (override with `SYMEM_JUDGE_MODEL`).
- The paper used **GPT-4o** (reported 97% agreement with human experts). We run deepseek-flash with
  the *identical* prompts — same rubric, far cheaper, minor model-dependent disagreements possible.
  For exact paper replication: `SYMEM_JUDGE_MODEL=openai/gpt-…` then `--rejudge`.

## Re-judging without re-answering — `--rejudge`

```
membench --symbiotic-memory --long-mem-eval --rejudge \
  --run-name <existing-run> --dataset <ds> --limit <n> --sample <s> \
  --distiller llm --embedder openrouter --store sqlite --memory-config <cfg> --memory-manifest <m>
```

Skips the entire ingest/recall/answer phase, reads the run's existing `hypotheses.jsonl`, re-grades
with the current judge, and rewrites `verdicts.jsonl` / `score-summary.json` / `benchmark-report.json`
in place. Cheap (judge calls only). Use it to swap graders or re-score after a judge change without
re-spending on answering.

## Audit trail — the full judge input is captured

Every verdict in `verdicts.jsonl` carries the **complete judge input** (mirrors the answerer call
trace), so a verdict *proves* which prompt graded it rather than inferring from the mode flag:

- `judge_system_prompt` — the exact per-type system prompt sent.
- `judge_user_prompt` — the rendered user message (question + gold/rubric + model response).
- (`judge_raw` = the judge's reply; `label` = the parsed verdict.)

Both are optional (absent on runs scored before the field existed). Inspect:

```
jq -rs '[.[]|select(.question_type=="temporal-reasoning")][0]
        | "[SYSTEM]\n"+.judge_system_prompt+"\n\n[USER]\n"+.judge_user_prompt' \
   runs/symbiotic-memory/long-mem-eval/<limit>/<run>/artifacts/verdicts.jsonl
```

A spot check on a real temporal verdict shows the system carries the off-by-one clause and the user
carries the question + gold + model answer — i.e. the literal per-type prompt was sent.

## Validation

On the 61-question hard+control set, a **clean `--rejudge` (same stored answers) with the official
per-type grader scored identically to the old generic grader** — all 5 reader models, every category.
So our earlier numbers were faithful and a full-500 re-judge was not warranted. (The apparent ±1–3
deltas seen when *re-answering* under the official judge were OpenRouter non-determinism at temp 0,
not the judge.)

*Last updated: 2026-06-29.*
