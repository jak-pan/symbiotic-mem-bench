#!/usr/bin/env bash
# File-backed regression fixtures for scripts/check-adapter-pins.sh.
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
checker="$repo_root/scripts/check-adapter-pins.sh"
fixture_root="$(mktemp -d "${TMPDIR:-/tmp}/membench-pin-fixtures.XXXXXX")"
trap 'rm -rf "$fixture_root"' EXIT

pin="$(tr -d '\n' < "$repo_root/.symbiotic-memory-pin")"
canonical="ssh://git@github.com/symbiotic-sh/symbiotic-memory"

write_valid_fixture() {
  local directory="$1"
  mkdir -p "$directory"
  printf '%s\n' "$pin" > "$directory/pin"
  {
    printf '[dependencies]\n'
    printf 'symbiotic-memory = { git = "%s", rev = "%s" }\n' "$canonical" "$pin"
    printf 'symbiotic-memory-config = { git = "%s", rev = "%s" }\n' "$canonical" "$pin"
  } > "$directory/Cargo.toml"
  {
    printf 'version = 4\n'
    for package in symbiotic-memory symbiotic-memory-config zvec zvec-sys; do
      printf '\n[[package]]\n'
      printf 'name = "%s"\n' "$package"
      printf 'version = "0.0.0"\n'
      printf 'source = "git+%s?rev=%s#%s"\n' "$canonical" "$pin" "$pin"
    done
  } > "$directory/Cargo.lock"
}

run_check() {
  local directory="$1"
  "$checker" \
    --manifest "$directory/Cargo.toml" \
    --lock "$directory/Cargo.lock" \
    --pin-file "$directory/pin"
}

expect_failure() {
  local name="$1"
  local directory="$2"
  if run_check "$directory" > "$directory/output" 2>&1; then
    echo "FAIL: $name fixture unexpectedly passed" >&2
    exit 1
  fi
  echo "OK: $name fixture rejected"
}

valid="$fixture_root/valid"
write_valid_fixture "$valid"
run_check "$valid"

old_owner="$fixture_root/old-owner"
write_valid_fixture "$old_owner"
sed -i.bak 's#symbiotic-sh/symbiotic-memory#jak-pan/symbiotic-memory#g' \
  "$old_owner/Cargo.toml" "$old_owner/Cargo.lock"
expect_failure "old-owner" "$old_owner"

old_revision="$fixture_root/c22"
write_valid_fixture "$old_revision"
old_pin="c22cfe30c9ccc7abcee28bf6f5abe6a7a659d74e"
sed -i.bak "s/$pin/$old_pin/g" "$old_revision/Cargo.toml" "$old_revision/Cargo.lock"
expect_failure "c22 revision" "$old_revision"

mixed="$fixture_root/mixed"
write_valid_fixture "$mixed"
mixed_pin="0000000000000000000000000000000000000000"
python3 - "$mixed/Cargo.lock" "$pin" "$mixed_pin" <<'PY'
import sys
from pathlib import Path

path = Path(sys.argv[1])
text = path.read_text()
marker = 'name = "zvec"\n'
before, after = text.split(marker, 1)
after = after.replace(sys.argv[2], sys.argv[3], 2)
path.write_text(before + marker + after)
PY
expect_failure "mixed revision" "$mixed"

missing="$fixture_root/missing"
write_valid_fixture "$missing"
python3 - "$missing/Cargo.lock" <<'PY'
import sys
from pathlib import Path

path = Path(sys.argv[1])
text = path.read_text()
text = text[:text.index('\n[[package]]\nname = "zvec-sys"')]
path.write_text(text + "\n")
PY
expect_failure "missing package" "$missing"

duplicate="$fixture_root/duplicate"
write_valid_fixture "$duplicate"
{
  printf '\n[[package]]\n'
  printf 'name = "zvec"\n'
  printf 'version = "0.0.1"\n'
  printf 'source = "git+%s?rev=%s#%s"\n' "$canonical" "$pin" "$pin"
} >> "$duplicate/Cargo.lock"
expect_failure "duplicate package" "$duplicate"

add_unpinned_git_fixture() {
  local name="$1"
  local table="$2"
  local directory="$fixture_root/$name"
  write_valid_fixture "$directory"
  {
    printf '\n%s\n' "$table"
    printf '%s = { git = "https://github.com/example/rogue", branch = "main" }\n' \
      "rogue-$name"
  } >> "$directory/Cargo.toml"
  expect_failure "$name unpinned git dependency" "$directory"
}

add_unpinned_git_fixture "dev-table" "[dev-dependencies]"
add_unpinned_git_fixture "build-table" "[build-dependencies]"
add_unpinned_git_fixture "target-table" "[target.'cfg(unix)'.dependencies]"
add_unpinned_git_fixture "workspace-table" "[workspace.dependencies]"
add_unpinned_git_fixture "patch-table" "[patch.crates-io]"

pinned_unlocked="$fixture_root/pinned-unlocked-dev"
write_valid_fixture "$pinned_unlocked"
{
  printf '\n[dev-dependencies]\n'
  printf 'rogue-pinned = { git = "https://github.com/example/rogue", rev = "%s" }\n' \
    "$pin"
} >> "$pinned_unlocked/Cargo.toml"
expect_failure "pinned dev dependency missing from lock" "$pinned_unlocked"

path_patch="$fixture_root/path-patch"
write_valid_fixture "$path_patch"
{
  printf '\n[patch.crates-io]\n'
  printf 'zvec = { path = "../arbitrary-zvec" }\n'
} >> "$path_patch/Cargo.toml"
expect_failure "path patch" "$path_patch"

replacement="$fixture_root/replacement-checkout"
mkdir -p "$replacement"
git -C "$replacement" init -q
git -C "$replacement" config user.name "Membench fixture"
git -C "$replacement" config user.email "fixture@example.invalid"
printf 'reviewed\n' > "$replacement/source"
git -C "$replacement" add source
git -C "$replacement" commit -qm reviewed
reviewed_sha="$(git -C "$replacement" rev-parse HEAD)"
printf 'substitute\n' > "$replacement/source"
git -C "$replacement" commit -qam substitute
substitute_sha="$(git -C "$replacement" rev-parse HEAD)"
git -C "$replacement" checkout -q "$reviewed_sha"
git -C "$replacement" remote add origin "$canonical"

replacement_metadata="$fixture_root/replacement-metadata"
write_valid_fixture "$replacement_metadata"
sed -i.bak "s/$pin/$reviewed_sha/g" \
  "$replacement_metadata/pin" \
  "$replacement_metadata/Cargo.toml" \
  "$replacement_metadata/Cargo.lock"

git -C "$replacement" replace "$reviewed_sha" "$substitute_sha"
run_checkout_check() {
  "$checker" \
    --manifest "$replacement_metadata/Cargo.toml" \
    --lock "$replacement_metadata/Cargo.lock" \
    --pin-file "$replacement_metadata/pin" \
    --checkout "$replacement"
}

expect_checkout_failure() {
  local name="$1"
  if run_checkout_check > "$replacement_metadata/$name.output" 2>&1; then
    echo "FAIL: $name checkout fixture unexpectedly passed" >&2
    exit 1
  fi
  echo "OK: $name checkout fixture rejected"
}

expect_checkout_failure "real replacement-object"
git -C "$replacement" replace -d "$reviewed_sha" >/dev/null

git_dir="$(git -C "$replacement" rev-parse --absolute-git-dir)"
printf '%s %s\n' "$reviewed_sha" "$substitute_sha" > "$git_dir/info/grafts"
expect_checkout_failure "grafts"
rm "$git_dir/info/grafts"

git -C "$replacement" config membench.namespaceOverride evil
expect_checkout_failure "identity-substitution config"
git -C "$replacement" config --unset membench.namespaceOverride

git -C "$replacement" update-ref \
  refs/namespaces/evil/refs/heads/main "$substitute_sha"
expect_checkout_failure "custom namespace"
git -C "$replacement" update-ref -d refs/namespaces/evil/refs/heads/main

if GIT_NAMESPACE=evil run_checkout_check \
  > "$replacement_metadata/environment-namespace.output" 2>&1; then
  echo "FAIL: environment namespace checkout fixture unexpectedly passed" >&2
  exit 1
fi
echo "OK: environment namespace checkout fixture rejected"

echo "OK: adapter pin fixtures passed"
