#!/usr/bin/env bash
# Offline architecture gate: the Symbiotic Memory consumer must stay on the
# public application facade. Storage layout and pipeline implementation details
# belong to Symbiotic Memory and must not leak back into the benchmark.
set -euo pipefail
cd "$(dirname "$0")/.."

python3 - <<'PY'
from pathlib import Path
import re
import sys

sources = [
    Path("src/symbiotic_memory_adapter.rs"),
    Path("src/bin/membench.rs"),
]

forbidden = {
    "private Memory module import": re.compile(
        r"symbiotic_memory::(?:storage|ingest|recall|manifest|vault)(?:::|\b)"
    ),
    "Memory implementation type": re.compile(
        r"\b(?:IngestPipeline|RecallEngine|VaultStore|MemoryStore|MemoryRunManifest|IngestDiagnosticMode)\b"
    ),
    "Memory storage-layout knowledge": re.compile(
        r"(?:\.zvec\b|\bvault\.db\b|\bmemory\.sqlite\b|\breceipts\.zvec\b|\bVAULT_INDEX_DIR\b)"
    ),
}

failures = []
for source in sources:
    text = source.read_text()
    for label, pattern in forbidden.items():
        for match in pattern.finditer(text):
            line = text.count("\n", 0, match.start()) + 1
            failures.append(f"{source}:{line}: {label}: {match.group(0)!r}")

manifest = Path("adapters/symbiotic-memory/Cargo.toml").read_text()
if re.search(r"^symbiotic-memory-config\s*=", manifest, re.MULTILINE):
    failures.append(
        "adapters/symbiotic-memory/Cargo.toml: direct symbiotic-memory-config dependency"
    )

for failure in failures:
    print(f"FAIL: {failure}", file=sys.stderr)
if failures:
    sys.exit(1)

print("OK: Symbiotic Memory adapter uses only the public application facade")
PY
