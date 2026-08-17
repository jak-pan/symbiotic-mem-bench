# Benchmark Records

Tracked benchmark records live here.

See `../docs/run-registry.md` for the full registry contract.

Local and active runs default to:

```text
runs/{system}/{benchmark}/{limit}/{run_name}/
```

That local registry is under the `symbiotic-mem-bench` repo root and is ignored by git. Promote a
run into tracked records with:

```bash
cargo run --bin membench -- save-record \
  --run-root runs/symbiotic-memory/long-mem-eval/500/<run-name>
```

The saved record path is:

```text
records/{system}/{benchmark}/{limit}/{run_name}/
```

Each saved record should contain the normalized run files:

- `run-params.json`
- `benchmark-report.json`
- `artifacts/`

Public tracked records must be portable: normalized metadata plus `artifacts/` only. Do not commit
`raw/`, `vaults/`, `workflow/`, `provider-queue/`, SQLite state, symlinks, or local debug bundles.
If native state is intentionally retained, keep it in an access-controlled external archive and
describe it through the external-artifact contract; do not put it in this source repository or a
public product bundle. Imported records can be artifact-only; their `artifact_manifest` must make
missing native state explicit.

Use `--force` only when intentionally replacing a tracked record with a corrected version.

For dashboard-safe timing evidence, prefer meta records:

```bash
scripts/save-run-meta-record.sh runs/{system}/{benchmark}/{limit}/{run_name}
```

Meta records retain normalized trace/timing artifacts, but omit queue databases, vaults, raw
outputs, raw provider request payloads, and question-level artifacts. They are intended to populate
the dashboard without carrying source data.
