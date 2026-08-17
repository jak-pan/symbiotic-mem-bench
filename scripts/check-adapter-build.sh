#!/usr/bin/env bash
# Release gate: the documented product CLI actually builds against the exact
# pins in Cargo.toml/Cargo.lock.
#
# The README's isolated-adapter `cargo run ... --bin membench -- explore` command
# is covered here. The private adapter is isolated from the public root manifest at
# adapters/symbiotic-memory/Cargo.toml. It builds against
# `symbiotic-sh/symbiotic-memory`, so this check needs credentials and cannot
# run on an anonymous CI runner (see docs/oss-release-handoff.md). It must pass
# on a credentialed machine before any release is tagged.
#
# `--locked` is deliberate: it fails rather than silently refreshing the
# lockfile, so the build proves the pinned revs, not whatever resolves today.
set -euo pipefail
cd "$(dirname "$0")/.."

manifest=adapters/symbiotic-memory/Cargo.toml
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-/tmp/symbiotic-mem-bench-adapter-target}

echo "== cargo fmt private adapter"
cargo fmt --manifest-path "$manifest" --all -- --check

echo "== cargo check private adapter --locked"
cargo check --manifest-path "$manifest" --locked --all-targets

# Correctness and suspicious lints are denied; the adapter's older style debt
# (long argument lists, collapsible ifs) is not, so this gate fails on bugs
# rather than on formatting opinions. Tightening to `-D warnings` needs a
# separate cleanup pass over src/symbiotic_memory_adapter.rs and src/bin/membench.rs.
echo "== cargo clippy private adapter (correctness)"
cargo clippy --manifest-path "$manifest" --locked --all-targets -- \
  -D clippy::correctness -D clippy::suspicious

echo "== LongMemEval-V2 text projection contract"
cargo test --manifest-path "$manifest" --locked --test benchmark_v2

echo "== the documented quickstart command"
cargo run --manifest-path "$manifest" --locked --quiet --bin membench -- explore --help > /dev/null

echo "OK: the adapter CLI builds and runs against the pinned revisions"
