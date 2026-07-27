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

python3 - "$@" <<'PY'
import re
import sys

CANONICAL_MEMORY_REPOSITORY = "github.com/symbiotic-sh/symbiotic-memory"
CANONICAL_MEMORY_URLS = {
    "https://github.com/symbiotic-sh/symbiotic-memory",
    "https://github.com/symbiotic-sh/symbiotic-memory.git",
    "ssh://git@github.com/symbiotic-sh/symbiotic-memory",
    "ssh://git@github.com/symbiotic-sh/symbiotic-memory.git",
    "git@github.com:symbiotic-sh/symbiotic-memory",
    "git@github.com:symbiotic-sh/symbiotic-memory.git",
}


def canonical_memory_repository(url):
    """Return the repository identity for exact supported GitHub transports."""
    if url in CANONICAL_MEMORY_URLS:
        return CANONICAL_MEMORY_REPOSITORY
    return None


LOCK_SOURCE_RE = re.compile(
    r'source = "git\+(?P<url>[^"?]+)\?rev=(?P<requested>[0-9a-f]{40})'
    r'#(?P<resolved>[0-9a-f]{40})"'
)


def lock_resolves(lock, url, rev):
    """Match an exact source, allowing Cargo's canonical transport spelling."""
    manifest_repository = canonical_memory_repository(url)
    for match in LOCK_SOURCE_RE.finditer(lock):
        if match.group("requested") != rev or match.group("resolved") != rev:
            continue
        lock_url = match.group("url")
        if lock_url == url:
            return True
        if (
            manifest_repository is not None
            and canonical_memory_repository(lock_url) == manifest_repository
        ):
            return True
    return False


def run_url_self_test():
    test_rev = "a" * 40
    canonical_lock = (
        'source = "git+ssh://git@github.com/symbiotic-sh/symbiotic-memory'
        f'?rev={test_rev}#{test_rev}"'
    )
    for url in CANONICAL_MEMORY_URLS:
        assert canonical_memory_repository(url) == CANONICAL_MEMORY_REPOSITORY, url
        assert lock_resolves(canonical_lock, url, test_rev), url
    rejected = {
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
    for url in rejected:
        assert canonical_memory_repository(url) is None, url
    print(
        f"OK: {len(CANONICAL_MEMORY_URLS)} canonical transports accepted; "
        f"{len(rejected)} unsafe or non-canonical forms rejected"
    )


if sys.argv[1:] == ["--self-test"]:
    run_url_self_test()
    sys.exit(0)
if sys.argv[1:]:
    print("usage: check-adapter-pins.sh [--self-test]", file=sys.stderr)
    sys.exit(2)

manifest = open("Cargo.toml").read()
lock = open("Cargo.lock").read()
pin_raw = open(".symbiotic-memory-pin").read()
pin = pin_raw.strip()

# `name = { git = "URL", rev = "SHA", ... }` — one line per dependency.
pattern = re.compile(r'^(?P<name>[A-Za-z0-9_-]+)\s*=\s*\{[^}]*git\s*=\s*"(?P<url>[^"]+)"[^}]*\}', re.M)

failures = []
checked = 0
memory_deps = {}
if not re.fullmatch(r"[0-9a-f]{40}\n?", pin_raw):
    failures.append(".symbiotic-memory-pin must contain exactly one lowercase 40-character SHA")
for match in pattern.finditer(manifest):
    name, url, line = match.group("name"), match.group("url"), match.group(0)
    rev = re.search(r'rev\s*=\s*"([0-9a-f]{40})"', line)
    if not rev:
        failures.append(f"{name}: git dependency is not pinned to a 40-char rev")
        continue
    rev = rev.group(1)
    checked += 1
    if name in {"symbiotic-memory", "symbiotic-memory-config"}:
        memory_deps[name] = (url, rev)
    # Cargo records `git+URL?rev=REV#RESOLVED`; the resolved sha must equal it.
    # Its lockfile may canonicalize an accepted SSH/HTTPS spelling.
    if not lock_resolves(lock, url, rev):
        failures.append(
            f"{name}: Cargo.lock does not resolve {url} to the pinned rev {rev}"
        )

if not checked:
    failures.append("no pinned git dependencies found — did the manifest change shape?")

for name in ("symbiotic-memory", "symbiotic-memory-config"):
    dependency = memory_deps.get(name)
    if dependency is None:
        failures.append(f"{name}: required Symbiotic Memory dependency is missing")
        continue
    url, rev = dependency
    if canonical_memory_repository(url) != CANONICAL_MEMORY_REPOSITORY:
        failures.append(
            f"{name}: expected canonical symbiotic-sh/symbiotic-memory SSH or HTTPS URL, got {url}"
        )
    if rev != pin:
        failures.append(
            f"{name}: manifest rev {rev} does not match .symbiotic-memory-pin {pin}"
        )

for failure in failures:
    print(f"FAIL: {failure}", file=sys.stderr)
if failures:
    sys.exit(1)
print(f"OK: {checked} git dependencies pinned and locked to exact revs")
PY
