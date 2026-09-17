#!/usr/bin/env bash
# Oracle answerer/prompt test: run one answerer model + answer-prompt over the 500 oracle-gold
# questions (gold evidence fed straight to the reader), grade with the official judge, then score
# by-type and report canonical cost. This is the "reader-ceiling" isolation — it holds retrieval
# perfect so the only variables are the reader model and the answer prompt. Cheap (no ingest:
# ~$0.4-2.7/run depending on model), reuses a stored vault for structure.
#
# Usage: scripts/oracle-test.sh <operator/model> [prompt-dir] [run-name] [baseline-run]
#   <operator/model>  OpenRouter id, operator forced to openrouter. e.g. qwen/qwen3.7-plus,
#                     google/gemini-3.5-flash, qwen/qwen3.6-35b-a3b
#   [prompt-dir]      answer-prompt dir (default: /tmp/prompts-v3, the production full prompt).
#                     Pass /tmp/prompts-min etc. to test a different prompt.
#   [run-name]        output run name (default: <model-slug>-<prompt-slug>-500)
#   [baseline-run]    optional existing run to print alongside for an A/B
#
# Env overrides: MEMBENCH_ANSWER_THINKING (default on), VAULT, DS, ANSWER_OPERATOR.
# Requires OPENROUTER_API_KEY in ./.env.test.local. One paid run at a time (membench enforces).
set -u
cd "$(dirname "$0")/.." || exit 1

MODEL="${1:?usage: oracle-test.sh <operator/model> [prompt-dir] [run-name] [baseline-run]}"
PROMPT_DIR="${2:-/tmp/prompts-v3}"
slug() { printf '%s' "$1" | sed 's@.*/@@; s@[^A-Za-z0-9]@@g'; }
RUN_NAME="${3:-$(slug "$MODEL")-$(slug "$PROMPT_DIR")-500}"
BASELINE="${4:-}"

DS="${DS:-runs/inputs/longmemeval-cleaned/longmemeval_s_cleaned.json}"
VAULT="${VAULT:-runs/symbiotic-memory/long-mem-eval/500/factconsol-thinkon-500-20260624/vaults}"
ANSWER_OPERATOR="${ANSWER_OPERATOR:-openrouter}"
THINKING="${MEMBENCH_ANSWER_THINKING:-on}"

[ -d "$PROMPT_DIR" ] || { echo "prompt-dir not found: $PROMPT_DIR" >&2; exit 1; }
[ -d "$VAULT" ]      || { echo "source vault not found: $VAULT" >&2; exit 1; }
[ -f .env.test.local ] && { set -a; . ./.env.test.local; set +a; }

echo ">> oracle-test  model=$MODEL  prompt=$PROMPT_DIR  run=$RUN_NAME  thinking=$THINKING"
log="/tmp/oracle-test-$RUN_NAME.log"
MEMBENCH_IGNORE_SOURCE_HASH=1 MEMBENCH_ANSWER_THINKING="$THINKING" \
MEMBENCH_ANSWER_OPERATOR="$ANSWER_OPERATOR" MEMBENCH_ANSWER_MODEL="$MODEL" \
./target/release/membench --symbiotic-memory --long-mem-eval --dataset "$DS" --limit 500 \
  --sample stratified --memory-config config/symbiotic-memory/longmemeval-raw-light.yaml \
  --memory-manifest ../symbiotic-memory/Cargo.toml --embedder openrouter --distiller llm \
  --prompt-dir "$PROMPT_DIR" --distill-prompt distill --query-planner flash \
  --answerer --answer-only --oracle-gold --source-vault-root "$VAULT" --score --judge-workers 100 \
  --run-name "$RUN_NAME" > "$log" 2>&1
rc=$?
echo ">> membench exit=$rc  ::  $(tail -1 "$log")"
[ $rc -eq 0 ] || { echo "run FAILED — see $log" >&2; exit $rc; }

# Score by-type + canonical cost (report cost is the cost.rs rollup, correct for fresh runs).
scripts/score-run.sh "$RUN_NAME" ${BASELINE:+"$BASELINE"}
