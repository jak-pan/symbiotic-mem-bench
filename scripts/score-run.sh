#!/usr/bin/env bash
# Score one or more membench LongMemEval runs.
#
# Usage: scripts/score-run.sh <run-name-or-path> [<run> ...]
#
# For each run prints:
#   - acc       overall correct/total (from artifacts/verdicts.jsonl)
#   - by-type   accuracy per LongMemEval question type (ms/tr/ku/ss-user/ss-asst/ss-pref)
#   - reasoned  how many answerer calls emitted a reasoning trace ("n/a" if no debug present);
#               use this to confirm thinking actually fired (opt-in models need REASONING_EFFORT=high)
#   - cost      total run cost from the report (the cost.rs rollup, priced from the OpenRouter
#               /models catalog + native static table). Canonical for runs scored after the
#               pricing-catalog change; re-score older runs to refresh. Per-model split: /api/run.
#   - hard/ctrl tier2 (/31) and control (/30) split, shown only when the run covers those sets
#
# A run name resolves to runs/symbiotic-memory/long-mem-eval/<limit>/<name>; a full path also works.
# Pass several names to compare them. The hard/control qids come from the canonical sets in
# runs/inputs/longmemeval-hard/ (not /tmp).
set -u
cd "$(dirname "$0")/.." || exit 1
ROOT=runs/symbiotic-memory/long-mem-eval
HARD=runs/inputs/longmemeval-hard/hard-tier2-cluster31.json
CTRL=runs/inputs/longmemeval-hard/control-easy30.json
[ $# -ge 1 ] || { echo "usage: $(basename "$0") <run-name-or-path> [<run> ...]"; exit 1; }

hard_ids='[]'; [ -f "$HARD" ] && hard_ids=$(jq -c '[.[].question_id]' "$HARD" 2>/dev/null)
ctrl_ids='[]'; [ -f "$CTRL" ] && ctrl_ids=$(jq -c '[.[].question_id]' "$CTRL" 2>/dev/null)

resolve() {
  local a="$1"
  [ -f "$a/artifacts/verdicts.jsonl" ] && { printf '%s\n' "$a"; return; }
  find "$ROOT" -maxdepth 2 -type d -name "$a" 2>/dev/null | head -1
}

bytype() {  # stdin: "<question_type> <true|false>" lines
  awk '
    {tot[$1]++; if($2=="true")c[$1]++}
    END{
      a["multi-session"]="ms"; a["temporal-reasoning"]="tr"; a["knowledge-update"]="ku";
      a["single-session-user"]="ss-user"; a["single-session-assistant"]="ss-asst"; a["single-session-preference"]="ss-pref";
      split("multi-session temporal-reasoning knowledge-update single-session-user single-session-assistant single-session-preference",o," ");
      s=""; for(i=1;i<=6;i++){t=o[i]; if(tot[t]>0) s=s sprintf("%s %d/%d  ",a[t],c[t]+0,tot[t]+0)}
      printf "%s", s
    }'
}

for name in "$@"; do
  dir=$(resolve "$name")
  if [ -z "${dir:-}" ] || [ ! -f "$dir/artifacts/verdicts.jsonl" ]; then
    echo "## $name: NOT FOUND / no verdicts"; echo; continue
  fi
  V="$dir/artifacts/verdicts.jsonl"
  n=$(grep -c . "$V")
  c=$(jq -rc 'select(.autoeval_label.label==true)' "$V" 2>/dev/null | grep -c .)
  acc=$(awk "BEGIN{if($n>0)printf \"%.3f\",$c/$n; else printf \"0\"}")
  cat=$(jq -r '.question_type+" "+((.autoeval_label.label==true)|tostring)' "$V" 2>/dev/null | bytype)
  rz=$(jq -rc '(.recall.answerer_calls[0].reasoning//"")|length' "$dir"/vaults/*/debug/question-debug.json 2>/dev/null | awk '$1>0{k++}END{print k+0}')
  tot=$(jq -rc '(.recall.answerer_calls[0].reasoning//"")|length' "$dir"/vaults/*/debug/question-debug.json 2>/dev/null | grep -c .)
  reasoned="n/a"; [ "${tot:-0}" -gt 0 ] 2>/dev/null && reasoned="$rz/$tot"
  costu=$(jq -r '.metrics.cost_micro_usd//0' "$dir/benchmark-report.json" 2>/dev/null || echo 0)
  cost=$(awk "BEGIN{printf \"%.4f\",${costu:-0}/1e6}")
  echo "## $name   acc $c/$n ($acc)   reasoned $reasoned   cost ~\$$cost"
  echo "   by-type: $cat"
  ht=$(jq --argjson h "$hard_ids" -rc 'select(.question_id|IN($h[]))' "$V" 2>/dev/null | grep -c .)
  if [ "${ht:-0}" -gt 0 ] 2>/dev/null; then
    hc=$(jq --argjson h "$hard_ids" -rc 'select((.question_id|IN($h[])) and .autoeval_label.label==true)' "$V" 2>/dev/null | grep -c .)
    ct=$(jq --argjson c "$ctrl_ids" -rc 'select(.question_id|IN($c[]))' "$V" 2>/dev/null | grep -c .)
    cc=$(jq --argjson c "$ctrl_ids" -rc 'select((.question_id|IN($c[])) and .autoeval_label.label==true)' "$V" 2>/dev/null | grep -c .)
    echo "   hard $hc/$ht   control $cc/$ct"
  fi
  echo
done
