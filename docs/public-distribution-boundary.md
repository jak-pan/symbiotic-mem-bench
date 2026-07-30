# Public distribution boundary

Membench is publishable as an OSS benchmark product without publishing a memory
implementation. The public contract is the portable run bundle and leaderboard
schema, not a vendored copy of the system under test.

## What an anonymous clone can do

The default feature set has no private source requirement. It builds the
`membench` CLI and supports:

- importing normalized hypotheses, scores, verdicts, provenance, and trace files;
- exploring scratch runs and tracked records;
- promoting portable records after the repository's review and hygiene gates;
- deriving Trials and analytics from existing run artifacts;
- exporting and serving the leaderboard and dashboard.

The public build must stay green with:

```bash
CARGO_TARGET_DIR=/tmp/symbiotic-mem-bench-target \
  cargo test --locked --bin membench
CARGO_TARGET_DIR=/tmp/symbiotic-mem-bench-target \
  cargo run --locked --bin membench -- explore --json
```

A system adapter may therefore run in another repository or service and hand
Membench a portable artifact bundle. For example:

```bash
cargo run --locked --bin membench -- \
  --system external-memory \
  --benchmark long-mem-eval \
  --import-report \
  --hypotheses exported/hypotheses.jsonl \
  --scored exported/scored.json \
  --verdicts exported/verdicts.jsonl \
  --provenance exported/provenance.json \
  --memory-traces exported/memory-traces.jsonl \
  --model-traces exported/model-traces.jsonl \
  --run-name external-memory-baseline
```

Missing optional artifacts remain explicitly missing in `artifact_manifest`;
the importer does not invent traces or claim native state.

## What remains access-gated

`--features symbiotic-memory-adapter` links the in-process Symbiotic Memory
adapter to the exact private `symbiotic-sh/symbiotic-memory` revision. That
feature is not part of the anonymous OSS build and must never be represented as
public source while the upstream repository is private.

The boundary is fail-closed:

- a no-feature native run exits with an explicit adapter-required error;
- the private feature uses exact manifest, lockfile, and source pins;
- credentialed CI builds the private adapter only from a reviewed read-only
  deploy key;
- no private checkout, native package, Cargo target, credential, or raw provider
  payload is included in public release artifacts.

If Symbiotic Memory itself later becomes public, the feature can become an
ordinary public adapter without changing the portable record or leaderboard
contracts. If it remains private, the OSS release is still complete as a
neutral harness and leaderboard, while Symbiotic's in-process executor remains
an owner-only integration.

