#!/usr/bin/env bash
# Hostile workflow fixtures for the candidate/tag release split.
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
checker="$repo_root/scripts/check-release-workflow.sh"
fixture_root="$(mktemp -d "${TMPDIR:-/tmp}/membench-release-workflow-fixtures.XXXXXX")"
trap 'rm -rf "$fixture_root"' EXIT

new_fixture() {
  local name="$1"
  local path="$fixture_root/$name.yml"
  cp "$repo_root/.github/workflows/ci.yml" "$path"
  echo "$path"
}

replace_once() {
  local path="$1"
  local needle="$2"
  local replacement="$3"
  python3 - "$path" "$needle" "$replacement" <<'PY'
import pathlib, sys
path = pathlib.Path(sys.argv[1])
needle, replacement = sys.argv[2:]
text = path.read_text()
if needle not in text:
    raise SystemExit("fixture mutation needle was not found")
path.write_text(text.replace(needle, replacement, 1))
PY
}

expect_failure() {
  local name="$1"
  local path="$2"
  local expected="$3"
  if "$checker" --workflow "$path" >"$fixture_root/$name.out" 2>&1; then
    echo "FAIL: $name workflow fixture unexpectedly passed" >&2
    exit 1
  fi
  if ! grep -Fq -- "$expected" "$fixture_root/$name.out"; then
    echo "FAIL: $name workflow fixture failed for the wrong reason" >&2
    cat "$fixture_root/$name.out" >&2
    exit 1
  fi
  echo "OK: $name workflow fixture rejected"
}

"$checker"

path="$(new_fixture candidate-pretends-tag)"
replace_once "$path" \
  "./scripts/check-release-metadata.sh --candidate --artifact" \
  './scripts/check-release-metadata.sh --tag "$GITHUB_REF_NAME" --artifact'
expect_failure candidate-pretends-tag "$path" "dashboard PR job must invoke exact candidate"

path="$(new_fixture tag-uses-candidate)"
replace_once "$path" \
  './scripts/check-release-metadata.sh --tag "$GITHUB_REF_NAME" --artifact' \
  "./scripts/check-release-metadata.sh --candidate --artifact"
expect_failure tag-uses-candidate "$path" "release landing gate must invoke exact dynamic tag"

path="$(new_fixture tag-omits-artifact)"
replace_once "$path" \
  './scripts/check-release-metadata.sh --tag "$GITHUB_REF_NAME" --artifact' \
  './scripts/check-release-metadata.sh --tag "$GITHUB_REF_NAME"'
expect_failure tag-omits-artifact "$path" "release landing gate must invoke exact dynamic tag"

path="$(new_fixture shallow-tag)"
replace_once "$path" "fetch-depth: 0" "fetch-depth: 1"
expect_failure shallow-tag "$path" "must fetch full tag history"

path="$(new_fixture broad-tag-trigger)"
replace_once "$path" 'tags: ["v*"]' 'tags: ["*"]'
expect_failure broad-tag-trigger "$path" "push tags must be exactly"

path="$(new_fixture ungated-release)"
replace_once "$path" "    if: startsWith(github.ref, 'refs/tags/v')" "    if: success()"
expect_failure ungated-release "$path" "must be restricted to refs/tags/v"

path="$(new_fixture publishing-step)"
replace_once "$path" \
  '      - run: ./scripts/check-release-metadata.sh --tag "$GITHUB_REF_NAME" --artifact' \
  $'      - run: ./scripts/check-release-metadata.sh --tag "$GITHUB_REF_NAME" --artifact\n      - run: netlify deploy'
expect_failure publishing-step "$path" "publishing is forbidden"

echo "OK: hostile release workflow fixtures passed"
