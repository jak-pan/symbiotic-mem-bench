#!/usr/bin/env bash
# Answer-only phase of the windowing sweep — reuses the 4 ingested vaults.
set -euo pipefail
cd "$(dirname "$0")/.."
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/tmp/symbiotic-mem-bench-target}"

LIMIT="${1:-50}"
ROOT="runs/symbiotic-memory/long-mem-eval/$LIMIT"
BIN=(cargo run --release --manifest-path adapters/symbiotic-memory/Cargo.toml --bin membench --
     --system symbiotic-memory --benchmark long-mem-eval --limit "$LIMIT")

arm_done() { [ -f "$ROOT/$1/artifacts/verdicts.jsonl" ]; }

RUNS=("w${LIMIT}-count" "w${LIMIT}-count-rw" "w${LIMIT}-sem" "w${LIMIT}-sem-rw")
for vault in "${RUNS[@]}"; do
  briefs_flag=()
  case "$vault" in
    *-rw) ;;
    *) briefs_flag=(--no-consolidate-briefs) ;;
  esac
  if arm_done "${vault}-keep"; then
    echo "skip ${vault}-keep (already scored)"
  else
    env SYMBIOTIC_MEMORY__EXPERIMENTAL__RERANK_COLLAPSE=false \
      "${BIN[@]}" --run-name "${vault}-keep" --answer-only ${briefs_flag[@]+"${briefs_flag[@]}"} \
      --source-vault-root "$ROOT/$vault/vaults"
  fi
  if arm_done "${vault}-rawonly"; then
    echo "skip ${vault}-rawonly (already scored)"
  else
    "${BIN[@]}" --run-name "${vault}-rawonly" --answer-only ${briefs_flag[@]+"${briefs_flag[@]}"} \
      --source-vault-root "$ROOT/$vault/vaults" \
      --memory-config config/symbiotic-memory/longmemeval-raw-only.yaml
  fi
done

echo "=== scoring ==="
ALL=()
for vault in "${RUNS[@]}"; do
  ALL+=("$vault" "${vault}-keep" "${vault}-rawonly")
done
scripts/score-run.sh "${ALL[@]}"
