# Run Registry

The run registry is the shared storage contract for local benchmark runs and tracked benchmark
records.

See `docs/schemas.md` for the JSON field reference.

## Paths

Local scratch registry:

```text
runs/{system}/{benchmark}/{limit}/{run_name}/
```

Tracked curated records:

```text
records/{system}/{benchmark}/{limit}/{run_name}/
```

Examples:

```text
runs/symbiotic-memory/long-mem-eval/50/candidate-rerank-orderfix/
runs/symbiotic-memory/long-mem-eval/500/20260617-153012-a1b2c3d4/
records/symbiotic-memory/long-mem-eval/500/baseline-clean/
```

`limit` is the intended benchmark size for native runs. For imported scored artifacts, it is derived
from the scored artifact's `counts.scored` field.

Local no-network `--smoke` adapter tests are not registry records by default. They execute under
`runs/.tmp/` and delete themselves after a successful validation. Use an explicit `--run-root` only
when intentionally preserving a smoke run for forensics.

## Required Files

Each native complete run folder contains:

```text
run-params.json
benchmark-report.json
artifacts/
raw/
vaults/
workflow/
provider-queue/
```

`run-params.json` records how the run was created or imported. It must use portable paths when
paths are stored.

`benchmark-report.json` records normalized score metrics and artifact summaries. It is the primary
file read by `membench explore`.

`artifact_manifest` records which common artifacts are available, which are missing, and whether
native state folders were captured. Native runs write it to `benchmark-report.json` after execution;
imported runs also copy it into `run-params.json` so artifact-only comparisons are self-describing.

`artifacts/` contains copied benchmark artifacts. The run must be inspectable even if the original
source files are deleted.

`raw/` contains executor-native outputs written in-place while a native run executes. These are
retained for forensics but are not the public artifact contract. It can be absent on imported runs.

`vaults/`, `workflow/`, and `provider-queue/` are durable state folders produced by native systems.
They may be absent for imported runs or external adapters that cannot expose internal state.

## Common Artifacts

```text
artifacts/hypotheses.jsonl
artifacts/scored.json
artifacts/verdicts.jsonl
artifacts/partial-verdicts.jsonl
artifacts/provenance.jsonl
artifacts/memory-traces.jsonl
artifacts/model-traces.jsonl
artifacts/score-summary.json
```

Not every adapter can produce every artifact. Missing artifacts should be visible in reports and
explorers; do not fake internal traces.

## Native Scratch Layout

A finalized native Symbiotic Memory run usually looks like:

```text
run-params.json
benchmark-report.json
artifacts/
  hypotheses.jsonl
  verdicts.jsonl
  partial-verdicts.jsonl
  scored.json
  score-summary.json
  memory-traces.jsonl
  model-traces.jsonl
raw/
  hypotheses.jsonl
  verdicts.jsonl
  partial-verdicts.jsonl
  scored.json
  score-summary.json
  memory-traces.jsonl
  model-traces.jsonl
  scores/
vaults/
workflow/
provider-queue/
```

`artifacts/` and `raw/` intentionally overlap in content for native runs. `raw/` is the live write
location for adapter-owned files; `artifacts/` is the normalized portable copy created for reports,
records, and publication.

## Native Run Lifecycle

1. `membench` selects a default run root under `runs/{system}/{benchmark}/{limit}/{timestamp-id}`.
2. For a normal native run, `membench` requests a fresh run. The adapter resets the run root and
   re-ingests every selected question from source.
3. If scoring is enabled, the scorer writes verdicts and scored output under `raw/`.
4. `membench` writes `run-params.json`, `benchmark-report.json`, and normalized artifact copies.
5. The run remains local and ignored by git unless explicitly promoted.

Reuse modes are explicit:

```text
--resume      continue an interrupted run root
--answer-only reuse an existing ingested run root and write new answers
```

## Async Native Runs

Native adapters should expose the system under test as it really runs. The registry format supports
incremental state and should not force adapters into a batch barrier:

- `raw/`, `vaults/`, `workflow/`, and `provider-queue/` may be written while the run is in progress;
- stage artifacts and traces should append as each item succeeds or fails;
- durable workflow state should distinguish queued, running, retryable failure, terminal failure,
  and succeeded items;
- completed stage outputs should remain usable after interruption;
- `--resume` continues incomplete durable work instead of starting a new run;
- `--answer-only` requires complete ingest/index state and writes new answer artifacts without
  mutating source ingest state.

When an adapter cannot expose native state, keep the state folders absent and mark the missing
artifacts in `artifact_manifest`.

## Imported Run Lifecycle

1. User passes `--import-report`, `--hypotheses`, and `--scored`.
2. Optional artifacts can include verdicts, partial verdicts, provenance, memory traces, and model
   traces.
3. `membench` derives metrics and `limit` from `scored.json`.
4. `membench` copies artifacts into `runs/{system}/{benchmark}/{limit}/{run_name}/artifacts/`.
5. `run-params.json` records which artifact classes were imported, not the original local source
   paths.
6. `artifact_manifest.native_state_available` is `false` for imported runs unless a future importer
   explicitly captures native state.

## Promotion

Promote a run with:

```bash
cargo run --bin membench -- save-record \
  --run-root runs/{system}/{benchmark}/{limit}/{run_name}
```

The destination is:

```text
records/{system}/{benchmark}/{limit}/{run_name}/
```

Use `--record-name` only when giving the tracked record a clearer public name. Use `--force` only
when intentionally replacing an existing record.

## Explorer Behavior

`membench explore` recursively scans a registry root for `benchmark-report.json`.

Default:

```bash
cargo run --bin membench -- explore
```

Specific run:

```bash
cargo run --bin membench -- explore \
  --run-root runs/symbiotic-memory/long-mem-eval/500/baseline-clean
```

Future explorer views should use the same files and should not invent another run format.

## Portability Rules

- Reports should use repo-relative paths for files inside this repository.
- Reports should not preserve original absolute import paths.
- Tracked records should not contain provider secrets, API keys, raw local `.env` data, or raw prompts
  unless a future explicit local-only debug mode marks them as such.
- `runs/` is ignored by git. `records/` is tracked by git.
- `.debug-session/` is not part of this repo's public workflow.
