#!/usr/bin/env bash
# Offline gate: the public root manifest contains no Git dependency, while
# every Git dependency in the isolated private adapter manifest is pinned to
# an exact rev and its own lockfile resolves that same rev.
#
# The adapter feature builds against private Symbiotic Memory crates and public
# Symbiotic Foundation crates. Without
# this check a lockfile refresh could silently float the adapter onto a
# different commit than the manifest advertises, so a clean clone would build
# something other than what the release notes claim. This runs anywhere — no
# network, no credentials — which is why it gates every CI run while the
# credentialed build check (scripts/check-adapter-build.sh) cannot.
set -euo pipefail
cd "$(dirname "$0")/.."

./scripts/check-memory-adapter-boundary.sh

python3 - <<'PY'
import re
import sys

root_manifest = open("Cargo.toml").read()
manifest = open("adapters/symbiotic-memory/Cargo.toml").read()
lock = open("adapters/symbiotic-memory/Cargo.lock").read()
build_script = open("adapters/symbiotic-memory/build.rs").read()
adapter_gate = open("scripts/check-adapter-build.sh").read()

# `name = { git = "URL", rev = "SHA", ... }` — one line per dependency.
pattern = re.compile(r'^(?P<name>[A-Za-z0-9_-]+)\s*=\s*\{[^}]*git\s*=\s*"(?P<url>[^"]+)"[^}]*\}', re.M)

failures = []
checked = 0
if re.search(r'git\s*=\s*"', root_manifest):
    failures.append("public root Cargo.toml contains a Git dependency")
for match in pattern.finditer(manifest):
    name, url, line = match.group("name"), match.group("url"), match.group(0)
    rev = re.search(r'rev\s*=\s*"([0-9a-f]{40})"', line)
    if not rev:
        failures.append(f"{name}: git dependency is not pinned to a 40-char rev")
        continue
    rev = rev.group(1)
    checked += 1
    # Cargo records `git+URL?rev=REV#RESOLVED`; the resolved sha must equal it.
    if f"git+{url}?rev={rev}#{rev}" not in lock:
        failures.append(
            f"{name}: Cargo.lock does not resolve {url} to the pinned rev {rev}"
        )

memory_rev = re.search(
    r'^symbiotic-memory\s*=\s*\{[^}]*rev\s*=\s*"([0-9a-f]{40})"',
    manifest,
    re.M,
)
build_rev = re.search(
    r'^const SYMBIOTIC_MEMORY_REV: &str = "([0-9a-f]{40})";',
    build_script,
    re.M,
)
if not memory_rev or not build_rev or memory_rev.group(1) != build_rev.group(1):
    failures.append("adapter build.rs zvec provenance revision does not match the locked symbiotic-memory dependency")

for required in [
    "cargo metadata --manifest-path \"$manifest\" --locked --format-version 1",
    "scripts/zvec-package.sh\" prepare",
    "--target \"$host_target\"",
    "export ZVEC_LIB_DIR",
    "export ZVEC_LIB_SHA256",
]:
    if required not in adapter_gate:
        failures.append(
            f"credentialed adapter gate does not provision the locked host zvec package: missing {required!r}"
        )

if not checked:
    failures.append("no pinned git dependencies found — did the manifest change shape?")

for failure in failures:
    print(f"FAIL: {failure}", file=sys.stderr)
if failures:
    sys.exit(1)
print(f"OK: public root has no Git deps; {checked} private-adapter Git dependencies are pinned and locked")
PY
