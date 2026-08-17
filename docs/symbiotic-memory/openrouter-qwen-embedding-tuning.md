# OpenRouter Qwen Embedding Transport Tuning

This document records the evidence path for the OpenRouter
`qwen/qwen3-embedding-8b` raw-embedding transport settings used by Symbiotic
Memory benchmarks. It is not a score claim. It is a throughput and tail-latency
decision record for the raw embedding stage.

## Decision

Use this transport shape for OpenRouter Qwen raw embeddings unless a later run
supersedes it:

```text
operator/model: openrouter qwen/qwen3-embedding-8b
HTTP mode:      HTTP/1
client pool:    32
idle per host:  32
dimensions:     1024
batch max:      32k chars
workflow:       raw-embed-only tuning runs use 10Q stratified LongMemEval
```

Runtime environment:

```bash
MEMBENCH_EMBED_OPERATOR=openrouter
MEMBENCH_EMBED_MODEL=qwen/qwen3-embedding-8b
MEMBENCH_EMBED_DIMS=1024
MEMBENCH_EMBED_REQUEST_DIMS=1024
SYMBIOTIC_MEMORY__EMBED__BATCH_SIZE=250
SYMBIOTIC_MEMORY__EMBED__BATCH_MAX_CHARS=32000
MEMBENCH_EMBED_MAX_CHARS=32000
SYMBIOTIC_MEMORY__TRANSPORT__OPENROUTER_CLIENT_POOL_SIZE=32
SYMBIOTIC_MEMORY__TRANSPORT__POOL_MAX_IDLE_PER_HOST=32
SYMBIOTIC_MEMORY__TRANSPORT__HTTP1_ONLY=true
```

Use `scripts/run-embedding-transport-tuning.sh openrouter-qwen3-8b-1024 h1-32x32`
to reproduce the current selected shape.

## Why This Exists

The pipeline initially looked like it was draining embedding requests at a
steady, suspiciously low rate. We had already removed benchmark-stage caps and
confirmed provider queue caps were the intended source of model concurrency.
The remaining question was whether the slow drain came from queue throttling,
batch sizing, local database work, response decoding, HTTP pooling, or provider
tail behavior.

The raw-embedding transport tests isolate the embedding stage:

- `--stop-after-raw-embed` avoids Distillery, fact embedding, indexing,
  retrieval, answer, judge, and brief consolidation tails.
- OpenRouter Qwen is fixed as the embedding model.
- Zvec is kept as the store shape, but the measured provider queue timings are
  read from `provider-queue/model-queue-traces.jsonl`.
- Every listed run has `waitMax=0ms` and effectively zero throttle wait, which
  rules out provider queue throttling as the dominant source for these tails.

## Reproduction

Run one shape:

```bash
scripts/run-embedding-transport-tuning.sh openrouter-qwen3-8b-1024 h1-32x32
```

Summarize the evidence set:

```bash
scripts/report-embedding-transport-tuning.sh --profile openrouter-qwen3-8b-1024 --markdown
```

Promote a completed local evidence run to a dashboard-safe meta record:

```bash
scripts/save-run-meta-record.sh runs/symbiotic-memory/long-mem-eval/10/<run-name>
```

Meta records retain memory/model traces, step analytics, workflow queue state,
and provider queue timing. They omit vaults, raw outputs, raw provider request
payloads, and question-level artifacts (`hypotheses`, `provenance`, `scored`,
`verdicts`, and `partial-verdicts`) so the dashboard can show timing evidence
without carrying source data.

The runner intentionally uses `./target/release/membench` by default and fails
if that binary is missing. Build deliberately before paid runs:

```bash
CARGO_TARGET_DIR=target cargo build --release --manifest-path adapters/symbiotic-memory/Cargo.toml --bin membench
```

## Evidence Runs

All runs below are 10-question stratified LongMemEval, raw-embed-only,
OpenRouter Qwen, 1024 dimensions, 32k char batch target, 167 successful
embedding batches unless noted. The `failed attempts` column counts retryable
provider attempts, not terminal benchmark failures.

| run | params | n | failed attempts | p50 | p80 | p95 | p98 | max | wait max | throttle max |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `target-10q-qwen1024-h2pool4x64-rawembed-no-zvec-batch-flush-20260623-173447` | pool=4 idle=64 http1=default | 167 | 0 | 3.483s | 5.643s | 31.499s | 43.269s | 112.864s | 0ms | 0ms |
| `target-10q-qwen1024-h2pool4x64-rawembed-repeat-20260623-173900` | pool=4 idle=64 http1=default | 167 | 0 | 4.986s | 7.274s | 12.527s | 52.935s | 123.896s | 0ms | 0ms |
| `target-10q-qwen1024-http1-rawembed-20260623-174255` | pool=4 idle=64 http1=true | 167 | 0 | 6.106s | 8.104s | 19.536s | 42.427s | 88.461s | 0ms | 0ms |
| `target-10q-qwen1024-h2pool8x64-rawembed-20260623-174546` | pool=8 idle=64 http1=default | 167 | 0 | 7.607s | 28.267s | 37.816s | 41.514s | 50.487s | 0ms | 0ms |
| `target-10q-qwen1024-h2pool16x64-rawembed-20260623-174819` | pool=16 idle=64 http1=default | 167 | 0 | 3.388s | 12.067s | 20.106s | 33.649s | 42.856s | 0ms | 0ms |
| `target-10q-qwen1024-http1-pool16x64-rawembed-20260623-174942` | pool=16 idle=64 http1=true | 167 | 0 | 5.741s | 8.767s | 11.516s | 15.101s | 48.305s | 0ms | 0ms |
| `target-10q-qwen1024-h2pool64x16-rawembed-20260623-180637` | pool=64 idle=16 http1=default | 167 | 0 | 6.517s | 9.083s | 11.146s | 11.938s | 66.053s | 0ms | 0ms |
| `target-10q-qwen1024-h2pool32x32-rawembed-20260623-181354` | pool=32 idle=32 http1=default | 167 | 0 | 9.309s | 12.326s | 22.865s | 24.882s | 34.506s | 0ms | 1ms |
| `target-10q-qwen1024-http1-pool32x32-rawembed-20260623-102020` | pool=32 idle=32 http1=true | 167 | 1 | 5.334s | 7.562s | 10.871s | 11.825s | 15.309s | 0ms | 0ms |
| `target-10q-qwen1024-http1-pool64x16-rawembed-20260623-102400` | pool=64 idle=16 http1=true | 167 | 0 | 5.8s | 14.114s | 21.82s | 25.756s | 47.97s | 0ms | 0ms |

The selected run had one retryable DNS blip:

```text
attempt 1 failed in 15ms:
provider unavailable: error sending request for url
client error (Connect): dns error: failed to lookup address information
```

It retried and completed. There were no terminal failed batches.

## Path Through The Hypotheses

1. **Queue caps were suspected.** We removed duplicate stage-local caps and made
   provider/model queue settings the only model-concurrency authority. The
   evidence runs still had staggered responses with `waitMax=0ms`, so queue
   wait was not the explanation.

2. **Batch size was suspected.** Earlier tests showed very large batches and
   larger output vectors make response bodies expensive. We standardized this
   transport sweep on 1024 output dimensions and a 32k char batch target so the
   HTTP shape could be isolated.

3. **Distillery and answerer interference was suspected.** These runs use
   `--stop-after-raw-embed`, `--no-answerer`, `--no-consolidate-briefs`, and
   `--no-score`. That keeps DeepSeek, answer construction, retrieval, judging,
   and brief generation out of the measurement.

4. **Zvec or store writes were suspected.** The run still writes through the
   normal adapter path, but the table above measures provider queue run time.
   For the selected `h1 32x32` run, worst provider run time was 15.309s and
   memory traces showed store upsert time in the low millisecond range for raw
   embedding batches. Store work was not the observed long tail in this sweep.

5. **HTTP/2 multiplexing was suspected.** HTTP/2-capable shapes had good common
   cases but unstable tails. `h2 64x16` had p95/p98 near 11-12s but a 66s max.
   Repeating with `h1 64x16` removed the HTTP/2 variable but still produced a
   poor tail, so the issue was not simply "HTTP/2 bad"; fanout shape mattered.

6. **Balanced HTTP/1 fanout won.** `h1 32x32` had the best normal and tail
   profile in the comparable 10Q sweep: p50 5.334s, p95 10.871s, p98 11.825s,
   max 15.309s.

## How To Supersede This Decision

Create a new evidence row only when the run is comparable:

- same benchmark family: LongMemEval raw-embed-only;
- same model: `openrouter qwen/qwen3-embedding-8b`;
- same output dimensions unless testing dimensions explicitly;
- same batch target unless testing packing explicitly;
- report p50, p80, p95, p98, max, failed attempts, queue wait max, and throttle
  wait max;
- include at least one repeat if a single run changes the default.

Do not report raw provider speed as retrieval quality. Transport tuning only
decides how fast the embedding stage drains.
