#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Promote a local run as a dashboard-safe metadata record.

Copies:
  benchmark-report.json
  run-params.json
  artifacts/memory-traces.jsonl
  artifacts/model-traces.jsonl
  artifacts/step-analytics.json
  artifacts/score-summary.json
  workflow/
  provider-queue/model-queue-traces.jsonl

Omits:
  vaults/
  raw/
  traces/
  provider-queue/requests/
  artifacts/hypotheses.jsonl
  artifacts/provenance.jsonl
  artifacts/scored.json
  artifacts/verdicts.jsonl
  artifacts/partial-verdicts.jsonl

Usage:
  scripts/save-run-meta-record.sh RUN_ROOT [--record-name NAME] [--records-root records] [--force]
USAGE
}

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" || $# -eq 0 ]]; then
  usage
  exit 0
fi

run_root="$1"
shift
records_root="records"
record_name=""
force=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --record-name)
      record_name="${2:-}"
      if [[ -z "$record_name" ]]; then
        echo "--record-name requires a value" >&2
        exit 2
      fi
      shift 2
      ;;
    --records-root)
      records_root="${2:-}"
      if [[ -z "$records_root" ]]; then
        echo "--records-root requires a value" >&2
        exit 2
      fi
      shift 2
      ;;
    --force)
      force=1
      shift
      ;;
    *)
      echo "unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

if [[ ! -f "$run_root/benchmark-report.json" ]]; then
  echo "missing benchmark-report.json under $run_root" >&2
  exit 1
fi

sanitize() {
  printf '%s' "$1" | tr -cs 'A-Za-z0-9._-' '-'
}

system="$(jq -r '.system // "unknown"' "$run_root/benchmark-report.json")"
benchmark="$(jq -r '.benchmark // "unknown"' "$run_root/benchmark-report.json")"
limit="$(jq -r '.run_params.limit // .metrics.accuracy.total // "unknown"' "$run_root/benchmark-report.json")"
if [[ -z "$record_name" ]]; then
  record_name="$(jq -r '.run_name // "unnamed"' "$run_root/benchmark-report.json")"
fi

dest="$records_root/$(sanitize "$system")/$(sanitize "$benchmark")/$(sanitize "$limit")/$(sanitize "$record_name")"
if [[ -e "$dest" ]]; then
  if [[ "$force" != "1" ]]; then
    echo "record already exists at $dest; pass --force to overwrite" >&2
    exit 1
  fi
  rm -rf "$dest"
fi

mkdir -p "$dest"

cp "$run_root/benchmark-report.json" "$dest/benchmark-report.json"
if [[ -f "$run_root/run-params.json" ]]; then
  cp "$run_root/run-params.json" "$dest/run-params.json"
fi
if [[ -d "$run_root/artifacts" ]]; then
  mkdir -p "$dest/artifacts"
  for artifact in memory-traces.jsonl model-traces.jsonl step-analytics.json score-summary.json; do
    if [[ -f "$run_root/artifacts/$artifact" ]]; then
      cp "$run_root/artifacts/$artifact" "$dest/artifacts/$artifact"
    fi
  done
fi
if [[ -d "$run_root/workflow" ]]; then
  mkdir -p "$dest/workflow"
  cp -R "$run_root/workflow/." "$dest/workflow/"
fi
if [[ -f "$run_root/provider-queue/model-queue-traces.jsonl" ]]; then
  mkdir -p "$dest/provider-queue"
  cp "$run_root/provider-queue/model-queue-traces.jsonl" "$dest/provider-queue/model-queue-traces.jsonl"
fi

tmp_report="$(mktemp)"
trap 'rm -f "$tmp_report"' EXIT
jq '
  def p: (.run_params // .);
  def truthy($v): ($v == true or $v == "true" or $v == "1" or $v == 1);
  def openrouter_qwen_raw:
    truthy(p.stop_after_raw_embed // p.ingest_stop_after_raw_embed // false)
    and (((p.configured_models.embed.model // p.runtime_models.embed // "") | tostring) | contains("qwen3-embedding-8b"));
  def deepseek_chat_distill:
    ((p.ingest_diagnostic // p.ingest_diagnostic_mode // "") == "distill")
    and (((p.configured_models.distill.operator // p.role_settings.distill.operator // "") | tostring) == "deepseek"
      or (((p.configured_models.distill.model // p.runtime_models.distill // "") | tostring) | contains("deepseek")))
    and (((p.configured_models.distill.model // p.runtime_models.distill // "") | tostring) | contains("deepseek-v4-flash"));
  def transport_shape:
    (if truthy(p.openrouter_http1_only // false) then "h1" else "h2" end)
    + " "
    + ((p.openrouter_http_client_pool_size // "?") | tostring)
    + "x"
    + ((p.openrouter_http_pool_max_idle_per_host // "?") | tostring);
  def chat_transport_shape:
    (if truthy(p.chat_http1_only // false) then "h1" else "h2" end)
    + " "
    + ((p.chat_http_client_pool_size // "?") | tostring)
    + "x"
    + ((p.chat_http_pool_max_idle_per_host // "?") | tostring);
  def with_tuning:
    if openrouter_qwen_raw then
      .tuning = {
        "schema": "membench.tuning.v1",
        "category": "embedding_transport",
        "profile": "openrouter-qwen3-8b-1024",
        "cohort": "embed-transport/openrouter-qwen3-8b-1024-32k",
        "shape": transport_shape,
        "label": (transport_shape + " · qwen3-emb-8b 1024d")
      }
    elif deepseek_chat_distill then
      .tuning = {
        "schema": "membench.tuning.v1",
        "category": "chat_transport",
        "profile": "deepseek-v4-flash-distill",
        "cohort": "chat-transport/deepseek-v4-flash-distill",
        "shape": chat_transport_shape,
        "label": (chat_transport_shape + " · deepseek-v4-flash")
      }
    else
      .
    end;
  .artifact_manifest.native_state_available = false
  | .artifact_manifest.native_state_note = "Meta record: dashboard-safe traces retained; vaults, raw outputs, raw provider request payloads, and question-level data artifacts omitted."
  | .artifact_manifest.available = ((.artifact_manifest.available // []) | map(select(. == "memory_traces" or . == "model_traces" or . == "step_analytics" or . == "score_summary")))
  | .artifact_manifest.missing = (((.artifact_manifest.missing // []) + ["hypotheses", "provenance", "scored", "verdicts", "partial_verdicts"]) | unique)
  | .artifact_manifest.omitted_data_artifacts = ["hypotheses", "provenance", "scored", "verdicts", "partial_verdicts"]
  | .meta_record = {
      "schema": "membench.meta_record.v1",
      "omitted": ["vaults", "raw", "traces", "provider-queue/requests", "artifacts/hypotheses.jsonl", "artifacts/provenance.jsonl", "artifacts/scored.json", "artifacts/verdicts.jsonl", "artifacts/partial-verdicts.jsonl"],
      "retained": ["benchmark-report.json", "run-params.json", "artifacts/memory-traces.jsonl", "artifacts/model-traces.jsonl", "artifacts/step-analytics.json", "workflow", "provider-queue/model-queue-traces.jsonl"]
    }
  | with_tuning
' "$dest/benchmark-report.json" > "$tmp_report"
mv "$tmp_report" "$dest/benchmark-report.json"

if [[ -f "$dest/run-params.json" ]]; then
  tmp_params="$(mktemp)"
  jq '
    def p: (.run_params // .);
    def truthy($v): ($v == true or $v == "true" or $v == "1" or $v == 1);
    def openrouter_qwen_raw:
      truthy(p.stop_after_raw_embed // p.ingest_stop_after_raw_embed // false)
      and (((p.configured_models.embed.model // p.runtime_models.embed // "") | tostring) | contains("qwen3-embedding-8b"));
    def deepseek_chat_distill:
      ((p.ingest_diagnostic // p.ingest_diagnostic_mode // "") == "distill")
      and (((p.configured_models.distill.operator // p.role_settings.distill.operator // "") | tostring) == "deepseek"
        or (((p.configured_models.distill.model // p.runtime_models.distill // "") | tostring) | contains("deepseek")))
      and (((p.configured_models.distill.model // p.runtime_models.distill // "") | tostring) | contains("deepseek-v4-flash"));
    def transport_shape:
      (if truthy(p.openrouter_http1_only // false) then "h1" else "h2" end)
      + " "
      + ((p.openrouter_http_client_pool_size // "?") | tostring)
      + "x"
      + ((p.openrouter_http_pool_max_idle_per_host // "?") | tostring);
    def chat_transport_shape:
      (if truthy(p.chat_http1_only // false) then "h1" else "h2" end)
      + " "
      + ((p.chat_http_client_pool_size // "?") | tostring)
      + "x"
      + ((p.chat_http_pool_max_idle_per_host // "?") | tostring);
    def with_tuning:
      if openrouter_qwen_raw then
        .tuning = {
          "schema": "membench.tuning.v1",
          "category": "embedding_transport",
          "profile": "openrouter-qwen3-8b-1024",
          "cohort": "embed-transport/openrouter-qwen3-8b-1024-32k",
          "shape": transport_shape,
          "label": (transport_shape + " · qwen3-emb-8b 1024d")
        }
      elif deepseek_chat_distill then
        .tuning = {
          "schema": "membench.tuning.v1",
          "category": "chat_transport",
          "profile": "deepseek-v4-flash-distill",
          "cohort": "chat-transport/deepseek-v4-flash-distill",
          "shape": chat_transport_shape,
          "label": (chat_transport_shape + " · deepseek-v4-flash")
        }
      else
        .
      end;
    .artifact_manifest.native_state_available = false
    | .artifact_manifest.native_state_note = "Meta record: dashboard-safe traces retained; vaults, raw outputs, raw provider request payloads, and question-level data artifacts omitted."
    | .artifact_manifest.available = ((.artifact_manifest.available // []) | map(select(. == "memory_traces" or . == "model_traces" or . == "step_analytics" or . == "score_summary")))
    | .artifact_manifest.missing = (((.artifact_manifest.missing // []) + ["hypotheses", "provenance", "scored", "verdicts", "partial_verdicts"]) | unique)
    | .artifact_manifest.omitted_data_artifacts = ["hypotheses", "provenance", "scored", "verdicts", "partial_verdicts"]
    | .meta_record = {
        "schema": "membench.meta_record.v1",
        "omitted": ["vaults", "raw", "traces", "provider-queue/requests", "artifacts/hypotheses.jsonl", "artifacts/provenance.jsonl", "artifacts/scored.json", "artifacts/verdicts.jsonl", "artifacts/partial-verdicts.jsonl"],
        "retained": ["benchmark-report.json", "run-params.json", "artifacts/memory-traces.jsonl", "artifacts/model-traces.jsonl", "artifacts/step-analytics.json", "workflow", "provider-queue/model-queue-traces.jsonl"]
      }
    | with_tuning
  ' "$dest/run-params.json" > "$tmp_params"
  mv "$tmp_params" "$dest/run-params.json"
fi

du -sh "$dest"
echo "saved meta record: $dest"
