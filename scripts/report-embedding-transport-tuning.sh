#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Summarize embedding transport tuning runs.

Usage:
  scripts/report-embedding-transport-tuning.sh [--markdown] [--profile PROFILE] [run-root ...]

Profiles:
  openrouter-qwen3-8b-1024

If no run roots are supplied, the script summarizes the local evidence runs for
the selected profile. It reads provider-queue/model-queue-traces.jsonl and
benchmark-report.json. It does not call providers and does not mutate run folders.
USAGE
}

mode="tsv"
profile="openrouter-qwen3-8b-1024"

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
    openrouter-qwen3-8b-1024)
      runs=(
        "runs/symbiotic-memory/long-mem-eval/10/target-10q-qwen1024-h2pool4x64-rawembed-no-zvec-batch-flush-20260623-173447"
        "runs/symbiotic-memory/long-mem-eval/10/target-10q-qwen1024-h2pool4x64-rawembed-repeat-20260623-173900"
        "runs/symbiotic-memory/long-mem-eval/10/target-10q-qwen1024-http1-rawembed-20260623-174255"
        "runs/symbiotic-memory/long-mem-eval/10/target-10q-qwen1024-h2pool8x64-rawembed-20260623-174546"
        "runs/symbiotic-memory/long-mem-eval/10/target-10q-qwen1024-h2pool16x64-rawembed-20260623-174819"
        "runs/symbiotic-memory/long-mem-eval/10/target-10q-qwen1024-http1-pool16x64-rawembed-20260623-174942"
        "runs/symbiotic-memory/long-mem-eval/10/target-10q-qwen1024-h2pool64x16-rawembed-20260623-180637"
        "runs/symbiotic-memory/long-mem-eval/10/target-10q-qwen1024-h2pool32x32-rawembed-20260623-181354"
        "runs/symbiotic-memory/long-mem-eval/10/target-10q-qwen1024-http1-pool32x32-rawembed-20260623-102020"
        "runs/symbiotic-memory/long-mem-eval/10/target-10q-qwen1024-http1-pool64x16-rawembed-20260623-102400"
      )
      ;;
    *)
      echo "unknown profile: $profile" >&2
      usage >&2
      exit 2
      ;;
  esac
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
    params="$(jq -r '.run_params
      | "pool="+((.openrouter_http_client_pool_size//"default")|tostring)
        +" idle="+((.openrouter_http_pool_max_idle_per_host//"default")|tostring)
        +" http1="+((.openrouter_http1_only//"default")|tostring)' "$report")"
  fi

  jq -s --arg run "$run_name" --arg path "$run" --arg params "$params" --arg trace_sha "$trace_sha" --arg profile "$profile" '
    def pct($a; $p):
      ($a|length) as $n
      | if $n == 0 then null else $a[((($n - 1) * $p) | floor)] end;
    def seconds($ms): if $ms == null then null else (($ms / 1000) * 1000 | round / 1000) end;

    . as $events
    | [$events[] | select(type=="object" and .status=="succeeded") | .run_ms] | sort as $runs
    | [$events[] | select(type=="object" and .status=="failed")] as $fails
    | [$events[] | select(type=="object" and .status=="running") | (.queue_wait_ms // 0)] | sort as $waits
    | [$events[] | select(type=="object" and .status=="running") | (.throttle_wait_ms // 0)] | sort as $throttles
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
