#!/usr/bin/env bash
# Offline gate for the adapter dependency identity. The manifest, repository
# pin, and every adapter package carried in Cargo.lock must agree exactly.
set -euo pipefail

for forbidden_git_env in \
  GIT_DIR \
  GIT_WORK_TREE \
  GIT_COMMON_DIR \
  GIT_INDEX_FILE \
  GIT_SHALLOW_FILE \
  GIT_NAMESPACE \
  GIT_REPLACE_REF_BASE \
  GIT_OBJECT_DIRECTORY \
  GIT_ALTERNATE_OBJECT_DIRECTORIES; do
  if [[ -n "${!forbidden_git_env-}" ]]; then
    echo "FAIL: $forbidden_git_env can substitute checkout identity" >&2
    exit 1
  fi
done
while IFS='=' read -r env_name _; do
  case "$env_name" in
    GIT_CONFIG|GIT_CONFIG_*)
      echo "FAIL: $env_name can inject checkout configuration" >&2
      exit 1
      ;;
  esac
done < <(env)
export GIT_NO_REPLACE_OBJECTS=1

safe_git() {
  GIT_NO_REPLACE_OBJECTS=1 command git "$@"
}

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
manifest="$repo_root/Cargo.toml"
lock="$repo_root/Cargo.lock"
pin_file="$repo_root/.symbiotic-memory-pin"
checkout=""
self_test=false

usage() {
  echo "usage: check-adapter-pins.sh [--self-test] [--manifest PATH --lock PATH --pin-file PATH] [--checkout PATH]" >&2
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --self-test)
      self_test=true
      shift
      ;;
    --manifest)
      manifest="${2:?missing path after --manifest}"
      shift 2
      ;;
    --lock)
      lock="${2:?missing path after --lock}"
      shift 2
      ;;
    --pin-file)
      pin_file="${2:?missing path after --pin-file}"
      shift 2
      ;;
    --checkout)
      checkout="${2:?missing path after --checkout}"
      shift 2
      ;;
    *)
      usage
      exit 2
      ;;
  esac
done

if [[ "$self_test" == true ]]; then
  python3 - <<'PY'
from urllib.parse import urlsplit

CANONICAL = {
    "https://github.com/symbiotic-sh/symbiotic-memory",
    "https://github.com/symbiotic-sh/symbiotic-memory.git",
    "ssh://git@github.com/symbiotic-sh/symbiotic-memory",
    "ssh://git@github.com/symbiotic-sh/symbiotic-memory.git",
    "git@github.com:symbiotic-sh/symbiotic-memory",
    "git@github.com:symbiotic-sh/symbiotic-memory.git",
}
REJECTED = {
    "http://github.com/symbiotic-sh/symbiotic-memory",
    "https://github.com/symbiotic-sh/symbiotic-memory/",
    "https://token@github.com/symbiotic-sh/symbiotic-memory",
    "https://github.com/symbiotic-sh/symbiotic-memory?ref=main",
    "https://github.com/symbiotic-sh/symbiotic-memory/other",
    "ssh://root@github.com/symbiotic-sh/symbiotic-memory",
    "ssh://git@github.com/Symbiotic-sh/symbiotic-memory",
    "git@github.com:symbiotic-sh/symbiotic-memory/other",
    "git@github.com:jak-pan/symbiotic-memory",
}

for url in CANONICAL:
    assert "symbiotic-sh/symbiotic-memory" in url
for url in REJECTED:
    assert url not in CANONICAL
    # Ensure the fixtures include syntactically plausible lookalikes.
    assert urlsplit(url.replace("git@github.com:", "ssh://git@github.com/")).scheme
print(
    f"OK: {len(CANONICAL)} canonical checkout transports accepted; "
    f"{len(REJECTED)} unsafe or non-canonical forms rejected"
)
PY
  exit 0
fi

python3 - "$manifest" "$lock" "$pin_file" <<'PY'
import re
import sys
import tomllib
from pathlib import Path

manifest_path, lock_path, pin_path = map(Path, sys.argv[1:])
EXPECTED_REPOSITORY = "ssh://git@github.com/symbiotic-sh/symbiotic-memory"
REQUIRED_PACKAGES = (
    "symbiotic-memory",
    "symbiotic-memory-config",
    "zvec",
    "zvec-sys",
)

failures = []

try:
    manifest_raw = manifest_path.read_text()
    lock_raw = lock_path.read_text()
    pin_raw = pin_path.read_text()
except OSError as error:
    print(f"FAIL: {error}", file=sys.stderr)
    sys.exit(1)

if not re.fullmatch(r"[0-9a-f]{40}\n?", pin_raw):
    failures.append(
        ".symbiotic-memory-pin must contain exactly one lowercase 40-character SHA"
    )
pin = pin_raw.strip()

try:
    manifest = tomllib.loads(manifest_raw)
except tomllib.TOMLDecodeError as error:
    failures.append(f"{manifest_path}: invalid TOML: {error}")
    manifest = {}
try:
    lock = tomllib.loads(lock_raw)
except tomllib.TOMLDecodeError as error:
    failures.append(f"{lock_path}: invalid TOML: {error}")
    lock = {}

dependencies = manifest.get("dependencies", {})
packages = lock.get("package", [])
if not isinstance(dependencies, dict):
    failures.append("manifest dependencies must be a table")
    dependencies = {}
if not isinstance(packages, list):
    failures.append("Cargo.lock package entries must be an array of tables")
    packages = []


def iter_git_dependencies(value, path=()):
    if isinstance(value, dict):
        if "git" in value:
            yield path, value
        for key, child in value.items():
            yield from iter_git_dependencies(child, path + (str(key),))
    elif isinstance(value, list):
        for index, child in enumerate(value):
            yield from iter_git_dependencies(child, path + (str(index),))


def iter_nodes(value, path=()):
    yield path, value
    if isinstance(value, dict):
        for key, child in value.items():
            yield from iter_nodes(child, path + (str(key),))
    elif isinstance(value, list):
        for index, child in enumerate(value):
            yield from iter_nodes(child, path + (str(index),))


checked_git_dependencies = 0
for dependency_path, dependency in iter_git_dependencies(manifest):
    checked_git_dependencies += 1
    dependency_name = ".".join(dependency_path) or "<root>"
    url = dependency.get("git")
    rev = dependency.get("rev")
    if not isinstance(url, str) or not url:
        failures.append(f"{dependency_name}: git dependency has an invalid URL")
        continue
    if not isinstance(rev, str) or not re.fullmatch(r"[0-9a-f]{40}", rev):
        failures.append(
            f"{dependency_name}: git dependency is not pinned to a lowercase 40-character rev"
        )
        continue
    package_name = dependency.get(
        "package", dependency_path[-1] if dependency_path else None
    )
    if not isinstance(package_name, str) or not package_name:
        failures.append(f"{dependency_name}: cannot determine dependency package name")
        continue
    matching = [
        package for package in packages
        if isinstance(package, dict) and package.get("name") == package_name
    ]
    if len(matching) != 1:
        failures.append(
            f"{dependency_name}: Cargo.lock must contain exactly one {package_name} "
            f"entry; found {len(matching)}"
        )
        continue
    expected_source = f"git+{url}?rev={rev}#{rev}"
    if matching[0].get("source") != expected_source:
        failures.append(
            f"{dependency_name}: Cargo.lock does not resolve exact source "
            f"{expected_source}"
        )
if checked_git_dependencies == 0:
    failures.append("no git dependencies found in the manifest")

patches = manifest.get("patch")
if isinstance(patches, dict):
    for path, value in iter_nodes(patches, ("patch",)):
        if isinstance(value, dict) and "path" in value:
            failures.append(
                f"{'.'.join(path)}: [patch] path dependencies are forbidden"
            )

for dependency_name in ("symbiotic-memory", "symbiotic-memory-config"):
    dependency = dependencies.get(dependency_name)
    if not isinstance(dependency, dict):
        failures.append(f"{dependency_name}: required manifest dependency is missing")
        continue
    url = dependency.get("git")
    rev = dependency.get("rev")
    if url != EXPECTED_REPOSITORY:
        failures.append(
            f"{dependency_name}: expected exact canonical SSH source "
            f"{EXPECTED_REPOSITORY}, got {url!r}"
        )
    if rev != pin:
        failures.append(
            f"{dependency_name}: manifest rev {rev!r} does not match "
            f".symbiotic-memory-pin {pin!r}"
        )

expected_lock_source = f"git+{EXPECTED_REPOSITORY}?rev={pin}#{pin}"
for package_name in REQUIRED_PACKAGES:
    matching = [
        package for package in packages
        if isinstance(package, dict) and package.get("name") == package_name
    ]
    if len(matching) != 1:
        failures.append(
            f"{package_name}: Cargo.lock must contain exactly one package entry; "
            f"found {len(matching)}"
        )
        continue
    source = matching[0].get("source")
    if source != expected_lock_source:
        failures.append(
            f"{package_name}: expected Cargo.lock source "
            f"{expected_lock_source}, got {source!r}"
        )

# Fail even if a stale or mixed owner/revision is hidden in an additional
# package block rather than one of the four expected names.
for package in packages:
    if not isinstance(package, dict):
        continue
    source = package.get("source")
    if not isinstance(source, str) or "symbiotic-memory" not in source:
        continue
    name = package.get("name", "<unnamed>")
    if name not in REQUIRED_PACKAGES:
        failures.append(
            f"{name}: unexpected Cargo.lock package sourced from symbiotic-memory"
        )
    if source != expected_lock_source:
        failures.append(
            f"{name}: stale, mixed, or non-canonical symbiotic-memory source {source}"
        )

for failure in dict.fromkeys(failures):
    print(f"FAIL: {failure}", file=sys.stderr)
if failures:
    sys.exit(1)
print(
    f"OK: {checked_git_dependencies} manifest git dependencies resolve exactly; "
    "Cargo.lock binds symbiotic-memory, "
    "symbiotic-memory-config, zvec, and zvec-sys to the exact canonical pin"
)
PY

if [[ -z "$checkout" ]]; then
  exit 0
fi

if [[ ! -e "$checkout/.git" ]] || \
  [[ "$(safe_git -C "$checkout" rev-parse --is-inside-work-tree 2>/dev/null || true)" != true ]]; then
  echo "FAIL: adapter checkout is not a git worktree: $checkout" >&2
  exit 1
fi

pin="$(tr -d '\n' < "$pin_file")"
head_sha="$(safe_git -C "$checkout" rev-parse --verify 'HEAD^{commit}')"
if [[ "$head_sha" != "$pin" ]]; then
  echo "FAIL: adapter checkout HEAD $head_sha does not match pin $pin" >&2
  exit 1
fi

origin_count="$(safe_git -C "$checkout" config --get-all remote.origin.url | wc -l | tr -d ' ')"
origin="$(safe_git -C "$checkout" config --get-all remote.origin.url || true)"
case "$origin" in
  https://github.com/symbiotic-sh/symbiotic-memory | \
  https://github.com/symbiotic-sh/symbiotic-memory.git | \
  ssh://git@github.com/symbiotic-sh/symbiotic-memory | \
  ssh://git@github.com/symbiotic-sh/symbiotic-memory.git | \
  git@github.com:symbiotic-sh/symbiotic-memory | \
  git@github.com:symbiotic-sh/symbiotic-memory.git)
    ;;
  *)
    echo "FAIL: adapter checkout origin is not the canonical repository: $origin" >&2
    exit 1
    ;;
esac
if [[ "$origin_count" != 1 ]]; then
  echo "FAIL: adapter checkout must have exactly one origin URL" >&2
  exit 1
fi

unsafe_config=""
while IFS= read -r config_key; do
  normalized_key="$(printf '%s' "$config_key" | tr '[:upper:]' '[:lower:]')"
  case "$normalized_key" in
    include.*|includeif.*|core.worktree|core.bare|*replace*|*graft*|*namespace*|*alternate*|*objectdirectory*|*.insteadof|*.pushinsteadof)
      unsafe_config="$config_key"
      break
      ;;
  esac
done < <(safe_git -C "$checkout" config --local --name-only --list)
if [[ -n "$unsafe_config" ]]; then
  echo "FAIL: adapter checkout contains identity-substitution config: $unsafe_config" >&2
  exit 1
fi

unsafe_ref="$(safe_git -C "$checkout" for-each-ref \
  --format='%(refname)' refs/replace refs/namespaces | head -n 1)"
if [[ -n "$unsafe_ref" ]]; then
  echo "FAIL: adapter checkout contains replacement or namespaced ref: $unsafe_ref" >&2
  exit 1
fi

git_common_dir="$(safe_git -C "$checkout" rev-parse --path-format=absolute --git-common-dir)"
grafts_path="$git_common_dir/info/grafts"
alternates_path="$git_common_dir/objects/info/alternates"
for substitution_file in "$grafts_path" "$alternates_path"; do
  if [[ -s "$substitution_file" ]]; then
    echo "FAIL: adapter checkout contains identity-substitution file: $substitution_file" >&2
    exit 1
  fi
done

unsafe_index_flag="$(safe_git -C "$checkout" ls-files -v | awk '$1 !~ /^H/ { print; exit }')"
if [[ -n "$unsafe_index_flag" ]]; then
  echo "FAIL: adapter checkout contains hidden index state: $unsafe_index_flag" >&2
  exit 1
fi
if [[ -n "$(safe_git -C "$checkout" status --porcelain=v1 --untracked-files=all)" ]]; then
  echo "FAIL: adapter checkout is not clean" >&2
  exit 1
fi

echo "OK: adapter checkout is canonical, clean, and exactly at $pin"
