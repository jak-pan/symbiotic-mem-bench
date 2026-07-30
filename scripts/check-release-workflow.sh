#!/usr/bin/env bash
# Prove PR validation uses candidate mode and tag validation uses exact tag mode.
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
workflow="$repo_root/.github/workflows/ci.yml"
if [[ "$#" -gt 0 ]]; then
  if [[ "$#" -ne 2 || "$1" != "--workflow" ]]; then
    echo "usage: $0 [--workflow PATH]" >&2
    exit 2
  fi
  workflow="$2"
fi

parsed="$(mktemp "${TMPDIR:-/tmp}/membench-release-workflow.XXXXXX")"
trap 'rm -f "$parsed"' EXIT
ruby -ryaml -rjson - "$workflow" "$parsed" <<'RUBY'
source, destination = ARGV
document = YAML.safe_load(
  File.read(source),
  permitted_classes: [],
  permitted_symbols: [],
  aliases: false
)
unless document.is_a?(Hash)
  warn "FAIL: workflow root must be a mapping"
  exit 1
end
File.write(destination, JSON.generate(document))
RUBY

python3 - "$parsed" <<'PY'
import json
import sys
from pathlib import Path

workflow = json.loads(Path(sys.argv[1]).read_text())
triggers = workflow.get("on", workflow.get("true"))
if not isinstance(triggers, dict):
    raise AssertionError("workflow triggers must be a mapping")
push = triggers.get("push")
if not isinstance(push, dict) or push.get("tags") != ["v*"]:
    raise AssertionError("push tags must be exactly ['v*']")
if push.get("branches") != ["master"]:
    raise AssertionError("push branches must remain exactly ['master']")
if "pull_request" not in triggers:
    raise AssertionError("pull_request candidate validation must remain enabled")

jobs = workflow.get("jobs")
if not isinstance(jobs, dict):
    raise AssertionError("workflow jobs must be a mapping")
dashboard = jobs.get("dashboard")
release = jobs.get("release-landing-gate")
adapter_pins = jobs.get("adapter-pins")
if not all(isinstance(job, dict) for job in (dashboard, release, adapter_pins)):
    raise AssertionError("dashboard, adapter-pins, and release-landing-gate jobs are required")


def run_steps(job):
    return [
        step
        for step in job.get("steps", [])
        if isinstance(step, dict) and isinstance(step.get("run"), str)
    ]


dashboard_runs = [step["run"].strip() for step in run_steps(dashboard)]
if dashboard_runs.count(
    "./scripts/check-release-metadata.sh --candidate --artifact"
) != 1:
    raise AssertionError("dashboard PR job must invoke exact candidate artifact mode once")
if any("--tag" in command for command in dashboard_runs):
    raise AssertionError("dashboard PR job must not invoke tag mode")
if dashboard_runs.count("./scripts/test-release-metadata.sh") != 1:
    raise AssertionError("dashboard PR job must run hostile release metadata fixtures once")

if release.get("if") != "startsWith(github.ref, 'refs/tags/v')":
    raise AssertionError("release landing gate must be restricted to refs/tags/v")
release_steps = release.get("steps")
if not isinstance(release_steps, list):
    raise AssertionError("release landing gate steps must be a list")
checkout = [
    step
    for step in release_steps
    if isinstance(step, dict)
    and step.get("uses")
    == "actions/checkout@11bd71901bbe5b1630ceea73d27597364c9af683"
]
if len(checkout) != 1 or (checkout[0].get("with") or {}).get("fetch-depth") != 0:
    raise AssertionError("release landing gate must fetch full tag history exactly once")

release_runs = [step["run"].strip() for step in run_steps(release)]
expected_tag_gate = (
    './scripts/check-release-metadata.sh --tag "$GITHUB_REF_NAME" --artifact'
)
if release_runs.count(expected_tag_gate) != 1:
    raise AssertionError("release landing gate must invoke exact dynamic tag artifact mode once")
if any("--candidate" in command for command in release_runs):
    raise AssertionError("release landing gate must not invoke candidate mode")

for step in release_steps:
    if not isinstance(step, dict):
        raise AssertionError("release landing gate step must be a mapping")
    value = str(step.get("uses", "")).casefold()
    command = str(step.get("run", "")).casefold()
    if any(marker in value or marker in command for marker in (
        "deploy",
        "netlify",
        "upload-artifact",
        "release create",
        "gh release",
    )):
        raise AssertionError("release landing gate may validate only; publishing is forbidden")
    if "secrets." in json.dumps(step):
        raise AssertionError("release landing gate must not read secrets")

adapter_commands = "\n".join(step["run"] for step in run_steps(adapter_pins))
for required in (
    "./scripts/check-release-metadata.sh --candidate",
    "./scripts/check-release-workflow.sh",
    "./scripts/test-release-workflow.sh",
):
    if required not in adapter_commands:
        raise AssertionError(f"adapter-pins must run {required}")

print("OK: PR candidate and exact-tag landing gates are separated and fail closed")
PY
