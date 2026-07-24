#!/usr/bin/env bash
# Regenerate the static leaderboard snapshot bundled with the dashboard.
#
# Writes the `membench.leaderboard.v1` export of the tracked records tree to
# dashboard/public/data/leaderboard.json, with real provenance (generation
# time + exporter git sha). Vite copies public/ into dist/, and the SPA falls
# back to this document when no /api backend is present (static deploys).
#
# Run after records/ changes, review the diff, and commit the result. The
# deterministic mode is reserved for the CI contract canary (see canary/).
set -euo pipefail
cd "$(dirname "$0")/.."

GIT_SHA="$(git rev-parse --short HEAD)" \
  cargo run --bin membench-leaderboard -- export \
  --records-root records \
  --out dashboard/public/data/leaderboard.json
