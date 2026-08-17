#!/usr/bin/env bash
# Offline, deterministic safety gate for files GitHub source archives can publish.
set -euo pipefail
cd "$(dirname "$0")/.."

python3 - <<'PY'
import os
import pathlib
import re
import subprocess
import sys

tracked = [
    pathlib.Path(os.fsdecode(item))
    for item in subprocess.check_output(["git", "ls-files", "-z"]).split(b"\0")
    if item
]
failures = []
native_parts = {"raw", "vaults", "workflow", "provider-queue", ".debug-session"}
database_suffixes = {".sqlite", ".sqlite3", ".db"}
secret_patterns = {
    "private key": re.compile(rb"-----BEGIN (?:RSA |OPENSSH |EC )?PRIVATE KEY-----"),
    "GitHub token": re.compile(rb"gh[pousr]_[A-Za-z0-9]{30,}"),
    "OpenAI-style key": re.compile(rb"sk-[A-Za-z0-9_-]{20,}"),
    "Google API key": re.compile(rb"AIza[0-9A-Za-z_-]{35}"),
    "AWS access key": re.compile(rb"AKIA[0-9A-Z]{16}"),
    "absolute macOS home path": re.compile(rb"/Users/[^/\s]+/"),
    "absolute Linux home path": re.compile(rb"/home/[^/\s]+/"),
}

for path in tracked:
    if path.is_symlink():
        failures.append(f"tracked symlink: {path}")
    if path.parts and path.parts[0] in {"runs", "target", ".debug-session"}:
        failures.append(f"tracked local state: {path}")
    if path.parts and path.parts[0] == "records":
        if any(part in native_parts for part in path.parts):
            failures.append(f"tracked native record state: {path}")
        if path.suffix.lower() in database_suffixes:
            failures.append(f"tracked record database: {path}")
    if path.name.startswith(".env") and not path.name.endswith(".example") and path.name != ".env.example":
        failures.append(f"tracked environment file: {path}")
    if not path.is_file() or path.stat().st_size > 150 * 1024 * 1024:
        continue
    data = path.read_bytes()
    for label, pattern in secret_patterns.items():
        if pattern.search(data):
            failures.append(f"{label} pattern: {path}")

for failure in failures:
    print(f"FAIL: {failure}", file=sys.stderr)
if failures:
    raise SystemExit(1)
print(f"OK: {len(tracked)} tracked files pass publication hygiene")
PY
