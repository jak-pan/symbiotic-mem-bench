#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Run a dashboard-safe chat-provider transport tuning arm.

The script uses an existing release membench binary. It does not build code.
It isolates the Distillery chat path with --ingest-diagnostic distill, a hash
embedder, no answerer, no briefs, and no scoring. Raw provider request payloads
are off by default because those can contain source text.

Usage:
  scripts/run-chat-transport-tuning.sh [profile] [shape] [-- extra membench args]

Profiles:
  deepseek-v4-flash-distill
      DeepSeek deepseek-v4-flash Distillery-only transport tuning.

Shapes:
  h1-32x32   HTTP/1, 32 clients, 32 idle connections per host
  h1-64x32   HTTP/1, 64 clients, 32 idle connections per host
  h1-128x16  HTTP/1, 128 clients, 16 idle connections per host
  h1-64x16   HTTP/1, 64 clients, 16 idle connections per host
  h1-16x64   HTTP/1, 16 clients, 64 idle connections per host
  h1-4x64    HTTP/1,  4 clients, 64 idle connections per host
  h2-32x32   HTTP/2-capable, 32 clients, 32 idle connections per host
  h2-64x32   HTTP/2-capable, 64 clients, 32 idle connections per host
  h2-128x16  HTTP/2-capable, 128 clients, 16 idle connections per host
  h2-64x16   HTTP/2-capable, 64 clients, 16 idle connections per host
  h2-16x64   HTTP/2-capable, 16 clients, 64 idle connections per host
  h2-8x64    HTTP/2-capable,  8 clients, 64 idle connections per host
  h2-4x64    HTTP/2-capable,  4 clients, 64 idle connections per host
  h2-1x64    HTTP/2-capable,  1 client,  64 idle connections per host

Useful env:
  MEMBENCH_BIN=./target/release/membench
  LIMIT=10
  SAMPLE=stratified
  RUN_NAME=<explicit run name>
  DEBUG_REQUESTS=1            # stores raw provider payloads; local forensics only
  MEMBENCH_WORKFLOW_MAX_IN_FLIGHT=10
  SYMBIOTIC_MEMORY__DISTILL__WINDOW_MAX_INPUT_TOKENS=<tokens>
  SYMBIOTIC_MEMORY__TRANSPORT__PROVIDER_TRACE=true   # default

Example:
  scripts/run-chat-transport-tuning.sh deepseek-v4-flash-distill h2-64x32
USAGE
}

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  usage
  exit 0
fi

profile="${1:-deepseek-v4-flash-distill}"
if [[ $# -gt 0 ]]; then
  shift
fi

shape="${1:-h2-64x32}"
if [[ $# -gt 0 ]]; then
  shift
fi

if [[ "${1:-}" == "--" ]]; then
  shift
fi

case "$profile" in
  deepseek-v4-flash-distill)
    cohort="chat-transport/deepseek-v4-flash-distill"
    operator="${MEMBENCH_DISTILL_OPERATOR:-deepseek}"
    model="${MEMBENCH_DISTILL_MODEL:-deepseek-v4-flash}"
    embedder="${MEMBENCH_EMBEDDER:-hash}"
    ;;
  *)
    echo "unknown profile: $profile" >&2
    usage >&2
    exit 2
    ;;
esac

case "$shape" in
  h1-32x32) http1=1; pool=32; idle=32 ;;
  h1-64x32) http1=1; pool=64; idle=32 ;;
  h1-128x16) http1=1; pool=128; idle=16 ;;
  h1-64x16) http1=1; pool=64; idle=16 ;;
  h1-16x64) http1=1; pool=16; idle=64 ;;
  h1-4x64)  http1=1; pool=4;  idle=64 ;;
  h2-32x32) http1=0; pool=32; idle=32 ;;
  h2-64x32) http1=0; pool=64; idle=32 ;;
  h2-128x16) http1=0; pool=128; idle=16 ;;
  h2-64x16) http1=0; pool=64; idle=16 ;;
  h2-16x64) http1=0; pool=16; idle=64 ;;
  h2-8x64)  http1=0; pool=8;  idle=64 ;;
  h2-4x64)  http1=0; pool=4;  idle=64 ;;
  h2-1x64)  http1=0; pool=1;  idle=64 ;;
  *)
    echo "unknown shape: $shape" >&2
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
  CARGO_TARGET_DIR=target cargo build --release --manifest-path adapters/symbiotic-memory/Cargo.toml --bin membench

Or set MEMBENCH_BIN=/path/to/membench.
EOF
  exit 1
fi

limit="${LIMIT:-10}"
sample="${SAMPLE:-stratified}"
workflow_max_in_flight="${MEMBENCH_WORKFLOW_MAX_IN_FLIGHT:-10}"
safe_profile="$(echo "$profile" | tr '/:' '--')"
run_name="${RUN_NAME:-tune-chat-${safe_profile}-${limit}q-${shape}-$(date -u +%Y%m%d-%H%M%S)}"
debug_requests="${DEBUG_REQUESTS:-${SYMBIOTIC_MEMORY__QUEUE__DEBUG_REQUESTS:-0}}"

# The kit config env layer parses booleans strictly (true/false).
to_bool() { case "$1" in 1|true|TRUE|on|yes) echo true ;; *) echo false ;; esac; }
http1_bool="$(to_bool "$http1")"
debug_requests_bool="$(to_bool "$debug_requests")"
provider_trace_bool="$(to_bool "${SYMBIOTIC_MEMORY__TRANSPORT__PROVIDER_TRACE:-1}")"

echo "cohort=$cohort"
echo "run_name=$run_name"
echo "profile=$profile shape=$shape http1=$http1 pool=$pool idle=$idle"
echo "distill=$operator:$model embedder=$embedder store=$store"
if [[ "$debug_requests" == "1" || "$debug_requests" == "true" ]]; then
  echo "debug_requests=on (raw provider payloads will be stored)"
else
  echo "debug_requests=off (dashboard-safe traces only)"
fi

SYMBIOTIC_MEMORY__TRANSPORT__PROVIDER_TRACE="$provider_trace_bool" \
SYMBIOTIC_MEMORY__QUEUE__DEBUG_REQUESTS="$debug_requests_bool" \
MEMBENCH_DISTILL_OPERATOR="$operator" \
MEMBENCH_DISTILL_MODEL="$model" \
MEMBENCH_WORKFLOW_MAX_IN_FLIGHT="$workflow_max_in_flight" \
SYMBIOTIC_MEMORY__TRANSPORT__CHAT_CLIENT_POOL_SIZE="$pool" \
SYMBIOTIC_MEMORY__TRANSPORT__POOL_MAX_IDLE_PER_HOST="$idle" \
SYMBIOTIC_MEMORY__TRANSPORT__HTTP1_ONLY="$http1_bool" \
"$membench_bin" \
  --system symbiotic-memory \
  --benchmark long-mem-eval \
  --limit "$limit" \
  --sample "$sample" \
  --distiller llm \
  --embedder "$embedder" \
  --fresh \
  --ingest-diagnostic distill \
  --no-answerer \
  --no-consolidate-briefs \
  --no-score \
  --run-name "$run_name" \
  "$@"
