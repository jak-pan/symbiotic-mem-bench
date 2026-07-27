# Environment

`membench` keeps benchmark credentials local to this repository.

## Variable Naming Contract

Two prefixes, two owners (the old `SYMEM_*` prefix is dead everywhere — commit `0e64b28` here,
CONFIG-TRIAGE in `../symbiotic-memory`):

- **`MEMBENCH_*` — benchmark-harness variables.** Everything the harness owns: role/model
  overrides (`MEMBENCH_<ROLE>_{OPERATOR,BASE_URL,MODEL,QUEUE_ID,MAX_TOKENS,THINKING,
  REASONING_EFFORT}` for the `DISTILL`, `ANSWER`, `EMBED`, `JUDGE`, `CONSOLIDATE`, `RERANK`
  roles), judge settings (`MEMBENCH_JUDGE_PROMPT_MODE`), the reranker gate
  (`MEMBENCH_RERANK`, `MEMBENCH_RERANK_MODEL`, `MEMBENCH_RERANK_STAGE1_*`), embedding adapter
  parameters (`MEMBENCH_EMBED_DIMS`, `MEMBENCH_EMBED_REQUEST_DIMS`, `MEMBENCH_EMBED_MAX_CHARS`),
  workflow fan-out (`MEMBENCH_WORKFLOW_MAX_IN_FLIGHT`, `_MAX_ATTEMPTS`, `_RETRY_DELAY_SECS`),
  run plumbing (`MEMBENCH_PROVIDER_QUEUE_DIR`, `MEMBENCH_RESPONSE_CACHE_DIR`,
  `MEMBENCH_TRACE_JSONL[_PATH]`, `MEMBENCH_QUEUE_TRACE_JSONL[_PATH]`,
  `MEMBENCH_QUESTION_TIMEOUT_SECS`), and experiment helpers (`MEMBENCH_REDO`,
  `MEMBENCH_ORACLE_*`, `MEMBENCH_REEMBED_*`, `MEMBENCH_IGNORE_SOURCE_HASH`,
  `MEMBENCH_REFERENCE_DATETIME`, `MEMBENCH_VAULT_STORE`, `MEMBENCH_CONSOLIDATOR`,
  `MEMBENCH_SUPERSESSION_DETECTION`).
- **`SYMBIOTIC_MEMORY__SECTION__FIELD` — engine config overrides.** These map mechanically onto
  the kit's typed config keys (`SYMBIOTIC_MEMORY__EMBED__BATCH_SIZE` → `embed.batch_size`).
  membench resolves this layer into the kit's typed config at run start; the engine itself reads
  no environment variables, and the resolved config hash is stamped into the run record (and echoed
  with the overridden keys in the run log). Full knob reference:
  `../symbiotic-memory/docs/RUNBOOK.md`.

There is no such thing as a "bench-owned" engine knob: anything steering memory behavior goes
through `SYMBIOTIC_MEMORY__*` config keys like every other host's settings.

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

## Dependency Sources

The Symbiotic Memory adapter dependencies (`symbiotic-memory`, `symbiotic-memory-config`,
`symbiotic-core`, `symbiotic-queue`) are pinned git revisions in `Cargo.toml`. The foundation
repository is public; the transferred `symbiotic-sh/symbiotic-memory` repository is currently
private, so adapter builds need read access to that repository. The pinned revision contains the
required provider-role and queue APIs and builds without a sibling override once authenticated.

For intentional co-development against sibling checkouts, use this optional gitignored
`.cargo/config.toml` at the repository root:

```toml
[patch."ssh://git@github.com/symbiotic-sh/symbiotic-memory"]
symbiotic-memory = { path = "../symbiotic-memory" }
symbiotic-memory-config = { path = "../symbiotic-memory/config" }

[patch."https://github.com/symbiotic-sh/symbiotic-foundation"]
symbiotic-core = { path = "../symbiotic-foundation/crates/symbiotic-core" }
symbiotic-queue = { path = "../symbiotic-foundation/crates/symbiotic-queue" }
```

Without the override, Cargo uses the exact remote revisions and
`CARGO_NET_GIT_FETCH_WITH_CLI=true` may be needed when credentials live in the git CLI. Anonymous
adapter builds remain blocked while the memory repository is private; see
`docs/oss-release-handoff.md`.

The f6 build graph also requires a verified native zvec package for the host target. Given a clean
canonical `symbiotic-sh/symbiotic-memory` checkout whose `HEAD` equals
`.symbiotic-memory-pin`, prepare it before Cargo:

```bash
target="$(rustc -vV | sed -n 's/^host: //p')"
zvec_dir="$(mktemp -d "${TMPDIR:-/tmp}/membench-zvec.XXXXXX")"
./scripts/prepare-adapter-zvec.sh /path/to/symbiotic-memory "$zvec_dir" "$target"
export ZVEC_LIB_DIR="$zvec_dir"
export LIBRARY_PATH="$zvec_dir${LIBRARY_PATH:+:$LIBRARY_PATH}"
export LD_LIBRARY_PATH="$zvec_dir${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" # Linux
export DYLD_LIBRARY_PATH="$zvec_dir${DYLD_LIBRARY_PATH:+:$DYLD_LIBRARY_PATH}" # macOS
```

The wrapper validates the checkout identity, pin, index state, and cleanliness, then delegates to
the pinned upstream `scripts/zvec-package.sh` and writes a marker binding the resulting directory
to that source pin and target. Adapter builds require that exact marker; they never search sibling
directories or Cargo caches for a first matching library. The exact source/tag object and commit,
digest-pinned Linux builder image, target architecture, and SPDX SBOM and its hash are independent
anchors. The native library digest is a self-consistency check: it is calculated during the build,
written to provenance, and checked against that same build output rather than against a separately
pinned expected binary digest. CI uses `x86_64-unknown-linux-gnu`, exports these paths through
`GITHUB_ENV`, and intentionally does not cache Cargo target, native-build, or private-derived
artifacts.

## Required Keys

Paid Symbiotic Memory LongMemEval runs on the owner-default stack currently need:

```text
DEEPSEEK_API_KEY      # distill, answer, query planning, judging
OPENROUTER_API_KEY    # qwen3-embedding-8b embeddings + the free nemotron reranker
```

`GEMINI_API_KEY` is only needed for explicit `--embedder gemini` comparison arms. Optional provider
keys may be added for experimental adapters, but keep real values only in ignored local files.

## Model And Queue Defaults

The templates document the current owner-default LongMemEval stack:

- DeepSeek Flash for Distillery, query planning, and judging (judge thinking disabled, 64 output
  tokens by code default unless explicitly overridden); DeepSeek Pro for answer generation.
- OpenRouter `qwen/qwen3-embedding-8b` embeddings at 1024 dims (`--embedder openrouter`, the CLI
  default). Gemini Embedding 2 remains the `--embedder gemini` comparison arm, in multi-input
  request mode with request packing.
- Cross-encoder reranking ON by default (`nvidia/llama-nemotron-rerank-vl-1b-v2:free`); disable
  per run with `MEMBENCH_RERANK=0`, swap models with `MEMBENCH_RERANK_MODEL`.
- Embedding request packing and memory-local chunking budgets are owned by `symbiotic-memory`
  code/config (`embed.*` keys). These are throughput/retry defaults, not model window limits.
- Distill prompt-cache prewarm is enabled by default. Set
  `SYMBIOTIC_MEMORY__DISTILL__PREWARM_CACHE=false` only for a deliberate no-prewarm comparison.
- Embedding per-input local text cap is controlled by `MEMBENCH_EMBED_MAX_CHARS`; batch packing
  must not truncate individual inputs.
- Distillery windows of 16 source turns by default. Set
  `SYMBIOTIC_MEMORY__DISTILL__TURNS_PER_WINDOW=1` only for an explicit atomic-turn experiment; it
  is too many paid model calls for normal runs.
- Reweave brief generation (the consolidate pass) is opt-in via `MEMBENCH_CONSOLIDATOR=llm`.

Queue and cache overrides are intentionally commented out. Prefer the selected memory YAML profile
(`queue.models."operation:operator:model"`) for normal runs; use env overrides only for explicit
experiments.

## Run Roots

Provider queues, model traces, and response-cache state belong under each run root unless a cache
root is explicitly overridden. This keeps scratch runs reproducible and prevents unrelated benchmark
attempts from silently sharing partial state.

For prompt forensics, set `SYMBIOTIC_MEMORY__QUEUE__DEBUG_REQUESTS=true` for the run. Native Symbiotic
Memory runs then write raw provider requests under
`provider-queue/requests/{chat,embedding}/{input_hash}.json`, matching the `input_hash` in
`provider-queue/model-queue-traces.jsonl`. This is deliberately off by default because it stores
full system/user prompts and embedding inputs, including source text.
