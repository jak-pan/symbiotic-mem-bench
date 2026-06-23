#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Summarize ingest-stage tuning runs.

Usage:
  scripts/report-ingest-stage-tuning.sh [--markdown] run-root [...]

The script reads provider-queue/model-queue-traces.jsonl and
benchmark-report.json. It does not call providers and does not mutate runs.
USAGE
}

mode="tsv"
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
    --)
      shift
      break
      ;;
    *)
      break
      ;;
  esac
done

if [[ $# -eq 0 ]]; then
  usage >&2
  exit 2
fi

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

tmp="$(mktemp)"
trap 'rm -f "$tmp"' EXIT

for run in "$@"; do
  traces="$run/provider-queue/model-queue-traces.jsonl"
  report="$run/benchmark-report.json"
  if [[ ! -f "$traces" ]]; then
    echo "warning: missing traces: $traces" >&2
    continue
  fi

  run_name="$(basename "$run")"
  diagnostic="?"
  trace_sha="$(shasum -a 256 "$traces" | awk '{print $1}')"
  if [[ -f "$report" ]]; then
    diagnostic="$(jq -r '.run_params.ingest_diagnostic // .run_params.ingest_diagnostic_mode // "none"' "$report")"
  fi

  jq -s --arg run "$run_name" --arg path "$run" --arg diagnostic "$diagnostic" --arg trace_sha "$trace_sha" '
    def pct($a; $p):
      ($a|length) as $n
      | if $n == 0 then null else $a[((($n - 1) * $p) | floor)] end;
    def seconds($ms): if $ms == null then null else (($ms / 1000) * 1000 | round / 1000) end;
    def sum($a): reduce $a[] as $x (0; . + ($x // 0));
    def epoch($ts): ($ts | sub("\\.[0-9]+Z$"; "Z") | fromdateiso8601);

    group_by(.queue_id // .queue // "unknown")
    | .[]
    | . as $events
    | [$events[] | select(type=="object" and .status=="succeeded") | .run_ms] | sort as $runs
    | [$events[] | select(type=="object" and .status=="failed")] as $fails
    | [$events[] | select(type=="object" and .status=="running") | (.queue_wait_ms // 0)] | sort as $waits
    | [$events[] | select(type=="object" and .status=="running") | (.throttle_wait_ms // 0)] | sort as $throttles
    | [$events[] | select(type=="object" and .status=="succeeded") | (.usage.prompt_tokens // .input_units // 0)] | sort as $input_units
    | [$events[] | select(type=="object" and .status=="succeeded") | (.usage.completion_tokens // .output_units // 0)] | sort as $output_units
    | [$events[] | select(type=="object" and .timestamp) | epoch(.timestamp)] | sort as $epochs
    | {
        run: $run,
        path: $path,
        diagnostic: $diagnostic,
        queue: (($events[0].queue_id // $events[0].queue // "unknown") | tostring),
        op: (($events[0].operation // $events[0].op // "") | tostring),
        n: ($runs | length),
        failed_attempts: ($fails | length),
        input_units: sum($input_units),
        output_units: sum($output_units),
        input_p50: (pct($input_units; 0.50) // 0),
        input_p95: (pct($input_units; 0.95) // 0),
        input_max: ($input_units[-1] // 0),
        output_p50: (pct($output_units; 0.50) // 0),
        output_p95: (pct($output_units; 0.95) // 0),
        output_max: ($output_units[-1] // 0),
        p50s: seconds(pct($runs; 0.50)),
        p80s: seconds(pct($runs; 0.80)),
        p95s: seconds(pct($runs; 0.95)),
        p98s: seconds(pct($runs; 0.98)),
        maxs: seconds($runs[-1] // null),
        wait_max_ms: ($waits[-1] // 0),
        throttle_max_ms: ($throttles[-1] // 0),
        trace_wall_s: (if ($epochs | length) == 0 then null else ((($epochs[-1] - $epochs[0]) * 1000 | round) / 1000) end),
        trace_sha256: $trace_sha
      }
  ' "$traces" >> "$tmp"
done

if [[ "$mode" == "markdown" ]]; then
  jq -s -r '
    "| run | diag | queue | n | failed | in total | in p50 | in p95 | in max | out total | out p50 | out p95 | out max | p50 | p80 | p95 | p98 | max | wait | throttle | trace wall |",
    "|---|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|",
    (.[] | "| `\(.run)` | \(.diagnostic) | `\(.queue)` | \(.n) | \(.failed_attempts) | \(.input_units) | \(.input_p50) | \(.input_p95) | \(.input_max) | \(.output_units) | \(.output_p50) | \(.output_p95) | \(.output_max) | \(.p50s)s | \(.p80s)s | \(.p95s)s | \(.p98s)s | \(.maxs)s | \(.wait_max_ms)ms | \(.throttle_max_ms)ms | \(.trace_wall_s)s |")
  ' "$tmp"
else
  jq -s -r '
    (["run","diagnostic","queue","op","n","failed","inputUnits","inputP50","inputP95","inputMax","outputUnits","outputP50","outputP95","outputMax","p50s","p80s","p95s","p98s","maxs","waitMaxMs","thrMaxMs","traceWallS","traceSha256"] | @tsv),
    (.[] | [.run,.diagnostic,.queue,.op,.n,.failed_attempts,.input_units,.input_p50,.input_p95,.input_max,.output_units,.output_p50,.output_p95,.output_max,.p50s,.p80s,.p95s,.p98s,.maxs,.wait_max_ms,.throttle_max_ms,.trace_wall_s,.trace_sha256] | @tsv)
  ' "$tmp"
fi
