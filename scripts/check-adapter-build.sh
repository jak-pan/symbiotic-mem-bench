#!/usr/bin/env bash
# Release gate: the documented product CLI actually builds against the exact
# pins in Cargo.toml/Cargo.lock.
#
# `cargo run --bin membench -- explore` is the first command in the README, and
# `membench` requires the `symbiotic-memory-adapter` feature. That feature
# builds against `symbiotic-sh/symbiotic-memory`, which is currently a private
# repository, so this check needs credentials and cannot run on an anonymous
# CI runner (see docs/oss-release-handoff.md). It must pass on a credentialed
# machine before any release is tagged.
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

echo "== the documented quickstart command"
cargo run --locked --quiet --features symbiotic-memory-adapter --bin membench -- explore --help > /dev/null

echo "OK: the adapter CLI builds and runs against the pinned revisions"
