# Environment

`membench` keeps benchmark credentials local to this repository.

## Local Files

Tracked templates:

```text
.env.example
.env.test.local.example
```

Ignored local files:

```text
.env
.env.*
```

Create the normal local provider environment with:

```bash
cp .env.example .env.test.local
```

`membench` loads `.env.test.local` from this repository by default for native provider-backed runs.
It does not implicitly read sibling repository env files. Use `--env-file path/to/file` only when
intentionally testing a different environment.

## Required Keys

Paid Symbiotic Memory LongMemEval runs currently need:

```text
DEEPSEEK_API_KEY
GEMINI_API_KEY
```

Optional provider keys may be added for experimental adapters, but keep real values only in ignored
local files.

## Model And Queue Defaults

The templates document the current raw-light LongMemEval defaults:

- DeepSeek Flash for Distillery and query planning.
- DeepSeek Pro for answer generation.
- DeepSeek Flash for judging, with thinking disabled and 64 output tokens by code default unless
  explicitly overridden.
- Gemini Embedding 2 for embeddings.
- Embedding multi-input request mode with request packing. Numeric defaults are owned by
  `symbiotic-memory` code/config. These are throughput/retry defaults, not model window limits.
- Memory-local chunking uses approximate token budgets before provider calls. Numeric defaults are
  owned by `symbiotic-memory` code/config.
- Distill prompt-cache prewarm is enabled by default. Set `SYMBIOTIC_MEMORY__DISTILL__PREWARM_CACHE=false` only for a
  deliberate no-prewarm comparison.
- Embedding per-input local text cap is controlled by `MEMBENCH_EMBED_MAX_CHARS`; batch packing must not
  truncate individual inputs.
- Distillery windows of 16 source turns by default. Set `SYMBIOTIC_MEMORY__DISTILL__TURNS_PER_WINDOW=1` only for
  an explicit atomic-turn experiment; it is too many paid model calls for normal runs.

Queue and cache overrides are intentionally commented out. Prefer the selected memory YAML profile
for normal runs; use env overrides only for explicit experiments.

## Run Roots

Provider queues, model traces, and response-cache state belong under each run root unless a cache
root is explicitly overridden. This keeps scratch runs reproducible and prevents unrelated benchmark
attempts from silently sharing partial state.

For prompt forensics, set `SYMBIOTIC_MEMORY__QUEUE__DEBUG_REQUESTS=true` for the run. Native Symbiotic
Memory runs then write raw provider requests under
`provider-queue/requests/{chat,embedding}/{input_hash}.json`, matching the `input_hash` in
`provider-queue/model-queue-traces.jsonl`. This is deliberately off by default because it stores
full system/user prompts and embedding inputs, including source text.
