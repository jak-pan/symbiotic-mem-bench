#!/usr/bin/env bash
# Prepare and verify the target-matched zvec package using the exact pinned
# Symbiotic Memory checkout. This script deliberately delegates provenance,
# SBOM, container-digest, and artifact checks to upstream's f6 contract.
set -euo pipefail
export GIT_NO_REPLACE_OBJECTS=1

repo_root="$(cd "$(dirname "$0")/.." && pwd)"

if [[ $# -lt 2 || $# -gt 3 ]]; then
  echo "usage: prepare-adapter-zvec.sh CHECKOUT OUTPUT_DIR [TARGET]" >&2
  exit 2
fi

checkout="$1"
output_dir="$2"
target="${3:-x86_64-unknown-linux-gnu}"

"$repo_root/scripts/check-adapter-pins.sh" --checkout "$checkout"

packager="$checkout/scripts/zvec-package.sh"
if [[ ! -x "$packager" ]]; then
  echo "FAIL: pinned checkout does not contain executable $packager" >&2
  exit 1
fi
if [[ -e "$output_dir" ]] && [[ -n "$(find "$output_dir" -mindepth 1 -print -quit)" ]]; then
  echo "FAIL: zvec output directory must start empty: $output_dir" >&2
  exit 1
fi

"$packager" prepare --target "$target" --output "$output_dir"
"$packager" verify --target "$target" --lib-dir "$output_dir"

pin="$(tr -d '\n' < "$repo_root/.symbiotic-memory-pin")"
{
  printf 'symbiotic_memory_pin=%s\n' "$pin"
  printf 'target=%s\n' "$target"
} > "$output_dir/.membench-zvec-verified"

echo "OK: target-matched zvec package prepared and verified at $output_dir"
