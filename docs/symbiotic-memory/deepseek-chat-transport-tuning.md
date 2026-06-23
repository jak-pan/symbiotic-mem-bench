# DeepSeek Chat Transport Tuning

This document records the first transport tuning pass for DeepSeek
`deepseek-v4-flash` chat calls in Symbiotic Memory's Distillery path. It is a
throughput and tail-latency decision record, not a memory-quality or retrieval
score claim.

## Decision

The current default for DeepSeek distill chat transport is:

```text
operator/model: deepseek deepseek-v4-flash
HTTP mode:      HTTP/2-capable default
client pool:    64
idle per host:  32
workflow:       distill-only tuning runs use 10Q stratified LongMemEval
```

Runtime environment:

```bash
SYMEM_DISTILL_OPERATOR=deepseek
SYMEM_DISTILL_MODEL=deepseek-v4-flash
SYMEM_CHAT_HTTP_CLIENT_POOL_SIZE=64
SYMEM_CHAT_HTTP_POOL_MAX_IDLE_PER_HOST=32
SYMEM_CHAT_HTTP_HTTP1_ONLY=0
```

Use this command to reproduce the selected shape:

```bash
scripts/run-chat-transport-tuning.sh deepseek-v4-flash-distill h2-64x32
```

This is now the product default for DeepSeek chat targets. The first 10Q sweep
shows `h2 32x32` had better p50/p80/p95, but `h2 64x32` had the lowest observed
worst-tail provider call. The Distillery default optimizes for avoiding very
long tails while keeping queue wait and throttle wait at zero. Both `h1 128x16`
and `h2 128x16` regressed common-case latency and did not supersede `64x32`.

## Why This Exists

OpenRouter Qwen embedding transport had already been tuned with a multi-client
pool. DeepSeek chat had not. Before this pass, OpenAI-compatible chat providers
used one shared `reqwest::Client`, so the OpenRouter embedding transport setting
was not a general setting for all providers.

The code now exposes a separate chat transport pool for DeepSeek and other
OpenAI-compatible chat providers:

```text
SYMEM_CHAT_HTTP_CLIENT_POOL_SIZE
SYMEM_CHAT_HTTP_POOL_MAX_IDLE_PER_HOST
SYMEM_CHAT_HTTP_HTTP1_ONLY
```

Those are below the provider queue. They do not change the workflow
`max_in_flight`, model catalog, retry policy, RPM/TPM accounting, prompt shape,
or token budgets.

## Reproduction

Run one shape:

```bash
scripts/run-chat-transport-tuning.sh deepseek-v4-flash-distill h2-64x32
```

Summarize selected runs:

```bash
scripts/report-chat-transport-tuning.sh --profile deepseek-v4-flash-distill --markdown
```

The runner uses `./target/release/membench` and fails if the release binary is
missing. Build deliberately before paid runs:

```bash
cargo build --release --features symbiotic-memory-adapter --bin membench
```

## Evidence Runs

All rows below are 10-question stratified LongMemEval, distill-only, DeepSeek
`deepseek-v4-flash`, hash embeddings, SQLite store, no answerer, no brief
consolidation, and no scoring. The intent is to isolate Distillery chat
transport. The `failed attempts` column counts retryable provider attempts, not
terminal benchmark failures.

| run | params | n | failed attempts | p50 | p80 | p95 | p98 | max | wait max | throttle max |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `ds-chat-h2-1x64-10q-20260623-133307` | pool=1 idle=64 http1=false | 323 | 0 | 24.63s | 34.235s | 45.59s | 54.938s | 83.024s | 0ms | 0ms |
| `ds-chat-h1-32x32-10q-20260623-133442` | pool=32 idle=32 http1=true | 323 | 0 | 25.078s | 34.54s | 44.346s | 50.415s | 98.627s | 45ms | 5ms |
| `ds-chat-h1-128x16-10q-20260623-141450` | pool=128 idle=16 http1=true | 323 | 0 | 29.902s | 42.211s | 56.719s | 65.181s | 79.456s | 0ms | 4ms |
| `ds-chat-h2-32x32-10q-20260623-133649` | pool=32 idle=32 http1=false | 323 | 0 | 23.213s | 34.638s | 46.306s | 55.751s | 72.631s | 0ms | 0ms |
| `ds-chat-h2-64x32-10q-20260623-134728` | pool=64 idle=32 http1=false | 324 | 0 | 25.046s | 37.392s | 48.928s | 54.752s | 69.787s | 0ms | 0ms |
| `ds-chat-h2-128x16-10q-20260623-141156` | pool=128 idle=16 http1=false | 323 | 0 | 29.359s | 40.503s | 53.012s | 61.112s | 96.065s | 0ms | 0ms |

## Interpretation

1. The queue is not the bottleneck in these runs. Queue wait and throttle wait
   are zero or effectively zero.

2. Forced HTTP/1 did not repeat the OpenRouter embedding result. `h1 32x32`
   had the best p95/p98 in this tiny sweep, but it had the worst max and worse
   wall tail.

3. HTTP/2-capable `h2 64x32` is the current default because it lowered the
   single worst provider call from 72.631s to 69.787s while preserving zero
   queue wait and zero throttle wait. `h2 32x32` remains the better common-case
   shape in this evidence set and should be re-tested if future Distillery work
   optimizes p50/p80 more than worst-tail. `h1 128x16` lowered the max relative
   to `h1 32x32`, but worsened p50, p80, p95, and p98. `h2 128x16` was worse on
   p50, p80, p95, p98, and max.

4. This transport tuning is separate from distill window-size tuning. Smaller
   distill windows can change prompt size, request count, output facts, and
   answer quality. Transport tuning only changes how already-admitted chat
   work is spread across HTTP clients.

## How To Supersede This Decision

Create a comparable row only when the run keeps the measurement isolated:

- same benchmark family: LongMemEval distill-only;
- same model: `deepseek deepseek-v4-flash`;
- hash or otherwise local embedding so embedding providers are not competing;
- no answerer, no briefs, no scoring;
- report p50, p80, p95, p98, max, failed attempts, queue wait max, and throttle
  wait max;
- include a repeat before changing the default transport shape.

Do not report chat transport speed as memory accuracy. This only decides how
fast Distillery chat calls drain.
