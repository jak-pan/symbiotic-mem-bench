#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Summarize chat-provider transport tuning runs.

Usage:
  scripts/report-chat-transport-tuning.sh [--markdown] [--profile PROFILE] [run-root ...]

Profiles:
  deepseek-v4-flash-distill

If no run roots are supplied, the script discovers local tune-chat runs for the
selected profile. It reads provider-queue/model-queue-traces.jsonl and
benchmark-report.json. It does not call providers and does not mutate runs.
USAGE
}

mode="tsv"
profile="deepseek-v4-flash-distill"

while [[ $# -gt 0 ]]; do
  case "$1" in
    -h|--help)
      usage
      exit 0
      ;;
    --markdown)
      mode="markdown"
      shift
      ;;
    --profile)
      profile="${2:-}"
      if [[ -z "$profile" ]]; then
        echo "--profile requires a value" >&2
        exit 2
      fi
      shift 2
      ;;
    --)
      shift
      break
      ;;
    *)
      break
      ;;
  esac
done

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

runs=("$@")
if [[ ${#runs[@]} -eq 0 ]]; then
  case "$profile" in
    deepseek-v4-flash-distill)
      mapfile -t runs < <(find runs/symbiotic-memory/long-mem-eval -type d -name 'tune-chat-deepseek-v4-flash-distill-*' | sort)
      ;;
    *)
      echo "unknown profile: $profile" >&2
      usage >&2
      exit 2
      ;;
  esac
fi

if [[ ${#runs[@]} -eq 0 ]]; then
  echo "no runs found for profile: $profile" >&2
  exit 1
fi

tmp="$(mktemp)"
trap 'rm -f "$tmp"' EXIT

for run in "${runs[@]}"; do
  traces="$run/provider-queue/model-queue-traces.jsonl"
  report="$run/benchmark-report.json"
  if [[ ! -f "$traces" ]]; then
    echo "warning: missing traces: $traces" >&2
    continue
  fi

  params="pool=? idle=? http1=?"
  run_name="$(basename "$run")"
  trace_sha="$(shasum -a 256 "$traces" | awk '{print $1}')"
  if [[ -f "$report" ]]; then
    params="$(jq -r '
      def shown($key): if has($key) then .[$key] else "default" end;
      .run_params
      | "pool="+((shown("chat_http_client_pool_size"))|tostring)
        +" idle="+((shown("chat_http_pool_max_idle_per_host"))|tostring)
        +" http1="+((shown("chat_http1_only"))|tostring)' "$report")"
  fi

  jq -s --arg run "$run_name" --arg path "$run" --arg params "$params" --arg trace_sha "$trace_sha" --arg profile "$profile" '
    def pct($a; $p):
      ($a|length) as $n
      | if $n == 0 then null else $a[((($n - 1) * $p) | floor)] end;
    def seconds($ms): if $ms == null then null else (($ms / 1000) * 1000 | round / 1000) end;

    . as $events
    | [$events[] | select(type=="object" and .operation=="chat" and .status=="succeeded") | .run_ms] | sort as $runs
    | [$events[] | select(type=="object" and .operation=="chat" and .status=="failed")] as $fails
    | [$events[] | select(type=="object" and .operation=="chat" and .status=="running") | (.queue_wait_ms // 0)] | sort as $waits
    | [$events[] | select(type=="object" and .operation=="chat" and .status=="running") | (.throttle_wait_ms // 0)] | sort as $throttles
    | {
        profile: $profile,
        run: $run,
        path: $path,
        params: $params,
        n: ($runs | length),
        failed_attempts: ($fails | length),
        p50s: seconds(pct($runs; 0.50)),
        p80s: seconds(pct($runs; 0.80)),
        p95s: seconds(pct($runs; 0.95)),
        p98s: seconds(pct($runs; 0.98)),
        maxs: seconds($runs[-1] // null),
        wait_max_ms: ($waits[-1] // 0),
        throttle_max_ms: ($throttles[-1] // 0),
        trace_sha256: $trace_sha
      }
  ' "$traces" >> "$tmp"
done

if [[ "$mode" == "markdown" ]]; then
  jq -s -r '
    "| run | params | n | failed attempts | p50 | p80 | p95 | p98 | max | wait max | throttle max |",
    "|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|",
    (.[] | "| `\(.run)` | \(.params) | \(.n) | \(.failed_attempts) | \(.p50s)s | \(.p80s)s | \(.p95s)s | \(.p98s)s | \(.maxs)s | \(.wait_max_ms)ms | \(.throttle_max_ms)ms |")
  ' "$tmp"
else
  jq -s -r '
    (["profile","run","params","n","failed_attempts","p50s","p80s","p95s","p98s","maxs","waitMaxMs","thrMaxMs","traceSha256"] | @tsv),
    (.[] | [.profile,.run,.params,.n,.failed_attempts,.p50s,.p80s,.p95s,.p98s,.maxs,.wait_max_ms,.throttle_max_ms,.trace_sha256] | @tsv)
  ' "$tmp"
fi
