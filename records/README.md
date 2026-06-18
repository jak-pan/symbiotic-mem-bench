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

Native records may also include `raw/`, `vaults/`, `workflow/`, and `provider-queue/`. Imported
records can be artifact-only; their `artifact_manifest` must make missing native state explicit.

Use `--force` only when intentionally replacing a tracked record with a corrected version.
