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
- Gemini embedding multi-input request mode with batch size 16.
- Distillery windows of 16 source turns by default. Set `SYMEM_DISTILL_TURNS_PER_WINDOW=1` only for
  an explicit atomic-turn experiment; it is too many paid model calls for normal runs.

Queue and cache overrides are intentionally commented out. Prefer the selected memory YAML profile
for normal runs; use env overrides only for explicit experiments.

## Run Roots

Provider queues, model traces, and response-cache state belong under each run root unless a cache
root is explicitly overridden. This keeps scratch runs reproducible and prevents unrelated benchmark
attempts from silently sharing partial state.
