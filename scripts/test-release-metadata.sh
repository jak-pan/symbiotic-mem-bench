#!/usr/bin/env bash
# Hostile fixtures for candidate/tag identity and built landing evidence.
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
fixture_root="$(mktemp -d "${TMPDIR:-/tmp}/membench-release-fixtures.XXXXXX")"
base="$fixture_root/base"
trap 'rm -rf "$fixture_root"' EXIT

mkdir -p \
  "$base/dashboard/public/data" \
  "$base/dashboard/scripts" \
  "$base/dashboard/dist" \
  "$base/release" \
  "$base/scripts"
cp "$repo_root/Cargo.toml" "$repo_root/Cargo.lock" "$repo_root/.gitignore" "$base/"
cp "$repo_root/dashboard/package.json" "$repo_root/dashboard/package-lock.json" "$base/dashboard/"
cp "$repo_root/dashboard/public/data/leaderboard.json" "$base/dashboard/public/data/"
cp "$repo_root/dashboard/scripts/write-version.mjs" "$base/dashboard/scripts/"
cp "$repo_root/release/release.json" "$base/release/"
cp "$repo_root/scripts/check-release-metadata.sh" "$base/scripts/"
cp -R "$repo_root/dashboard/dist/." "$base/dashboard/dist/"
rm -f "$base/dashboard/dist/version.json"
printf 'release fixture\n' > "$base/README.md"

git -C "$base" init --quiet --initial-branch=main
git -C "$base" config user.name "Membench release fixture"
git -C "$base" config user.email "release-fixture@example.invalid"
git -C "$base" add .
git -C "$base" commit --quiet -m "release fixture"
git -C "$base" tag -a v0.1.1 -m "release fixture"
node "$base/dashboard/scripts/write-version.mjs" >/dev/null

expect_success() {
  local name="$1"
  local path="$2"
  shift 2
  if ! (cd "$path" && ./scripts/check-release-metadata.sh "$@") \
    >"$fixture_root/$name.out" 2>&1; then
    echo "FAIL: $name unexpectedly failed" >&2
    cat "$fixture_root/$name.out" >&2
    exit 1
  fi
  echo "OK: $name accepted"
}

expect_failure() {
  local name="$1"
  local path="$2"
  local expected="$3"
  shift 3
  if (cd "$path" && ./scripts/check-release-metadata.sh "$@") \
    >"$fixture_root/$name.out" 2>&1; then
    echo "FAIL: $name unexpectedly passed" >&2
    exit 1
  fi
  if ! grep -Fq -- "$expected" "$fixture_root/$name.out"; then
    echo "FAIL: $name failed for the wrong reason" >&2
    cat "$fixture_root/$name.out" >&2
    exit 1
  fi
  echo "OK: $name rejected"
}

new_fixture() {
  local name="$1"
  local path="$fixture_root/$name"
  git clone --quiet "$base" "$path"
  mkdir -p "$path/dashboard/dist"
  cp -R "$base/dashboard/dist/." "$path/dashboard/dist/"
  echo "$path"
}

expect_success candidate "$base" --candidate --artifact
expect_success exact-tag "$base" --tag v0.1.1 --artifact

path="$(new_fixture absent-tag)"
git -C "$path" tag -d v0.1.1 >/dev/null
expect_failure absent-tag "$path" "tag ref refs/tags/v0.1.1 does not exist" --tag v0.1.1

path="$(new_fixture wrong-tag)"
expect_failure wrong-tag "$path" "checked tag 'v9.9.9' does not match" --tag v9.9.9

path="$(new_fixture non-head-tag)"
printf 'new head\n' >> "$path/README.md"
git -C "$path" add README.md
git -C "$path" -c user.name=Fixture -c user.email=fixture@example.invalid \
  commit --quiet -m "advance head"
node "$path/dashboard/scripts/write-version.mjs" >/dev/null
expect_failure non-head-tag "$path" "not exact HEAD" --tag v0.1.1 --artifact

path="$(new_fixture dirty-checkout)"
printf 'dirty\n' >> "$path/README.md"
expect_failure dirty-checkout "$path" "tag verification requires a clean checkout" \
  --tag v0.1.1 --artifact

path="$(new_fixture altered-metadata)"
python3 - "$path/release/release.json" <<'PY'
import json, pathlib, sys
path = pathlib.Path(sys.argv[1])
value = json.loads(path.read_text())
value["records_digest"] = "0" * 64
path.write_text(json.dumps(value, indent=2) + "\n")
PY
expect_failure altered-metadata "$path" "release records_digest does not match" --candidate

path="$(new_fixture altered-evidence)"
python3 - "$path/dashboard/dist/version.json" <<'PY'
import json, pathlib, sys
path = pathlib.Path(sys.argv[1])
value = json.loads(path.read_text())
value["commit"] = "0" * 40
path.write_text(json.dumps(value, indent=2) + "\n")
PY
expect_failure altered-evidence "$path" "landing evidence commit" --candidate --artifact

path="$(new_fixture stale-snapshot)"
printf '\n' >> "$path/dashboard/public/data/leaderboard.json"
expect_failure stale-snapshot "$path" "landing evidence snapshot_sha256" --candidate --artifact

path="$(new_fixture stale-dist)"
printf '\n<!-- altered -->\n' >> "$path/dashboard/dist/index.html"
expect_failure stale-dist "$path" "landing evidence dist_tree_sha256" --candidate --artifact

path="$(new_fixture version-drift)"
python3 - "$path/dashboard/package.json" <<'PY'
import json, pathlib, sys
path = pathlib.Path(sys.argv[1])
value = json.loads(path.read_text())
value["version"] = "0.1.2"
path.write_text(json.dumps(value, indent=2) + "\n")
PY
expect_failure version-drift "$path" "dashboard/package.json version does not match" --candidate

path="$(new_fixture digest-drift)"
python3 - "$path/dashboard/public/data/leaderboard.json" <<'PY'
import json, pathlib, sys
path = pathlib.Path(sys.argv[1])
value = json.loads(path.read_text())
value["source"]["records_digest"] = "f" * 64
path.write_text(json.dumps(value, indent=2) + "\n")
PY
expect_failure digest-drift "$path" "release records_digest does not match" --candidate --artifact

echo "OK: hostile release metadata fixtures passed"
