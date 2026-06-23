#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Run a dashboard-safe ingest-stage tuning arm.

The script uses an existing release membench binary. It does not build code.
Provider/model traces are enabled by default; raw provider request payloads are
off by default because those can contain source text.

Usage:
  scripts/run-ingest-stage-tuning.sh [mode] [profile] [-- extra membench args]

Modes:
  raw-embed          Raw-turn embedding only.
  distill            Distill windows only; skips raw embed and stops after distill.
  raw-embed-distill  Run raw embedding and distill together, then stop.

Profiles:
  deepseek-flash-qwen3-8b-1024
      Current tuning baseline: native/default flash distill model plus
      OpenRouter qwen/qwen3-embedding-8b, 1024 dims, HTTP/1 32x32.

Useful env:
  MEMBENCH_BIN=./target/release/membench
  LIMIT=10
  SAMPLE=stratified
  RUN_NAME=<explicit run name>
  DEBUG_REQUESTS=1            # stores raw provider request payloads; local forensics only
  SYMEM_DISTILL_WINDOW_MAX_INPUT_TOKENS=<tokens>
  SYMEM_PROVIDER_TRACE=1      # default

Examples:
  scripts/run-ingest-stage-tuning.sh distill
  scripts/run-ingest-stage-tuning.sh raw-embed-distill deepseek-flash-qwen3-8b-1024
USAGE
}

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  usage
  exit 0
fi

mode="${1:-distill}"
if [[ $# -gt 0 ]]; then
  shift
fi

profile="${1:-deepseek-flash-qwen3-8b-1024}"
if [[ $# -gt 0 ]]; then
  shift
fi

if [[ "${1:-}" == "--" ]]; then
  shift
fi

case "$mode" in
  raw-embed|distill|raw-embed-distill) ;;
  *)
    echo "unknown mode: $mode" >&2
    usage >&2
    exit 2
    ;;
esac

case "$profile" in
  deepseek-flash-qwen3-8b-1024)
    cohort="ingest-stage/deepseek-flash-qwen3-8b-1024"
    embedder="${SYMEM_EMBEDDER:-openrouter}"
    store="${SYMEM_STORE:-zvec-hybrid}"
    operator="${SYMEM_EMBED_OPERATOR:-openrouter}"
    model="${SYMEM_EMBED_MODEL:-qwen/qwen3-embedding-8b}"
    dims="${SYMEM_EMBED_DIMS:-1024}"
    request_dims="${SYMEM_EMBED_REQUEST_DIMS:-$dims}"
    batch_size="${SYMEM_EMBED_BATCH_SIZE:-250}"
    batch_max_chars="${SYMEM_EMBED_BATCH_MAX_CHARS:-32000}"
    max_chars="${SYMEM_EMBED_MAX_CHARS:-32000}"
    http1="${SYMEM_OPENROUTER_HTTP_HTTP1_ONLY:-1}"
    pool="${SYMEM_OPENROUTER_HTTP_CLIENT_POOL_SIZE:-32}"
    idle="${SYMEM_OPENROUTER_HTTP_POOL_MAX_IDLE_PER_HOST:-32}"
    ;;
  *)
    echo "unknown profile: $profile" >&2
    usage >&2
    exit 2
    ;;
esac

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

membench_bin="${MEMBENCH_BIN:-./target/release/membench}"
if [[ ! -x "$membench_bin" ]]; then
  cat >&2 <<EOF
Missing executable release binary: $membench_bin

Build it deliberately before paid tuning runs:
  cargo build --release --features symbiotic-memory-adapter --bin membench

Or set MEMBENCH_BIN=/path/to/membench.
EOF
  exit 1
fi

limit="${LIMIT:-10}"
sample="${SAMPLE:-stratified}"
workflow_max_in_flight="${SYMEM_WORKFLOW_MAX_IN_FLIGHT:-10}"
safe_profile="$(echo "$profile" | tr '/:' '--')"
run_name="${RUN_NAME:-tune-ingest-${mode}-${safe_profile}-${limit}q-$(date -u +%Y%m%d-%H%M%S)}"
debug_requests="${DEBUG_REQUESTS:-${SYMEM_PROVIDER_QUEUE_DEBUG_REQUESTS:-0}}"

echo "cohort=$cohort"
echo "run_name=$run_name"
echo "mode=$mode profile=$profile"
echo "embedding=$operator:$model dims=$dims request_dims=$request_dims batch_max_chars=$batch_max_chars"
echo "openrouter_http http1=$http1 pool=$pool idle=$idle"
if [[ "$debug_requests" == "1" || "$debug_requests" == "true" ]]; then
  echo "debug_requests=on (raw provider payloads will be stored)"
else
  echo "debug_requests=off (dashboard-safe traces only)"
fi

SYMEM_PROVIDER_TRACE="${SYMEM_PROVIDER_TRACE:-1}" \
SYMEM_PROVIDER_QUEUE_DEBUG_REQUESTS="$debug_requests" \
SYMEM_EMBED_OPERATOR="$operator" \
SYMEM_EMBED_MODEL="$model" \
SYMEM_EMBED_DIMS="$dims" \
SYMEM_EMBED_REQUEST_DIMS="$request_dims" \
SYMEM_EMBED_BATCH_SIZE="$batch_size" \
SYMEM_EMBED_BATCH_MAX_CHARS="$batch_max_chars" \
SYMEM_EMBED_MAX_CHARS="$max_chars" \
SYMEM_WORKFLOW_MAX_IN_FLIGHT="$workflow_max_in_flight" \
SYMEM_OPENROUTER_HTTP_CLIENT_POOL_SIZE="$pool" \
SYMEM_OPENROUTER_HTTP_POOL_MAX_IDLE_PER_HOST="$idle" \
SYMEM_OPENROUTER_HTTP_HTTP1_ONLY="$http1" \
"$membench_bin" \
  --system symbiotic-memory \
  --benchmark long-mem-eval \
  --limit "$limit" \
  --sample "$sample" \
  --embedder "$embedder" \
  --store "$store" \
  --fresh \
  --ingest-diagnostic "$mode" \
  --no-answerer \
  --no-consolidate-briefs \
  --no-score \
  --run-name "$run_name" \
  "$@"
