#!/usr/bin/env bash
# Prove the committed static leaderboard snapshot still describes the records
# tree in this checkout.
#
# The snapshot's `source.git_sha` is a weak witness: it names the commit that
# generated the document, which is necessarily the commit *before* the one that
# committed it, so a stale snapshot and a fresh one look alike. The real check
# is content: re-export from records/ and compare everything except the two
# fields that legitimately move (generation time and exporter sha). Any drift —
# a promoted record, an edited artifact, a changed gate verdict — fails here
# instead of being published as a truthful-looking document.
#
# Run `scripts/export-leaderboard-snapshot.sh` to fix a failure.
set -euo pipefail
cd "$(dirname "$0")/.."

SNAPSHOT="dashboard/public/data/leaderboard.json"
FRESH="$(mktemp "${TMPDIR:-/tmp}/membench-leaderboard-fresh.XXXXXX")"
trap 'rm -f "$FRESH"' EXIT

cargo run --quiet --bin membench-leaderboard -- export --records-root records > "$FRESH"

# Strip volatile provenance and filesystem-mtime fields from both sides, then diff.
strip() {
  python3 -c '
import json, sys
doc = json.load(open(sys.argv[1]))
doc.pop("generated_at", None)
doc.get("source", {}).pop("git_sha", None)
for cohort in doc.get("cohorts", []):
    for row in cohort.get("rows", []):
        row.pop("modified_ms", None)
print(json.dumps(doc, indent=2, sort_keys=True))
' "$1"
}

if ! diff -u <(strip "$SNAPSHOT") <(strip "$FRESH"); then
  echo
  echo "FAIL: $SNAPSHOT is stale — it no longer matches records/." >&2
  echo "Regenerate it with scripts/export-leaderboard-snapshot.sh and commit the diff." >&2
  exit 1
fi

DIGEST="$(python3 -c '
import json, sys
print(json.load(open(sys.argv[1])).get("source", {}).get("records_digest") or "")
' "$SNAPSHOT")"
if [ -z "$DIGEST" ]; then
  echo "FAIL: $SNAPSHOT carries no source.records_digest." >&2
  exit 1
fi

echo "OK: snapshot matches records/ (records_digest ${DIGEST:0:12}…)"
