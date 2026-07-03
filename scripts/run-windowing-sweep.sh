#!/usr/bin/env bash
# Windowing / reweave / recall-shape sweep (owner-directed, 2026-07-04).
#
# 4 fresh-ingest vaults crossing distill window boundary x reweave:
#   w<L>-count      count-16 windows (control)
#   w<L>-count-rw   + reweave consolidator
#   w<L>-sem        semantic window boundaries
#   w<L>-sem-rw     + reweave consolidator
# The fresh run itself answers with the DEFAULT path (collapse-to-raw).
#
# Per vault, two answer-only arms reuse the vault:
#   *-keep          facts/briefs kept in the answer context (collapse off)
#   *-rawonly       raw-only recall (fact lane off entirely)
#
# 12 scored runs total. Usage: scripts/run-windowing-sweep.sh [limit]
set -euo pipefail
cd "$(dirname "$0")/.."
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/tmp/symbiotic-mem-bench-target}"

LIMIT="${1:-50}"
ROOT="runs/symbiotic-memory/long-mem-eval/$LIMIT"
BIN=(cargo run --release --features symbiotic-memory-adapter --bin membench --
     --system symbiotic-memory --benchmark long-mem-eval --limit "$LIMIT")

echo "=== windowing sweep limit=$LIMIT ==="

# ---- vault builds (fresh ingest; the run itself is the collapse-default arm)
"${BIN[@]}" --run-name "w${LIMIT}-count"
env SYMEM_CONSOLIDATOR=llm \
  "${BIN[@]}" --run-name "w${LIMIT}-count-rw"
env SYMBIOTIC_MEMORY__DISTILL__WINDOW_BOUNDARY=semantic \
  "${BIN[@]}" --run-name "w${LIMIT}-sem"
env SYMBIOTIC_MEMORY__DISTILL__WINDOW_BOUNDARY=semantic SYMEM_CONSOLIDATOR=llm \
  "${BIN[@]}" --run-name "w${LIMIT}-sem-rw"

# ---- answer-only arms over each vault
RUNS=("w${LIMIT}-count" "w${LIMIT}-count-rw" "w${LIMIT}-sem" "w${LIMIT}-sem-rw")
for vault in "${RUNS[@]}"; do
  env SYMBIOTIC_MEMORY__EXPERIMENTAL__RERANK_COLLAPSE=false \
    "${BIN[@]}" --run-name "${vault}-keep" --answer-only \
    --source-vault-root "$ROOT/$vault/vaults"
  "${BIN[@]}" --run-name "${vault}-rawonly" --answer-only \
    --source-vault-root "$ROOT/$vault/vaults" \
    --memory-config config/symbiotic-memory/longmemeval-raw-only.yaml
done

echo "=== scoring ==="
ALL=()
for vault in "${RUNS[@]}"; do
  ALL+=("$vault" "${vault}-keep" "${vault}-rawonly")
done
scripts/score-run.sh "${ALL[@]}"
