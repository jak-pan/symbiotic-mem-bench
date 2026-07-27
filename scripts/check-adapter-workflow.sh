#!/usr/bin/env bash
# Static fail-closed guard for the native adapter setup in CI.
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
workflow="$repo_root/.github/workflows/ci.yml"

if [[ $# -gt 0 ]]; then
  if [[ $# -ne 2 || "$1" != "--workflow" ]]; then
    echo "usage: check-adapter-workflow.sh [--workflow PATH]" >&2
    exit 2
  fi
  workflow="$2"
fi

parsed="$(mktemp "${TMPDIR:-/tmp}/membench-workflow.XXXXXX")"
trap 'rm -f "$parsed"' EXIT

# Ruby's standard-library Psych parser is available on GitHub's Ubuntu images.
# Parse YAML before inspecting it so quoted keys, inline mappings, and other
# equivalent YAML spellings cannot evade the policy.
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
import re
import shlex
import sys
from pathlib import Path

workflow = json.loads(Path(sys.argv[1]).read_text())
jobs = workflow.get("jobs")
if not isinstance(jobs, dict):
    raise AssertionError("workflow jobs must be a mapping")

ADAPTER_FEATURE = "symbiotic-memory-adapter"
CACHE_EXECUTABLES = {"sccache", "cachepot", "cargo-cache"}
WRAPPER_ENV = {"RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER"}
ALLOWED_PROTECTED_ACTIONS = {
    "actions/checkout@v4",
    "dtolnay/rust-toolchain@1.93.0",
    "webfactory/ssh-agent@v0.9.0",
}
SHELL_CONTROL = {
    "!",
    "{",
    "}",
    "do",
    "done",
    "elif",
    "else",
    "fi",
    "if",
    "then",
    "while",
}


def steps(job_name, body):
    value = body.get("steps")
    if not isinstance(value, list):
        raise AssertionError(f"{job_name}: steps must be a list")
    for index, step in enumerate(value):
        if not isinstance(step, dict):
            raise AssertionError(f"{job_name}: step {index} must be a mapping")
    return value


def run_commands(job_name, body):
    commands = []
    for index, step in enumerate(steps(job_name, body)):
        if "run" not in step:
            continue
        command = step["run"]
        if not isinstance(command, str):
            raise AssertionError(f"{job_name}: step {index} run must be a string")
        commands.append((index, command))
    return commands


def shell_segments(command):
    command = re.sub(r"\\\s*\n\s*", " ", command)
    command = command.replace("\n", " ; ")
    lexer = shlex.shlex(command, posix=True, punctuation_chars=";&|")
    lexer.whitespace_split = True
    lexer.commenters = "#"
    words = list(lexer)
    segments = []
    current = []
    for word in words:
        if word in {";", "&&", "||", "|", "&"}:
            if current:
                segments.append(current)
                current = []
        else:
            current.append(word)
    if current:
        segments.append(current)
    return segments


def is_assignment(word):
    return re.match(r"^[A-Za-z_][A-Za-z0-9_]*=", word) is not None


def command_word(segment):
    index = 0
    while index < len(segment) and (
        is_assignment(segment[index]) or segment[index] in SHELL_CONTROL
    ):
        index += 1
    if index >= len(segment):
        return None
    if segment[index] in {"export", "local", "readonly", "declare", "typeset"}:
        return None
    if segment[index] == "env":
        index += 1
        while index < len(segment) and (
            segment[index].startswith("-") or is_assignment(segment[index])
        ):
            index += 1
    if index < len(segment) and segment[index] == "command":
        index += 1
        while index < len(segment) and segment[index].startswith("-"):
            index += 1
    return segment[index] if index < len(segment) else None


def is_cargo(word):
    return word == "cargo" or word.endswith("/cargo")


def feature_value_enables_adapter(value):
    return ADAPTER_FEATURE in re.split(r"[\s,]+", value)


def segment_enables_adapter(job_name, step_index, segment):
    executable = command_word(segment)
    if executable is not None and ("$" in executable or "`" in executable):
        raise AssertionError(
            f"{job_name}: step {step_index} uses a dynamic shell executable; "
            "adapter job classification must be static"
        )

    normalized = [word.removeprefix("./") for word in segment]
    if any(word.endswith("scripts/check-adapter-build.sh") for word in normalized):
        return True

    # Explicit adapter/all-feature text classifies the job even when a dynamic
    # executable precedes it. This prevents `$CARGO_BIN ... --features adapter`
    # from silently escaping the protected setup.
    if any(
        word == "--all-features"
        or word.startswith("--all-features=")
        or feature_value_enables_adapter(word.split("=", 1)[-1])
        for word in segment
    ):
        return True

    for cargo_index, word in enumerate(segment):
        if not is_cargo(word):
            continue
        args = segment[cargo_index + 1 :]
        if any("$" in arg or "`" in arg for arg in args):
            raise AssertionError(
                f"{job_name}: step {step_index} uses dynamic Cargo arguments; "
                "use explicit arguments"
            )
        index = 0
        while index < len(args):
            arg = args[index]
            if arg in {"--features", "-F"}:
                if index + 1 >= len(args):
                    raise AssertionError(
                        f"{job_name}: step {step_index} is missing a value for {arg}"
                    )
                if feature_value_enables_adapter(args[index + 1]):
                    return True
                index += 2
                continue
            if arg.startswith("--features=") and feature_value_enables_adapter(
                arg.split("=", 1)[1]
            ):
                return True
            if arg.startswith("-F=") and feature_value_enables_adapter(
                arg.split("=", 1)[1]
            ):
                return True
            if (
                arg.startswith("-F")
                and len(arg) > 2
                and feature_value_enables_adapter(arg[2:])
            ):
                return True
            index += 1
    return False


def body_enables_adapter(job_name, body):
    enabled = False
    for step_index, command in run_commands(job_name, body):
        for segment in shell_segments(command):
            if segment_enables_adapter(job_name, step_index, segment):
                enabled = True
    return enabled


def walk(value, path=()):
    yield path, value
    if isinstance(value, dict):
        for key, child in value.items():
            yield from walk(child, path + (str(key),))
    elif isinstance(value, list):
        for index, child in enumerate(value):
            yield from walk(child, path + (str(index),))


def assert_no_conditions(job_name, body):
    for forbidden in ("if", "continue-on-error"):
        if forbidden in body:
            raise AssertionError(
                f"{job_name}: protected adapter job must not set {forbidden}"
            )
    for index, step in enumerate(steps(job_name, body)):
        for forbidden in ("if", "continue-on-error"):
            if forbidden in step:
                raise AssertionError(
                    f"{job_name}: protected step {index} must not set {forbidden}"
                )


def assert_no_cache_or_artifacts(job_name, body):
    for path, value in walk(body):
        if path:
            key = path[-1]
            if "cache" in key.casefold():
                raise AssertionError(
                    f"{job_name}: adapter-enabled jobs must not configure cache key "
                    + ".".join(path)
                )
        if path and path[-1] == "uses" and isinstance(value, str):
            action = value.casefold()
            if "cache" in action or "artifact" in action:
                raise AssertionError(
                    f"{job_name}: adapter-enabled jobs must not use persistence action "
                    f"{value}"
                )
            if value not in ALLOWED_PROTECTED_ACTIONS:
                raise AssertionError(
                    f"{job_name}: protected adapter job uses unreviewed action {value}"
                )

    for scope_path, value in walk(body):
        if not scope_path or scope_path[-1] != "env" or not isinstance(value, dict):
            continue
        for key, env_value in value.items():
            normalized_key = str(key).upper()
            normalized_value = str(env_value).casefold()
            if (
                normalized_key in WRAPPER_ENV
                or normalized_key.endswith("_RUSTC_WRAPPER")
                or normalized_key.endswith("_RUSTC_WORKSPACE_WRAPPER")
            ) and str(env_value).strip():
                raise AssertionError(
                    f"{job_name}: adapter-enabled jobs must not set {normalized_key}"
                )
            if any(executable in normalized_value for executable in CACHE_EXECUTABLES):
                raise AssertionError(
                    f"{job_name}: adapter-enabled jobs must not configure cache wrapper "
                    f"{env_value}"
                )

    for step_index, command in run_commands(job_name, body):
        for segment in shell_segments(command):
            for word in segment:
                executable = word.rsplit("=", 1)[-1].rsplit("/", 1)[-1].casefold()
                if executable in CACHE_EXECUTABLES:
                    raise AssertionError(
                        f"{job_name}: step {step_index} must not invoke {executable}"
                    )


def assert_no_git_url_rewrites(job_name, body):
    rewrite = re.compile(r"\.(?:push)?insteadof(?:=|$)", re.IGNORECASE)
    for path, value in walk(body):
        if isinstance(value, str) and rewrite.search(value):
            raise AssertionError(
                f"{job_name}: Git URL rewrite is forbidden at " + ".".join(path)
            )

    for scope_path, value in walk(body):
        if not scope_path or scope_path[-1] != "env" or not isinstance(value, dict):
            continue
        for key in value:
            if str(key).upper().startswith("GIT_CONFIG"):
                raise AssertionError(
                    f"{job_name}: Git config injection env {key} is forbidden"
                )

    for step_index, command in run_commands(job_name, body):
        for segment in shell_segments(command):
            lowered = [word.casefold() for word in segment]
            git_indexes = [
                index
                for index, word in enumerate(lowered)
                if word == "git" or word.endswith("/git")
            ]
            for git_index in git_indexes:
                args = segment[git_index + 1 :]
                if any("$" in arg or "`" in arg for arg in args):
                    raise AssertionError(
                        f"{job_name}: step {step_index} uses dynamic Git arguments; "
                        "URL rewrite policy cannot verify them"
                    )


def exactly_one(job_name, description, candidates):
    if len(candidates) != 1:
        raise AssertionError(
            f"{job_name}: expected exactly one {description}; found {len(candidates)}"
        )
    return candidates[0]


def assert_protected_setup(job_name, body):
    if body.get("runs-on") != "ubuntu-latest":
        raise AssertionError(f"{job_name}: must run on ubuntu-latest")
    if body.get("timeout-minutes") != 180:
        raise AssertionError(f"{job_name}: must have a 180-minute cold-build timeout")
    assert_no_conditions(job_name, body)
    if "needs" in body or "strategy" in body:
        raise AssertionError(
            f"{job_name}: protected adapter job must not use needs or strategy"
        )
    assert_no_cache_or_artifacts(job_name, body)
    assert_no_git_url_rewrites(job_name, body)

    job_steps = steps(job_name, body)
    pin_index = exactly_one(
        job_name,
        "pin-binding step",
        [
            index
            for index, step in enumerate(job_steps)
            if "./scripts/check-adapter-pins.sh" in str(step.get("run", ""))
            and "$GITHUB_OUTPUT" in str(step.get("run", ""))
        ],
    )
    key_index = exactly_one(
        job_name,
        "deploy-key preflight",
        [
            index
            for index, step in enumerate(job_steps)
            if step.get("run") == 'test -n "$SYMBIOTIC_MEMORY_DEPLOY_KEY"'
        ],
    )
    agent_index = exactly_one(
        job_name,
        "SSH-agent step",
        [
            index
            for index, step in enumerate(job_steps)
            if step.get("uses") == "webfactory/ssh-agent@v0.9.0"
        ],
    )
    source_index = exactly_one(
        job_name,
        "canonical adapter checkout",
        [
            index
            for index, step in enumerate(job_steps)
            if step.get("uses") == "actions/checkout@v4"
            and isinstance(step.get("with"), dict)
            and step["with"].get("repository") == "symbiotic-sh/symbiotic-memory"
        ],
    )
    prepare_index = exactly_one(
        job_name,
        "zvec preparation step",
        [
            index
            for index, step in enumerate(job_steps)
            if "./scripts/prepare-adapter-zvec.sh" in str(step.get("run", ""))
        ],
    )
    if not pin_index < key_index < agent_index < source_index < prepare_index:
        raise AssertionError(
            f"{job_name}: required pin/key/agent/checkout/prepare step order changed"
        )

    pin_step = job_steps[pin_index]
    if pin_step.get("id") != "adapter_pin":
        raise AssertionError(f"{job_name}: pin-binding step id must be adapter_pin")
    pin_run = pin_step["run"]
    pin_check = pin_run.find("./scripts/check-adapter-pins.sh")
    pin_output = pin_run.find(
        'echo "sha=$(tr -d \'\\n\' < .symbiotic-memory-pin)" >> "$GITHUB_OUTPUT"'
    )
    if pin_check < 0 or pin_output <= pin_check:
        raise AssertionError(f"{job_name}: pin must be checked before it is exported")

    key_env = job_steps[key_index].get("env")
    if not isinstance(key_env, dict) or key_env.get(
        "SYMBIOTIC_MEMORY_DEPLOY_KEY"
    ) != "${{ secrets.SYMBIOTIC_MEMORY_DEPLOY_KEY }}":
        raise AssertionError(f"{job_name}: key preflight must use the scoped secret")

    agent_with = job_steps[agent_index].get("with")
    if not isinstance(agent_with, dict) or agent_with.get("ssh-private-key") != (
        "${{ secrets.SYMBIOTIC_MEMORY_DEPLOY_KEY }}"
    ):
        raise AssertionError(f"{job_name}: SSH agent must use the scoped deploy key")

    source_with = job_steps[source_index]["with"]
    required_source = {
        "ref": "${{ steps.adapter_pin.outputs.sha }}",
        "path": ".adapter-source/symbiotic-memory",
        "ssh-key": "${{ secrets.SYMBIOTIC_MEMORY_DEPLOY_KEY }}",
        "persist-credentials": False,
    }
    for key, expected in required_source.items():
        if source_with.get(key) != expected:
            raise AssertionError(
                f"{job_name}: canonical checkout {key} must be {expected!r}"
            )

    prepare_run = job_steps[prepare_index]["run"]
    jobs_export = prepare_run.find('export ZVEC_BUILD_JOBS="$(nproc)"')
    wrapper = prepare_run.find("./scripts/prepare-adapter-zvec.sh")
    target = prepare_run.find("x86_64-unknown-linux-gnu")
    zvec_export = prepare_run.find('echo "ZVEC_LIB_DIR=$zvec_dir"')
    library_export = prepare_run.find('echo "LIBRARY_PATH=$zvec_dir')
    runtime_export = prepare_run.find('echo "LD_LIBRARY_PATH=$zvec_dir')
    github_env = prepare_run.find('>> "$GITHUB_ENV"')
    positions = (
        jobs_export,
        wrapper,
        target,
        zvec_export,
        library_export,
        runtime_export,
        github_env,
    )
    if any(position < 0 for position in positions) or list(positions) != sorted(
        positions
    ):
        raise AssertionError(
            f"{job_name}: zvec jobs/prepare/target/export sequence must remain exact"
        )

    cargo_steps = []
    for step_index, command in run_commands(job_name, body):
        for segment in shell_segments(command):
            if any(is_cargo(word) for word in segment) or any(
                word.removeprefix("./").endswith("scripts/check-adapter-build.sh")
                for word in segment
            ):
                cargo_steps.append(step_index)
    if cargo_steps and min(cargo_steps) <= prepare_index:
        raise AssertionError(
            f"{job_name}: Cargo runs before verified zvec preparation and export"
        )


for job_name, body in jobs.items():
    if not isinstance(body, dict):
        raise AssertionError(f"{job_name}: job must be a mapping")
    if "uses" in body:
        raise AssertionError(
            f"{job_name}: reusable workflow jobs are unsupported by adapter classification"
        )

adapter_feature_jobs = {
    name for name, body in jobs.items() if body_enables_adapter(name, body)
}
if adapter_feature_jobs != {"rust", "adapter-build"}:
    raise AssertionError(
        "every adapter-enabled Cargo/script job must use the protected native setup; "
        "found " + ", ".join(sorted(adapter_feature_jobs))
    )

for name in sorted(adapter_feature_jobs):
    assert_protected_setup(name, jobs[name])

if "adapter-key" in jobs:
    raise AssertionError("adapter build must fail on a missing key, not be skipped")

print("OK: every adapter-enabled Ubuntu job has fail-closed pinned zvec preparation")
PY
