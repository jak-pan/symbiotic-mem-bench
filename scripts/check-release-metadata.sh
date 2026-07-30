#!/usr/bin/env bash
# Candidate mode validates declared release identity without requiring a tag.
# Tag mode additionally proves the exact tag exists, peels to HEAD, and the
# checkout is clean. --artifact verifies the built static landing evidence.
set -euo pipefail
cd "$(dirname "$0")/.."

MODE=""
EXPECTED_TAG=""
CHECK_ARTIFACT=false

while [[ "$#" -gt 0 ]]; do
  case "$1" in
    --candidate)
      [[ -z "$MODE" ]] || { echo "choose exactly one of --candidate or --tag" >&2; exit 2; }
      MODE="candidate"
      shift
      ;;
    --tag)
      [[ -z "$MODE" && "$#" -ge 2 ]] || { echo "--tag requires one tag and no other mode" >&2; exit 2; }
      MODE="tag"
      EXPECTED_TAG="$2"
      shift 2
      ;;
    --artifact)
      CHECK_ARTIFACT=true
      shift
      ;;
    *)
      echo "usage: $0 (--candidate | --tag vX.Y.Z) [--artifact]" >&2
      exit 2
      ;;
  esac
done

if [[ -z "$MODE" ]]; then
  echo "usage: $0 (--candidate | --tag vX.Y.Z) [--artifact]" >&2
  exit 2
fi

python3 - "$MODE" "$EXPECTED_TAG" "$CHECK_ARTIFACT" <<'PY'
import hashlib
import json
import pathlib
import re
import subprocess
import sys
import tomllib

root = pathlib.Path(".")
mode, expected_tag, artifact_flag = sys.argv[1:]
check_artifact = artifact_flag == "true"


def git(*args: str, check: bool = True) -> str:
    result = subprocess.run(
        ["git", *args],
        cwd=root,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    if check and result.returncode != 0:
        detail = result.stderr.strip() or result.stdout.strip()
        raise RuntimeError(f"git {' '.join(args)} failed: {detail}")
    return result.stdout.strip()


def sha256(path: pathlib.Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def dist_tree_digest(directory: pathlib.Path) -> str:
    digest = hashlib.sha256()
    for path in sorted(
        candidate
        for candidate in directory.rglob("*")
        if candidate.is_file() and candidate.relative_to(directory).as_posix() != "version.json"
    ):
        relative = path.relative_to(directory).as_posix()
        digest.update(relative.encode())
        digest.update(b"\0")
        digest.update(sha256(path).encode())
        digest.update(b"\n")
    return digest.hexdigest()


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
snapshot_path = root / str(landing.get("snapshot_path", ""))
snapshot_digest = (leaderboard.get("source") or {}).get("records_digest")
head = git("rev-parse", "HEAD")

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
if landing.get("evidence_path") != "dashboard/dist/version.json":
    failures.append("landing evidence_path must be dashboard/dist/version.json")
if landing.get("dist_digest") != (
    "sha256(path + NUL + sha256(file) + LF), sorted paths, excluding version.json"
):
    failures.append("landing dist_digest algorithm is not the reviewed v1 contract")

if mode == "tag":
    if tag != expected_tag:
        failures.append(f"checked tag {expected_tag!r} does not match release tag {tag!r}")
    ref = f"refs/tags/{expected_tag}"
    exists = subprocess.run(
        ["git", "show-ref", "--verify", "--quiet", ref],
        cwd=root,
        check=False,
    ).returncode == 0
    if not exists:
        failures.append(f"tag ref {ref} does not exist")
    else:
        peeled = git("rev-parse", f"{ref}^{{commit}}")
        if peeled != head:
            failures.append(f"tag {expected_tag} peels to {peeled}, not exact HEAD {head}")
    if git("status", "--porcelain", "--untracked-files=all"):
        failures.append("tag verification requires a clean checkout")

if check_artifact:
    artifact_path = root / str(landing.get("artifact_path", ""))
    evidence_path = root / str(landing.get("evidence_path", ""))
    built_snapshot_path = artifact_path / "data/leaderboard.json"
    index_path = artifact_path / "index.html"
    if not artifact_path.is_dir():
        failures.append(f"landing artifact directory is missing: {artifact_path}")
    if not evidence_path.is_file():
        failures.append(f"landing evidence is missing: {evidence_path}")
    if not built_snapshot_path.is_file():
        failures.append(f"built leaderboard snapshot is missing: {built_snapshot_path}")
    if not index_path.is_file():
        failures.append(f"landing index is missing: {index_path}")
    if all(
        path.is_file()
        for path in (evidence_path, built_snapshot_path, snapshot_path, index_path)
    ):
        with evidence_path.open() as handle:
            evidence = json.load(handle)
        html = index_path.read_text()
        bundle_match = re.search(r"index-([A-Za-z0-9_-]+)\.js", html)
        expected_evidence = {
            "schema": "membench.landing-evidence.v1",
            "version": version,
            "tag": tag,
            "commit": head,
            "records_digest": snapshot_digest,
            "snapshot_sha256": sha256(snapshot_path),
            "dist_tree_sha256": dist_tree_digest(artifact_path),
            "bundle": bundle_match.group(1) if bundle_match else "unknown",
        }
        if set(evidence) != set(expected_evidence):
            failures.append(
                f"landing evidence fields are {sorted(evidence)}, "
                f"expected {sorted(expected_evidence)}"
            )
        for field, expected in expected_evidence.items():
            if evidence.get(field) != expected:
                failures.append(
                    f"landing evidence {field} is {evidence.get(field)!r}, expected {expected!r}"
                )
        if sha256(built_snapshot_path) != sha256(snapshot_path):
            failures.append("built leaderboard snapshot is stale or altered")

if failures:
    for failure in failures:
        print(f"FAIL: {failure}", file=sys.stderr)
    raise SystemExit(1)

suffix = " with verified landing artifact" if check_artifact else ""
print(
    f"OK: {mode} {tag} binds HEAD {head[:12]} and records_digest "
    f"{snapshot_digest[:12]}…{suffix}"
)
PY
