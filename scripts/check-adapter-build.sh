#!/usr/bin/env bash
# Release gate: the optional in-process Symbiotic Memory adapter actually
# builds against the exact pins in Cargo.toml/Cargo.lock.
#
# The public no-feature `membench` CLI builds anonymously for portable import,
# exploration, promotion, analytics, and Trials. Native Symbiotic Memory
# execution is the credentialed surface: `--features
# symbiotic-memory-adapter` links `symbiotic-sh/symbiotic-memory`, which is
# currently private. This adapter gate therefore needs credentials and must
# pass on a trusted machine before a release claims that integration.
#
# `--locked` is deliberate: it fails rather than silently refreshing the
# lockfile, so the build proves the pinned revs, not whatever resolves today.
set -euo pipefail
cd "$(dirname "$0")/.."

echo "== cargo check --features symbiotic-memory-adapter --locked"
cargo check --locked --features symbiotic-memory-adapter --all-targets

# Correctness and suspicious lints are denied; the adapter's older style debt
# (long argument lists, collapsible ifs) is not, so this gate fails on bugs
# rather than on formatting opinions. Tightening to `-D warnings` needs a
# separate cleanup pass over src/symbiotic_memory_adapter.rs and src/bin/membench.rs.
echo "== cargo clippy --features symbiotic-memory-adapter (correctness)"
cargo clippy --locked --features symbiotic-memory-adapter --all-targets -- \
  -D clippy::correctness -D clippy::suspicious

echo "== LongMemEval-V2 text projection contract"
cargo test --locked --features symbiotic-memory-adapter --test benchmark_v2

echo "== the adapter-enabled CLI"
cargo run --locked --quiet --features symbiotic-memory-adapter --bin membench -- explore --help > /dev/null

echo "OK: the adapter CLI builds and runs against the pinned revisions"
