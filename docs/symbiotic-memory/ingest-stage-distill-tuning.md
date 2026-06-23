# Ingest Stage Distill Tuning

This document records the first evidence pass for tuning Symbiotic Memory's
Distillery flash stage separately from raw embedding. It is not a score claim.
The runs below are diagnostic ingest runs that intentionally stop before
Archive/fact embedding/index/briefs/answer/judge so prompt size and provider
timing can be compared without unrelated stage noise.

## Current Signal

The best candidate from this pass is a smaller distill window budget:

```text
SYMEM_DISTILL_WINDOW_MAX_INPUT_TOKENS=3000
```

This setting produced materially faster DeepSeek tail latency on the 10Q
stratified LongMemEval sample while retaining or increasing the number of
distilled facts. It should be treated as a candidate default, not final product
truth, until it is validated in a normal scored run.

## Reproduction

Run a distill-only arm:

```bash
RUN_NAME=target-10q-distillonly-flash-window3k-$(date -u +%Y%m%d-%H%M%S) \
LIMIT=10 \
SAMPLE=stratified \
SYMEM_DISTILL_WINDOW_MAX_INPUT_TOKENS=3000 \
scripts/run-ingest-stage-tuning.sh distill
```

Run raw embedding and distill concurrently, still stopping before later stages:

```bash
RUN_NAME=target-10q-rawembed-distill-flash-qwen-window3k-$(date -u +%Y%m%d-%H%M%S) \
LIMIT=10 \
SAMPLE=stratified \
SYMEM_DISTILL_WINDOW_MAX_INPUT_TOKENS=3000 \
scripts/run-ingest-stage-tuning.sh raw-embed-distill
```

Summarize comparable runs:

```bash
scripts/report-ingest-stage-tuning.sh --markdown runs/symbiotic-memory/long-mem-eval/10/<run-name> [...]
```

## Evidence Runs

All rows are 10-question stratified LongMemEval runs. DeepSeek rows are
`deepseek-v4-flash`; embedding rows are OpenRouter
`qwen/qwen3-embedding-8b`, 1024 dimensions, HTTP/1 32x32 transport, 32k char
batch target.

| run | mode | queue | calls | failed | input p50 | input p95 | input max | output p50 | output p95 | p50 | p95 | p98 | max | trace wall |
|---|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `target-10q-distillonly-flash-20260623-121342` | distill | chat | 323 | 0 | 4726 | 6352 | 8976 | 2919 | 5407 | 29.578s | 51.887s | 55.574s | 121.711s | 130s |
| `target-10q-rawembed-distill-flash-qwen-20260623-121605` | raw+distill | chat | 323 | 0 | 4726 | 6352 | 8976 | 3022 | 6079 | 30.344s | 56.082s | 67.540s | 95.687s | 105s |
| `target-10q-rawembed-distill-flash-qwen-20260623-121605` | raw+distill | embed | 167 | 0 | 8002 | 8030 | 8070 | 0 | 0 | 3.183s | 6.429s | 8.233s | 10.090s | 13s |
| `target-10q-qwen1024-h1-32x32-deepseek-h2-64x32-ingest-20260623-144505` | raw+distill | chat | 323 | 0 | 4726 | 6352 | 8976 | 2967 | 5779 | 20.668s | 38.960s | 43.796s | 64.972s | 76s |
| `target-10q-qwen1024-h1-32x32-deepseek-h2-64x32-ingest-20260623-144505` | raw+distill | embed | 167 | 0 | 8002 | 8030 | 8070 | 0 | 0 | 3.525s | 6.241s | 8.160s | 104.639s | 108s |
| `target-10q-distillonly-flash-window6k-20260623-122354` | distill | chat | 327 | 0 | 4736 | 6101 | 6583 | 2935 | 5857 | 29.359s | 53.484s | 60.546s | 81.579s | 91s |
| `target-10q-distillonly-flash-window3k-20260623-122613` | distill | chat | 503 | 0 | 3330 | 3732 | 4286 | 2566 | 5122 | 23.269s | 43.680s | 47.961s | 83.436s | 93s |
| `target-10q-rawembed-distill-flash-qwen-window3k-20260623-122825` | raw+distill | chat | 503 | 0 | 3330 | 3732 | 4286 | 2565 | 4918 | 22.275s | 41.310s | 46.036s | 59.302s | 69s |
| `target-10q-rawembed-distill-flash-qwen-window3k-20260623-122825` | raw+distill | embed | 167 | 0 | 8002 | 8030 | 8070 | 0 | 0 | 6.024s | 9.385s | 9.878s | 13.205s | 18s |

All rows had zero terminal provider failures. Queue wait and throttle wait were
effectively zero.

## Fact Count Check

Diagnostic fact counts come from `artifacts/step-analytics.json` adapter-call
metrics:

| run | facts total | p50 facts/source | max facts/source | turn count |
|---|---:|---:|---:|---:|
| `target-10q-distillonly-flash-20260623-121342` | 2390 | 244.5 | 276 | 0 |
| `target-10q-rawembed-distill-flash-qwen-20260623-121605` | 2413 | 238.0 | 314 | 4920 |
| `target-10q-qwen1024-h1-32x32-deepseek-h2-64x32-ingest-20260623-144505` | 2357 | 232.5 | 280 | 4920 |
| `target-10q-distillonly-flash-window6k-20260623-122354` | 2374 | 235.5 | 275 | 0 |
| `target-10q-distillonly-flash-window3k-20260623-122613` | 2986 | 304.0 | 329 | 0 |
| `target-10q-rawembed-distill-flash-qwen-window3k-20260623-122825` | 2825 | 281.5 | 326 | 4920 |

The 6k run barely changed call count or output shape, so this sample was mostly
turn-count limited at the default. The 3k run changed the shape: more windows,
more calls, smaller prompts, faster p95/p98, and more facts. That is a real
candidate, but it also means quality must be validated with a scored run before
making it the production default.

## Interpretation

1. Raw embedding and distill can overlap without choking DeepSeek on this 10Q
   sample. In the 3k mixed run, DeepSeek p95 improved to 41.310s and max to
   59.302s. The embedding p95 rose from 6.429s in the default mixed run to
   9.385s in the 3k mixed run, but embedding remained a short side branch.

2. Smaller distill prompts help the DeepSeek tail more than they hurt through
   extra request count, at least from 12k/default to 3k on this sample.

3. The 3k setting increases generated facts substantially. This could improve
   recall density or add noise. Only a normal end-to-end scored run can decide
   that.

4. These runs include 10 small prompt-cache warmup calls per 10Q run. The
   remaining DeepSeek calls are distill windows.

## Next Tests

- Run a normal 10Q scored pipeline with the 3k setting and compare answer
  quality, support density, and retrieval noise against the current default.
- If quality holds, test a midpoint such as 4k or 4.5k to see whether it keeps
  most of the latency gain with fewer calls and fewer extra facts.
- Add the same diagnostic isolation to query planning, answer context, support
  check, and answerer flash passes before tuning those stages. Do not infer
  their behavior from distill timing.
