#!/usr/bin/env bash
# Fail closed unless every version-bearing release surface matches one exact tag.
set -euo pipefail
cd "$(dirname "$0")/.."

tag=${1:-}
if [[ ! "$tag" =~ ^v([0-9]+\.[0-9]+\.[0-9]+)$ ]]; then
    echo "usage: $0 vMAJOR.MINOR.PATCH" >&2
    exit 2
fi
version=${BASH_REMATCH[1]}

python3 - "$version" <<'PY'
import json
import pathlib
import sys
import tomllib

expected = sys.argv[1]
root = pathlib.Path(".")
manifest = tomllib.loads((root / "Cargo.toml").read_text())
lock = tomllib.loads((root / "Cargo.lock").read_text())
adapter_manifest = tomllib.loads((root / "adapters/symbiotic-memory/Cargo.toml").read_text())
adapter_lock = tomllib.loads((root / "adapters/symbiotic-memory/Cargo.lock").read_text())
dashboard = json.loads((root / "dashboard/package.json").read_text())
dashboard_lock = json.loads((root / "dashboard/package-lock.json").read_text())

actual = {
    "Cargo.toml": manifest["package"]["version"],
    "Cargo.lock": next(
        package["version"]
        for package in lock["package"]
        if package["name"] == manifest["package"]["name"]
    ),
    "dashboard/package.json": dashboard["version"],
    "dashboard/package-lock.json": dashboard_lock["version"],
    "dashboard/package-lock.json packages['']": dashboard_lock["packages"][""]["version"],
    "adapters/symbiotic-memory/Cargo.toml": adapter_manifest["package"]["version"],
    "adapters/symbiotic-memory/Cargo.lock": next(
        package["version"]
        for package in adapter_lock["package"]
        if package["name"] == adapter_manifest["package"]["name"]
    ),
}
failures = [f"{name}: expected {expected}, got {value}" for name, value in actual.items() if value != expected]
if manifest["package"].get("publish") is not False:
    failures.append("Cargo.toml: package.publish must remain false for the GitHub bundle distribution")
if manifest["package"].get("autobins") is not False or manifest["package"].get("autotests") is not False:
    failures.append("public root must keep autobins/autotests disabled so private adapter targets stay isolated")
if adapter_manifest["package"].get("publish") is not False:
    failures.append("private adapter package must remain publish=false")

for dependency in ("symbiotic-memory",):
    spec = adapter_manifest["dependencies"][dependency]
    if spec.get("git") != "ssh://git@github.com/symbiotic-sh/symbiotic-memory":
        failures.append(f"{dependency}: must use the consented symbiotic-sh repository")
    rev = spec.get("rev", "")
    if len(rev) != 40 or any(char not in "0123456789abcdef" for char in rev):
        failures.append(f"{dependency}: rev is not an exact lowercase 40-character Git object id")

if failures:
    for failure in failures:
        print(f"FAIL: {failure}", file=sys.stderr)
    raise SystemExit(1)

print(f"OK: release versions match v{expected}; Cargo publication is disabled; private adapter pins are exact")
PY

./scripts/check-adapter-pins.sh
