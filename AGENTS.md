# Agent Instructions

This repository is the neutral benchmark harness for memory systems. Keep it easy for the next
agent to run, inspect, and publish results without reverse-engineering local scratch state.

## Core Rules

- Work from this repository root when possible.
- Prefer `cargo run --manifest-path adapters/symbiotic-memory/Cargo.toml --bin membench -- ...`
  over ad hoc scripts, and use `cargo run --release` for paid/provider-backed benchmark runs.
- Do not add Python scoring scripts or manual score entry paths.
- Do not commit `runs/`, `.debug-session/`, `target/`, secrets, provider queues, or raw local
  datasets.
- Use `CARGO_TARGET_DIR=/tmp/symbiotic-mem-bench-target` for validation when preparing the repo for
  publication, so `target/` is not recreated in the repo.
- Keep benchmark records portable: repo-relative paths in reports, no local absolute source paths.
- Do not store raw prompts or secrets in tracked records. Raw content is allowed only in explicit
  local debug artifacts under ignored scratch paths.
- GitHub Actions must not use hosted macOS runners. Membench, its private Memory adapter, and its
  release bundles run on Linux. Only the separate Symbiotic iOS application may require macOS CI.

## Validate

```bash
CARGO_TARGET_DIR=/tmp/symbiotic-mem-bench-target cargo fmt -- --check
CARGO_TARGET_DIR=/tmp/symbiotic-mem-bench-target cargo test
```

If you need to run from another repository, use:

```bash
CARGO_TARGET_DIR=/tmp/symbiotic-mem-bench-target \
  cargo test --manifest-path ../symbiotic-mem-bench/Cargo.toml
```

## Codex Skill

The repo ships a Codex skill at `skills/membench/SKILL.md`. Use it as the compact workflow entry
point for benchmark runs, imports, explorer usage, publication checks, and run-shape rules. Its
longer command reference lives at `skills/membench/references/membench-commands.md`.

## Run Registry Shape

Scratch runs:

```text
runs/{system}/{benchmark}/{limit}/{run_name}/
```

Tracked curated records:

```text
records/{system}/{benchmark}/{limit}/{run_name}/
```

Every complete run should contain:

```text
run-params.json
benchmark-report.json
artifacts/
```

Use the same shape for all systems, not just Symbiotic Memory.

For native runs, `artifacts/` is the portable public bundle and `raw/` keeps executor-native outputs.
Do not delete `vaults/`, `workflow/`, or `provider-queue/` from local scratch runs unless you are
intentionally making a minimal exported record.

Provider/model trace facts:

- Native Symbiotic Memory provider calls are emitted under
  `provider-queue/model-queue-traces.jsonl`.
- New completed native runs also export that file as `artifacts/model-traces.jsonl`.
- Older runs may show `model_traces` missing in their manifest but still have provider traces under
  `provider-queue/`; the dashboard live/detail paths read that fallback directly.
- In the live monitor, provider queue counts are latest state per queue item, not cumulative event
  counts. `running` should drop after terminal events.
- Live per-queue `rpm`, `maxrpm`, and `peak` are observed from tailed provider events. They are
  useful for diagnosing pressure and async flow, but are not a replacement for full-run cost or
  provider billing summaries.
- Completed runs show queue `avg`/`avgrpm` summaries derived from provider event timing instead of
  emphasizing current active counts, which should naturally be zero after completion.
- The live activity stream should show memory-stage events and provider events interleaved. If stages
  appear wave-like in the bars, inspect activity before concluding the executor is sequential.
- Memory setup, ingest, and recall stages come from the public Memory trace sink. The benchmark
  does not inspect backend files, manifests, indexes, or storage-open metrics.
- The stage label `recall setup` maps to adapter `pre_recall_setup`: post-ingest count loading and
  recall-index readiness before the answer/recall path starts.
- The stage label `briefs` maps to the trace operation `consolidate`, which is the source-backed
  extractive brief pass.
- The stage label `prompt plan` maps to the trace operation `query_plan`. It is emitted from the
  memory engine's recall debug result; the benchmark must not run an extra planner call for display.
  Inspect `recall.planner` in the per-question facade debug bundle for the raw planner prompt and
  response; memory traces keep hashes/pointers.

Imported runs can be artifact-only. Their `artifact_manifest.native_state_available` must be `false`,
and their `artifact_manifest.missing` list must make absent traces or state folders explicit.

## Native Symbiotic Memory Run

Paid provider-backed runs read `.env.test.local` from this benchmark repository by default. Start
from the tracked template:

```bash
cp .env.example .env.test.local
```

Do not commit real `.env` files. Use `--env-file path/to/file` only when intentionally testing a
different environment; the runner does not implicitly load sibling repo env files. See
`docs/environment.md` for the env file and queue/cache boundary.

Run native Symbiotic Memory benchmarks through the adapter:

```bash
CARGO_TARGET_DIR=/tmp/symbiotic-mem-bench-target cargo run --release \
  --manifest-path adapters/symbiotic-memory/Cargo.toml \
  --bin membench -- \
  --system symbiotic-memory \
  --benchmark long-mem-eval \
  --limit 50
```

Current adapter status:

```text
wired:   explicit `--smoke` local no-network run-shape tests
wired:   default paid provider-backed LLM/Gemini ingestion and queued LongMemEval scoring
pending: compact normalized `artifacts/model-traces.jsonl` export
```

Do not restore `symem` benchmark subcommands. Benchmark orchestration, scoring, records, and
dashboard artifacts belong in this repository.

Symbiotic Memory model/provider defaults are owned by `../symbiotic-memory` code and config. Keep
membench profiles focused on benchmark policy such as recall shape, scorer choice, dataset sampling,
or an explicitly named tuning arm. Do not inject answer/distill/embed model env defaults from the
harness just to mirror production defaults.

Default launches are paid, provider-backed, scored, and must run in Cargo release mode. A local
no-network smoke test must be requested explicitly:

```bash
CARGO_TARGET_DIR=/tmp/symbiotic-mem-bench-target cargo run \
  --manifest-path adapters/symbiotic-memory/Cargo.toml \
  --bin membench -- \
  --system symbiotic-memory \
  --benchmark long-mem-eval \
  --smoke
```

Explicit local smoke tests map internally to local deterministic providers and no scorer.
They are temporary by default: without an explicit `--run-root` or hidden `--keep-smoke-run`, they
run outside the dashboard registry and delete themselves after success.

`membench` chooses a run root automatically:

```text
runs/symbiotic-memory/long-mem-eval/{limit}/{timestamp-shortid}/
```

Normal native benchmark runs are fresh by default. `membench` passes `--fresh`, resets the run root,
and re-ingests every selected question from source. Only reuse state when explicitly testing resume
behavior:

```text
default      fresh re-ingest
--resume     continue an interrupted run root
--answer-only reuse an already ingested run root
```

Answer-only native reruns default to `workflow_max_in_flight=500` because they only fan out
recall/answer work. Normal ingest runs use the configured workflow window. In both cases model
concurrency is still enforced by provider queue id, not by benchmark stage.

Paid provider-backed native runs also take a repo-local single-process lock at
`runs/.locks/paid-provider-run.lock`. This intentionally keeps large benches one-at-a-time while
each run keeps its own `provider-queue/` state. If a run is killed, inspect
`runs/.locks/paid-provider-run.lock/owner.json` and remove the lock only after confirming that
process is dead.

Only pass `--run-root` for a deliberate, named local run. If you do pass `--run-root` without
`--resume` or `--answer-only`, expect it to be reset.

## Async Adapter Contract

Adapters must preserve the system under test's native pipeline:

- no duplicate benchmark-owned ingestion, embedding, or recall logic;
- no artificial phase barrier between capture, Distillery, raw embedding, fact embedding, indexing,
  answer, and score;
- write stage outputs and traces incrementally;
- acknowledge durable work only after outputs are written;
- cap provider concurrency by model queue id, not by benchmark stage name.

## Import Existing Artifacts

Use imports when artifacts already exist and should be normalized without regenerating answers:

```bash
cargo run --manifest-path adapters/symbiotic-memory/Cargo.toml --bin membench -- \
  --system symbiotic-memory \
  --benchmark long-mem-eval \
  --import-report \
  --run-name baseline-clean \
  --hypotheses path/to/hypotheses.jsonl \
  --verdicts path/to/verdicts.jsonl \
  --partial-verdicts path/to/partial-verdicts.jsonl \
  --memory-traces path/to/memory-traces.jsonl \
  --model-traces path/to/model-traces.jsonl \
  --scored path/to/scored.json
```

The importer derives `{limit}` from `scored.json`, copies artifacts into the run folder, and records
only portable artifact presence and missing-artifact metadata in `run-params.json`.

## Explore And Promote

List runs:

```bash
cargo run --manifest-path adapters/symbiotic-memory/Cargo.toml --bin membench -- explore
```

Inspect one run:

```bash
cargo run --manifest-path adapters/symbiotic-memory/Cargo.toml --bin membench -- explore \
  --run-root runs/symbiotic-memory/long-mem-eval/500/baseline-clean
```

Promote a local run to tracked records:

```bash
cargo run --manifest-path adapters/symbiotic-memory/Cargo.toml --bin membench -- save-record \
  --run-root runs/symbiotic-memory/long-mem-eval/500/baseline-clean
```

Records must keep the same `{system}/{benchmark}/{limit}/{run_name}` shape.

## Trials

Use trials to record improvement experiments from existing debug artifacts. Do not hand-edit the
JSON/JSONL ledger when the source runs exist; derive it from run artifacts:

```bash
cargo run --manifest-path adapters/symbiotic-memory/Cargo.toml --bin membench -- trials derive \
  --trial-run-root runs/symbiotic-memory/long-mem-eval/50/candidate \
  --comparison-run-root runs/symbiotic-memory/long-mem-eval/50/previous \
  --original-baseline-run-root runs/symbiotic-memory/long-mem-eval/500/baseline \
  --change-title "Evidence grouping clarification" \
  --reasoning "Observed retrieved evidence present but answerer mixed lanes; test generic prompt wording." \
  --changed-file "../symbiotic-memory/src/recall/prompt_policy.rs:120|answer prompt|Clarify lane boundaries" \
  --verification "cargo test --manifest-path ../symbiotic-memory/Cargo.toml prompt_ --features cli" \
  --decision "diagnostic_only"
```

This writes `runs/analysis/trial-{generated-change-id}/trial-stack.json`, `trials.jsonl`, and
`trial-question-deltas.jsonl`. `--stack-id` and `--change-id` are generated by default from the
change title and compared run roots; pass explicit ids only when intentionally grouping several
trial rows into one ledger. The command computes wins/regressions from verdicts and hypotheses,
links question-debug bundles by path/hash, and keeps raw prompts out of the trial ledger.

Use focused sub-25Q trial stacks for tight failure-class forensics. Use stratified 25-50Q trial stacks
before drawing broader diagnostic conclusions. Only full benchmark-scale runs can become benchmark
claims.

The dashboard flags benchmark runs as `TRIAL` when they are referenced by
`runs/analysis/{stack_id}/trials.jsonl`. Treat those rows as diagnostic improvement experiments, not
publishable benchmark claims, unless a separate full benchmark run is promoted to records.

## Naming Runs

Use neutral names that describe the benchmark condition, not private debugging history.

Good:

```text
baseline-clean
candidate-answer-thinking
candidate-count-ledger
diagnostic-clean-baseline
```

Avoid:

```text
task numbers
private branch names
provider incident names
raw local script names
```

## Publication Hygiene

Before publishing or opening a PR:

```bash
CARGO_TARGET_DIR=/tmp/symbiotic-mem-bench-target cargo fmt -- --check
CARGO_TARGET_DIR=/tmp/symbiotic-mem-bench-target cargo test
cargo run --manifest-path adapters/symbiotic-memory/Cargo.toml --bin membench -- explore
```

Then check:

```bash
git status --short --ignored
rg -n "source_path|task-[0-9]+|task152|v12|legacy python" . \
  -g '!target' -g '!runs' -g '!AGENTS.md'
```

Expected ignored local paths are `runs/` and any external target directory. `.debug-session/` and
`target/` should not be present in the repo when preparing for publication.

---

# LongMemEval Push — Working Bible (READ ON EVERY CONTEXT RESET)

**GOAL:** 95% on LongMemEval-S 500Q, CLEAN pipeline, NO LongMemEval hacks (no gold-string matching,
no per-question special-casing). Detailed running ledger: [`PUSH-TO-95.md`](PUSH-TO-95.md). Durable
memory: `~/.claude/projects/-Users-k-p-symbiotic-mem-bench/memory/longmemeval-{clean-ceiling,competition}.md`.

**CURRENT STATE (forensic-corrected):** BEST CLEAN CONFIG = **rerank-ON ≈ 88.3–88.5%** + baseline 8229-char
answer prompt + fact_top_k=20/raw_turn=10 + candidates=100 + briefs ON + reasoning=high (NOT max). A 9-agent
forensic audit (verify-it-FIRED then real-or-noise) proved **the reranker is the ONLY real lever (+3.34pp,
ranges fully separate 88.20 min > 86.00 max, causal mechanism confirmed).** My earlier "89.1 = +dedup+lowconf"
was WRONG: lowconf@0.7 removes 0 facts (min confidence IS 0.70), dedup removes 0.43% then the 20-cap backfills,
so the "+0.8 stack" had NO mechanism = variance. Even con→condcon "collapsed under replication" (con-r2 already
fixed the questions condcon "fixed"). EVERYTHING except the reranker is a no-op (didn't fire), noise (fired,
moved nothing), or worse. No clean comparable 95% exists publicly. **ANSWER SIDE EXHAUSTED at ~88.5; only the
reranker moved the needle.** ALWAYS verify a lever FIRED (changed the prompt/evidence) before trusting a score delta.

## GOLDEN RULES (hard-won — break these and you burn runs on noise)
1. **VARIANCE IS ~±1.5pp (std ~0.8).** The think-on answerer is non-deterministic; the SAME config
   scores 87.2–89.6 run to run. NEVER trust a single run or an N=2 aggregate delta under ~2pp.
2. **DETECT LEVERS BY SUBSET ISOLATION, not aggregate score.** A lever changes a deterministic subset
   (e.g. split-prompts only touches count/list questions). Compare ON THAT SUBSET, with a noise-check
   on the untouched subset (must be ≈0). This is the only way to see a sub-noise effect.
3. **INSPECT EVERYTHING manually after every run** — the exact prompt sent, the reasoning chain, which
   questions flipped, the evidence. The aggregate score hides the story (Chain-of-Note "failed" at 87.8
   but GAINED on counting/temporal, only lost on preference → made it conditional).
4. **ONE LEVER AT A TIME**, behind a gated env flag (off by default). Answer-only (no re-ingest)
   whenever possible: ~$0.25 / ~10 min vs a ~$13 re-ingest.
5. **CHEAP-FIRST. NO HACKS.** Cross-check every "miss" vs the raw source — ~30% of LongMemEval misses
   are broken golds / ambiguous data / format-dings. Noise floor ≈15/500 → effective ceiling ≈97%.
6. **SETTLED — do not re-litigate:** reranker (cohere/rerank-4-fast, 100 cand) = +3.4pp, KEEP. Reader =
   deepseek-v4-flash thinking-on(=high); `max` is high-variance, `pro` is worse, other models are noise.
   Retrieval is SOLVED (51/52 misses have the gold in the candidate pool) — the bottleneck is the ANSWERER.
7. **THE WEAK CLASS is counting:** count/list = 82.4% vs 90.1% for everything else. Prompt CONTENT does
   not fix it (long/short/lean/split all +0.0 on the subset). Cleaner EVIDENCE is the live hypothesis.

## CORE LOOP — answer-only A/B
- Source vault: `runs/symbiotic-memory/long-mem-eval/500/factconsol-thinkon-500-20260624/vaults`
- Rebuild after symem code change: `CARGO_TARGET_DIR=target cargo build --release --manifest-path adapters/symbiotic-memory/Cargo.toml --bin membench`
- Run (repo root; `.env.test.local` auto-loads keys; var names post-SYMEM-removal — see docs/environment.md):
  `env MEMBENCH_DISTILL_THINKING=off MEMBENCH_ANSWER_THINKING=on MEMBENCH_EMBED_MODEL=qwen/qwen3-embedding-8b MEMBENCH_EMBED_DIMS=1024 MEMBENCH_EMBED_REQUEST_DIMS=1024 SYMBIOTIC_MEMORY__TRANSPORT__HTTP1_ONLY=true MEMBENCH_RERANK=on MEMBENCH_RERANK_MODEL=cohere/rerank-4-fast SYMBIOTIC_MEMORY__RECALL__RERANK_CANDIDATES=100 <LEVER_ENV> ./target/release/membench --system symbiotic-memory --benchmark long-mem-eval --limit 500 --sample stratified --embedder openrouter --memory-config config/symbiotic-memory/longmemeval-raw-light.yaml --prompt-dir /tmp/prompts-v3 --answer-only --source-vault-root <SRC> --score --run-name <name>`
  (rerank is ON by default; the harness default model is `nvidia/llama-nemotron-rerank-vl-1b-v2:free` — pin `MEMBENCH_RERANK_MODEL` explicitly for A/B comparability)
- Score: `is_correct` in `runs/.../<name>/artifacts/verdicts.jsonl`. Prompt+reasoning: `runs/.../<name>/vaults/<qid>/debug/facade/question-debug.json` → `recall.answer_calls[0].{system_prompt,prompt,reasoning,response_text}`. Planner: `recall.query_plan` / `recall.planner`.
- Fast single-question probe (no harness, ~10s): `/tmp/ask_one.py` replays a question's exact prompt to flash.
- Paid runs serialize on `runs/.locks/paid-provider-run.lock` (one at a time). Sweep via a background script.

## ENV KNOBS (the SYMEM_* env API is dead — engine gates are typed `[experimental]`/`[recall]` config keys, harness levers are `MEMBENCH_*`; see docs/environment.md)
- Rerank (harness-owned): `MEMBENCH_RERANK` (ON by default, `=0` disables), `MEMBENCH_RERANK_MODEL`, `MEMBENCH_RERANK_STAGE1_MODEL`(+`_TOP_X`); engine candidate width `SYMBIOTIC_MEMORY__RECALL__RERANK_CANDIDATES`.
- Engine experiment gates (off by default): `SYMBIOTIC_MEMORY__EXPERIMENTAL__MULTIHOP`, `…__TEMPORAL_FILTER`, `…__DROP_LOWCONF`(+`…__MIN_CONFIDENCE`), `…__DROP_CONFLICTING`, `…__RELEVANCE_CUTOFF`(frac 0–1), `…__SPLIT_PROMPTS`.
- Answer (harness roles): `MEMBENCH_ANSWER_THINKING`(off/on/high/max), `MEMBENCH_ANSWER_OPERATOR`/`_MODEL`. k via the config yaml (`fact_top_k`/`raw_turn_top_k`).
- Removed in the config triage (do not set; re-add only as typed `[experimental]` fields with referee evidence): `SYMEM_EXCLUDE_BRIEFS`, `SYMEM_LEDGER_RETRIEVAL`, `SYMEM_DEDUP_EVIDENCE`, `SYMEM_DETERMINISTIC_COUNT` (shelved), `SYMEM_RERANK_RESERVE`.

## LEVER LEDGER (vs ~88.5 baseline; detail in PUSH-TO-95.md)
- Within-noise-or-worse: short prompt 84, rerank-200 87.3, Chain-of-Note 87.8, surgical 87.8, condcon
  88.7, split-prompts +0.0 (count/list), smaller-k neutral, reasoning-max 90.8/86.8 (unstable), pro 87.
- PROVEN: reranker +3.4. SHELVED: deterministic-count (user: don't count in code).
- TODO (untried, lots): evidence cleaning (running), evidence-ROOT-following (top-k fact → its source
  turns/related facts), prompt STRUCTURING, hybrid KEYWORD search (sparse_terms are generated but NEVER
  used as BM25/FTS — a real gap), then STACK all non-negative levers and test N=3 vs baseline N=3.

## FILE MAP
- Answer prompt + per-task split: `../symbiotic-memory/src/recall/prompt_policy.rs`
- Recall flow, evidence filters, env gates: `../symbiotic-memory/src/recall/engine.rs`
- Query/search planner prompt: `../symbiotic-memory/src/recall/planner.rs:274` `query_planner_system_prompt`
- Evidence ledger (structured emission): `../symbiotic-memory/src/recall/evidence_ledger.rs`
- k / recall profiles: `config/symbiotic-memory/longmemeval-raw-light.yaml`
- Cleaned dataset: `runs/inputs/longmemeval-cleaned/longmemeval_s_cleaned.json` (gold = `answer`, gold sessions = `answer_session_ids`)
