#!/usr/bin/env bash
# Refresh the OpenRouter pricing catalog consumed by src/cost.rs.
#
# Pulls live per-model pricing from OpenRouter's public /models API (no key needed) and rewrites
# config/pricing/openrouter-pricing.json (USD per 1M tokens). cost.rs loads that file to price
# every openrouter-routed model; native-API models (DeepSeek/Gemini) and embeddings stay in the
# static table in cost.rs. Run this whenever prices drift, or on a cron.
set -euo pipefail
cd "$(dirname "$0")/.."
OUT=config/pricing/openrouter-pricing.json
tmp="$(mktemp)"
curl -sS --fail https://openrouter.ai/api/v1/models | jq --arg ts "$(date -u +%Y-%m-%dT%H:%M:%SZ)" '{
  _source: "https://openrouter.ai/api/v1/models",
  _fetched_at: $ts,
  _note: "USD per 1M tokens. Refresh: scripts/refresh-pricing.sh",
  models: (.data | map(select(.pricing != null)) | map({key: .id, value: {
    input_per_million_usd:  ((.pricing.prompt // "0")     | tonumber) * 1000000,
    output_per_million_usd: ((.pricing.completion // "0") | tonumber) * 1000000
  }}) | from_entries)
}' > "$tmp"
mv "$tmp" "$OUT"
echo "refreshed $OUT: $(jq '.models|length' "$OUT") models @ $(jq -r '._fetched_at' "$OUT")"
