#!/usr/bin/env bash
# Offline gate: every git dependency declared in Cargo.toml is pinned to an
# exact rev, and Cargo.lock resolves that same rev.
#
# The adapter feature builds against two private/sibling repositories. Without
# this check a lockfile refresh could silently float the adapter onto a
# different commit than the manifest advertises, so a clean clone would build
# something other than what the release notes claim. This runs anywhere — no
# network, no credentials — which is why it gates every CI run while the
# credentialed build check (scripts/check-adapter-build.sh) cannot.
set -euo pipefail
cd "$(dirname "$0")/.."

python3 - <<'PY'
import re
import sys

manifest = open("Cargo.toml").read()
lock = open("Cargo.lock").read()

# `name = { git = "URL", rev = "SHA", ... }` — one line per dependency.
pattern = re.compile(r'^(?P<name>[A-Za-z0-9_-]+)\s*=\s*\{[^}]*git\s*=\s*"(?P<url>[^"]+)"[^}]*\}', re.M)

failures = []
checked = 0
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

if not checked:
    failures.append("no pinned git dependencies found — did the manifest change shape?")

for failure in failures:
    print(f"FAIL: {failure}", file=sys.stderr)
if failures:
    sys.exit(1)
print(f"OK: {checked} git dependencies pinned and locked to exact revs")
PY
