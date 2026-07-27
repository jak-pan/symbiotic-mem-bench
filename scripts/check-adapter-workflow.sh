#!/usr/bin/env bash
# Static fail-closed guard for the native adapter setup in CI.
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"

python3 - "$repo_root/.github/workflows/ci.yml" <<'PY'
import re
import sys
from pathlib import Path

workflow = Path(sys.argv[1]).read_text()


def job(name):
    match = re.search(
        rf"^  {re.escape(name)}:\n(?P<body>.*?)(?=^  [a-zA-Z0-9_-]+:\n|\Z)",
        workflow,
        re.MULTILINE | re.DOTALL,
    )
    if match is None:
        raise AssertionError(f"missing {name} job")
    return match.group("body")


for name in ("rust", "adapter-build"):
    body = job(name)
    required = {
        "Ubuntu runner": "runs-on: ubuntu-latest",
        "bounded cold-build timeout": "timeout-minutes: 180",
        "offline pin gate": "./scripts/check-adapter-pins.sh",
        "validated pin output": 'echo "sha=$(tr -d \'\\n\' < .symbiotic-memory-pin)"',
        "secret preflight": 'test -n "$SYMBIOTIC_MEMORY_DEPLOY_KEY"',
        "Cargo SSH agent": "uses: webfactory/ssh-agent@v0.9.0",
        "scoped deploy key": "ssh-key: ${{ secrets.SYMBIOTIC_MEMORY_DEPLOY_KEY }}",
        "canonical source": "repository: symbiotic-sh/symbiotic-memory",
        "dynamic exact ref": "ref: ${{ steps.adapter_pin.outputs.sha }}",
        "non-persistent checkout credentials": "persist-credentials: false",
        "upstream packaging wrapper": "./scripts/prepare-adapter-zvec.sh",
        "target match": "x86_64-unknown-linux-gnu",
        "zvec build path": 'echo "ZVEC_LIB_DIR=$zvec_dir"',
        "linker path": 'echo "LIBRARY_PATH=$zvec_dir',
        "runtime linker path": 'echo "LD_LIBRARY_PATH=$zvec_dir',
        "cross-step export": '>> "$GITHUB_ENV"',
    }
    for description, needle in required.items():
        if needle not in body:
            raise AssertionError(f"{name}: missing {description}")
    if "Swatinem/rust-cache" in body:
        raise AssertionError(f"{name}: native/private-derived build cache is forbidden")
    if "insteadOf" in body:
        raise AssertionError(f"{name}: global Git URL rewriting is not scoped authentication")
    first_cargo = body.find("cargo ")
    prepare = body.find("./scripts/prepare-adapter-zvec.sh")
    if first_cargo != -1 and first_cargo < prepare:
        raise AssertionError(f"{name}: Cargo runs before verified zvec preparation")

if "\n  adapter-key:" in workflow:
    raise AssertionError("adapter build must fail on a missing key, not be conditionally skipped")

adapter_feature_jobs = set()
for match in re.finditer(
    r"^  ([a-zA-Z0-9_-]+):\n(?P<body>.*?)(?=^  [a-zA-Z0-9_-]+:\n|\Z)",
    workflow,
    re.MULTILINE | re.DOTALL,
):
    if "--features symbiotic-memory-adapter" in match.group("body"):
        adapter_feature_jobs.add(match.group(1))
if adapter_feature_jobs != {"rust", "adapter-build"}:
    raise AssertionError(
        "every adapter-enabled job must use the protected native setup; found "
        + ", ".join(sorted(adapter_feature_jobs))
    )

print("OK: every adapter-enabled Ubuntu job has fail-closed pinned zvec preparation")
PY
