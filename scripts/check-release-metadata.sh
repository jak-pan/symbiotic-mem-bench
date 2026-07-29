#!/usr/bin/env bash
# Verify that release identity, leaderboard provenance, and the static landing
# source ref describe one release. Pass --tag vX.Y.Z on the tagged checkout.
set -euo pipefail
cd "$(dirname "$0")/.."

EXPECTED_TAG=""
if [ "$#" -gt 0 ]; then
  if [ "$#" -ne 2 ] || [ "$1" != "--tag" ]; then
    echo "usage: $0 [--tag vX.Y.Z]" >&2
    exit 2
  fi
  EXPECTED_TAG="$2"
fi

python3 - "$EXPECTED_TAG" <<'PY'
import json
import pathlib
import sys
import tomllib

root = pathlib.Path(".")
expected_tag = sys.argv[1]

with (root / "Cargo.toml").open("rb") as handle:
    cargo = tomllib.load(handle)
with (root / "Cargo.lock").open("rb") as handle:
    cargo_lock = tomllib.load(handle)
with (root / "dashboard/package.json").open() as handle:
    dashboard = json.load(handle)
with (root / "dashboard/package-lock.json").open() as handle:
    dashboard_lock = json.load(handle)
with (root / "dashboard/public/data/leaderboard.json").open() as handle:
    leaderboard = json.load(handle)
with (root / "release/release.json").open() as handle:
    release = json.load(handle)

failures = []
version = release.get("version")
tag = release.get("tag")
landing = release.get("landing") or {}
snapshot_digest = (leaderboard.get("source") or {}).get("records_digest")

if release.get("schema") != "membench.release.v1":
    failures.append("release schema must be membench.release.v1")
if not isinstance(version, str) or not version:
    failures.append("release version is missing")
if tag != f"v{version}":
    failures.append(f"release tag {tag!r} does not match version {version!r}")
if cargo.get("package", {}).get("version") != version:
    failures.append("Cargo.toml package version does not match release version")

locked_versions = [
    package.get("version")
    for package in cargo_lock.get("package", [])
    if package.get("name") == "membench"
]
if locked_versions != [version]:
    failures.append(f"Cargo.lock membench version is {locked_versions!r}, expected {[version]!r}")
if dashboard.get("version") != version:
    failures.append("dashboard/package.json version does not match release version")
if dashboard_lock.get("version") != version:
    failures.append("dashboard/package-lock.json root version does not match release version")
if (dashboard_lock.get("packages", {}).get("") or {}).get("version") != version:
    failures.append("dashboard package-lock root package version does not match release version")
if release.get("records_digest") != snapshot_digest:
    failures.append("release records_digest does not match the bundled leaderboard snapshot")
if landing.get("source_ref") != f"refs/tags/{tag}":
    failures.append("landing source_ref must be the exact release tag ref")
if landing.get("artifact_path") != "dashboard/dist":
    failures.append("landing artifact_path must be dashboard/dist")
if landing.get("snapshot_path") != "dashboard/public/data/leaderboard.json":
    failures.append("landing snapshot_path must name the bundled leaderboard snapshot")
if expected_tag and tag != expected_tag:
    failures.append(f"checked tag {expected_tag!r} does not match release tag {tag!r}")

if failures:
    for failure in failures:
        print(f"FAIL: {failure}", file=sys.stderr)
    raise SystemExit(1)

print(
    f"OK: release {tag} binds {landing['artifact_path']} to "
    f"{landing['source_ref']} and records_digest {snapshot_digest[:12]}…"
)
PY
