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

echo "OK: adapter pin fixtures passed"
