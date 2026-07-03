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
# METHODOLOGY (owner, 2026-07-04): distill is a nondeterministic LLM phase —
# never re-roll it unless the treatment changes distill inputs. Build reweave
# arms from the SAME base vault via the redo machinery so the fact base is
# byte-identical and the measured delta is the reweave pass alone (variance
# control first, cost second):
#   MEMBENCH_CONSOLIDATOR=llm MEMBENCH_REDO=reweave <run> --source-vault-root .../w50-count/vaults
# Fresh re-ingest is only correct for arms that change ingest itself
# (e.g. count vs semantic windowing).
"${BIN[@]}" --run-name "w${LIMIT}-count"
env MEMBENCH_CONSOLIDATOR=llm \
  "${BIN[@]}" --run-name "w${LIMIT}-count-rw"
env SYMBIOTIC_MEMORY__DISTILL__WINDOW_BOUNDARY=semantic \
  "${BIN[@]}" --run-name "w${LIMIT}-sem"
env SYMBIOTIC_MEMORY__DISTILL__WINDOW_BOUNDARY=semantic MEMBENCH_CONSOLIDATOR=llm \
  "${BIN[@]}" --run-name "w${LIMIT}-sem-rw"

# ---- answer-only arms over each vault
# Non-reweave vaults have no `consolidate` manifest stage; answer-only runs
# must pass --no-consolidate-briefs there or the vault-completeness check
# (post_ingest_complete) rejects every question.
RUNS=("w${LIMIT}-count" "w${LIMIT}-count-rw" "w${LIMIT}-sem" "w${LIMIT}-sem-rw")
for vault in "${RUNS[@]}"; do
  briefs_flag=()
  case "$vault" in
    *-rw) ;;
    *) briefs_flag=(--no-consolidate-briefs) ;;
  esac
  env SYMBIOTIC_MEMORY__EXPERIMENTAL__RERANK_COLLAPSE=false \
    "${BIN[@]}" --run-name "${vault}-keep" --answer-only ${briefs_flag[@]+"${briefs_flag[@]}"} \
    --source-vault-root "$ROOT/$vault/vaults"
  "${BIN[@]}" --run-name "${vault}-rawonly" --answer-only ${briefs_flag[@]+"${briefs_flag[@]}"} \
    --source-vault-root "$ROOT/$vault/vaults" \
    --memory-config config/symbiotic-memory/longmemeval-raw-only.yaml
done

echo "=== scoring ==="
ALL=()
for vault in "${RUNS[@]}"; do
  ALL+=("$vault" "${vault}-keep" "${vault}-rawonly")
done
scripts/score-run.sh "${ALL[@]}"
