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

sha256() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}

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
case "$target" in
  aarch64-apple-darwin | x86_64-apple-darwin)
    library_name="libzvec_c_api.dylib"
    ;;
  aarch64-unknown-linux-gnu | x86_64-unknown-linux-gnu)
    library_name="libzvec_c_api.so"
    ;;
  *)
    echo "FAIL: unsupported zvec target: $target" >&2
    exit 1
    ;;
esac
library_sha256="$(sha256 "$output_dir/$library_name")"
provenance_sha256="$(sha256 "$output_dir/.zvec-provenance")"
sbom_sha256="$(sha256 "$output_dir/SBOM.spdx.json")"
{
  printf 'symbiotic_memory_pin=%s\n' "$pin"
  printf 'target=%s\n' "$target"
  printf 'library_sha256=%s\n' "$library_sha256"
  printf 'provenance_sha256=%s\n' "$provenance_sha256"
  printf 'sbom_sha256=%s\n' "$sbom_sha256"
} > "$output_dir/.membench-zvec-verified"

echo "OK: target-matched zvec package prepared and verified at $output_dir"
