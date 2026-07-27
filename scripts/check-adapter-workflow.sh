#!/usr/bin/env bash
# Static fail-closed guard for the native adapter setup in CI.
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
workflow="$repo_root/.github/workflows/ci.yml"
workflow_root="$repo_root/.github/workflows"
inventory=true

if [[ $# -gt 0 ]]; then
  if [[ $# -ne 2 ]]; then
    echo "usage: check-adapter-workflow.sh [--workflow PATH | --workflow-root DIR]" >&2
    exit 2
  fi
  case "$1" in
    --workflow)
      workflow="$2"
      inventory=false
      ;;
    --workflow-root)
      workflow_root="$2"
      workflow="$workflow_root/ci.yml"
      ;;
    *)
      echo "usage: check-adapter-workflow.sh [--workflow PATH | --workflow-root DIR]" >&2
      exit 2
      ;;
  esac
fi

if [[ "$inventory" == true ]]; then
  workflow_files=""
  while IFS= read -r candidate; do
    workflow_files="${workflow_files}${candidate#$workflow_root/}"$'\n'
  done < <(find "$workflow_root" -maxdepth 1 -type f \( -name '*.yml' -o -name '*.yaml' \) | sort)
  if [[ "$workflow_files" != $'ci.yml\n' ]]; then
    echo "FAIL: workflow inventory must contain exactly ci.yml; review every new workflow before it can run adapter code" >&2
    printf '%s' "$workflow_files" >&2
    exit 1
  fi
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
CACHE_EXECUTABLES = {"ccache", "sccache", "cachepot", "cargo-cache"}
WRAPPER_ENV = {"RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER"}
SHELL_WRAPPERS = {"bash", "dash", "ksh", "sh", "zsh"}
COMMAND_COMPOSERS = {"eval", "exec"}
OPAQUE_INTERPRETERS = {"deno", "node", "perl", "python", "python2", "python3", "ruby"}
ALLOWED_PROTECTED_ACTIONS = {
    "actions/checkout@11bd71901bbe5b1630ceea73d27597364c9af683",
    "dtolnay/rust-toolchain@d0befba8b9ddf874327619e84c39b094edd58b66",
    "webfactory/ssh-agent@dc588b651fe13675774614f8e6a936a468676387",
}
IMMUTABLE_ACTION = re.compile(r"^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+@[0-9a-f]{40}$")
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
    "until",
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


def shell_tokens(command):
    command = re.sub(r"\\\s*\n\s*", " ", command)
    command = command.replace("\n", " ; ")
    lexer = shlex.shlex(command, posix=True, punctuation_chars=";&|")
    lexer.whitespace_split = True
    lexer.commenters = "#"
    return list(lexer)


def shell_segments(command):
    words = shell_tokens(command)
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


def command_position(segment):
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
    return index if index < len(segment) else None


def command_word(segment):
    index = command_position(segment)
    return segment[index] if index is not None else None


def executable_name(word):
    return word.rsplit("/", 1)[-1].casefold()


def word_uses_cache(word):
    components = re.split(r"[/=]", word.casefold())
    return any(component in CACHE_EXECUTABLES for component in components)


def nested_shell_payloads(job_name, step_index, segment):
    payloads = []
    executable_index = command_position(segment)
    for index, word in enumerate(segment):
        name = executable_name(word)
        if name in SHELL_WRAPPERS:
            args = segment[index + 1 :]
            payload = None
            for option_index, arg in enumerate(args):
                if arg == "--":
                    break
                if arg == "-c" or (
                    arg.startswith("-")
                    and not arg.startswith("--")
                    and "c" in arg[1:]
                ):
                    if option_index + 1 >= len(args):
                        raise AssertionError(
                            f"{job_name}: step {step_index} shell wrapper {word} "
                            "is missing its -c payload"
                        )
                    payload = args[option_index + 1]
                    break
            if payload is None and index == executable_index:
                raise AssertionError(
                    f"{job_name}: step {step_index} invokes shell wrapper {word} "
                    "without a statically inspectable -c payload"
                )
            if payload is not None:
                payloads.append((name, payload))
        elif name in COMMAND_COMPOSERS and index == executable_index:
            args = segment[index + 1 :]
            if not args:
                raise AssertionError(
                    f"{job_name}: step {step_index} invokes {name} without a payload"
                )
            payloads.append((name, " ".join(args)))
        elif name in {".", "source"} and index == executable_index:
            raise AssertionError(
                f"{job_name}: step {step_index} sources an uninspectable shell payload"
            )
        elif name in OPAQUE_INTERPRETERS and index == executable_index:
            args = segment[index + 1 :]
            if not (
                name.startswith("python")
                and len(args) == 2
                and args == ["-m", "json.tool"]
            ):
                raise AssertionError(
                    f"{job_name}: step {step_index} invokes opaque interpreter {word}; "
                    "adapter classification cannot inspect it"
                )
    return payloads


def recursive_shell_commands(job_name, step_index, command, depth=0):
    if depth > 8:
        raise AssertionError(
            f"{job_name}: step {step_index} exceeds nested shell inspection depth"
        )
    yield depth, command
    for segment in shell_segments(command):
        for _, payload in nested_shell_payloads(job_name, step_index, segment):
            yield from recursive_shell_commands(
                job_name, step_index, payload, depth=depth + 1
            )


def recursive_shell_segments(job_name, step_index, command):
    for depth, nested_command in recursive_shell_commands(
        job_name, step_index, command
    ):
        for segment in shell_segments(nested_command):
            yield depth, segment


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
                    if "--locked" not in args:
                        raise AssertionError(
                            f"{job_name}: step {step_index} adapter Cargo command "
                            "must use --locked"
                        )
                    return True
                index += 2
                continue
            if arg.startswith("--features=") and feature_value_enables_adapter(
                arg.split("=", 1)[1]
            ):
                if "--locked" not in args:
                    raise AssertionError(
                        f"{job_name}: step {step_index} adapter Cargo command "
                        "must use --locked"
                    )
                return True
            if arg.startswith("-F=") and feature_value_enables_adapter(
                arg.split("=", 1)[1]
            ):
                if "--locked" not in args:
                    raise AssertionError(
                        f"{job_name}: step {step_index} adapter Cargo command "
                        "must use --locked"
                    )
                return True
            if (
                arg.startswith("-F")
                and len(arg) > 2
                and feature_value_enables_adapter(arg[2:])
            ):
                if "--locked" not in args:
                    raise AssertionError(
                        f"{job_name}: step {step_index} adapter Cargo command "
                        "must use --locked"
                    )
                return True
            if arg == "--all-features" or arg.startswith("--all-features="):
                if "--locked" not in args:
                    raise AssertionError(
                        f"{job_name}: step {step_index} all-feature Cargo command "
                        "must use --locked"
                    )
                return True
            index += 1
    return False


def body_enables_adapter(job_name, body):
    enabled = False
    for step_index, command in run_commands(job_name, body):
        statically_allowed = command.replace("$(nproc)", "").replace(
            "$(tr -d '\\n' < .symbiotic-memory-pin)", ""
        )
        if "$(" in statically_allowed or "`" in statically_allowed:
            raise AssertionError(
                f"{job_name}: step {step_index} contains an unreviewed command substitution"
            )
        if "<(" in command or ">(" in command:
            allowed_process_substitution = (
                "cargo run --bin membench-leaderboard -- export "
                "--records-root canary/records --deterministic > /tmp/canary-out.json\n"
                "diff <(python3 -m json.tool /tmp/canary-out.json) "
                "<(python3 -m json.tool canary/expected-leaderboard.json)"
            )
            if command.strip() != allowed_process_substitution:
                raise AssertionError(
                    f"{job_name}: step {step_index} contains unreviewed process substitution"
                )
        for _, segment in recursive_shell_segments(job_name, step_index, command):
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


def assert_global_policy():
    for path, value in walk(workflow):
        if path and path[-1] == "uses" and isinstance(value, str):
            if not value.startswith("./") and IMMUTABLE_ACTION.fullmatch(value) is None:
                raise AssertionError(
                    "every workflow action must use an immutable 40-character SHA at "
                    + ".".join(path)
                )
        if path and path[-1] == "shell":
            raise AssertionError(
                "workflow and job defaults.run.shell are forbidden at "
                + ".".join(path)
            )
        if not path or path[-1] != "env" or not isinstance(value, dict):
            continue
        for key, env_value in value.items():
            normalized_key = str(key).upper()
            normalized_value = str(env_value).casefold()
            native_compiler_launcher = re.fullmatch(
                r"CMAKE_[A-Z0-9_]+_(?:COMPILER|LINKER)_LAUNCHER",
                normalized_key,
            )
            if normalized_key.startswith("GIT_CONFIG") or normalized_key in {
                "GIT_DIR",
                "GIT_WORK_TREE",
                "GIT_COMMON_DIR",
                "GIT_INDEX_FILE",
                "GIT_SHALLOW_FILE",
                "GIT_NAMESPACE",
                "GIT_REPLACE_REF_BASE",
                "GIT_OBJECT_DIRECTORY",
                "GIT_ALTERNATE_OBJECT_DIRECTORIES",
            }:
                raise AssertionError(
                    f"workflow Git identity injection env {key} is forbidden"
                )
            if (
                normalized_key in WRAPPER_ENV
                or normalized_key.endswith("_RUSTC_WRAPPER")
                or normalized_key.endswith("_RUSTC_WORKSPACE_WRAPPER")
                or native_compiler_launcher is not None
            ) and str(env_value).strip():
                raise AssertionError(
                    f"workflow compiler wrapper env {normalized_key} is forbidden"
                )
            if (
                normalized_key in {"CC", "CXX", "AR", "LD"}
                and "cache" in normalized_value
            ):
                raise AssertionError(
                    f"workflow compiler env {normalized_key} selects a cache executable"
                )


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
            native_compiler_launcher = re.fullmatch(
                r"CMAKE_[A-Z0-9_]+_(?:COMPILER|LINKER)_LAUNCHER",
                normalized_key,
            )
            if (
                normalized_key in WRAPPER_ENV
                or normalized_key.endswith("_RUSTC_WRAPPER")
                or normalized_key.endswith("_RUSTC_WORKSPACE_WRAPPER")
                or native_compiler_launcher is not None
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
        for _, segment in recursive_shell_segments(job_name, step_index, command):
            for word in segment:
                if word_uses_cache(word):
                    raise AssertionError(
                        f"{job_name}: step {step_index} must not invoke cache tool {word}"
                    )


def assert_no_git_url_rewrites(job_name, body):
    rewrite = re.compile(r"\b(?:push)?insteadof\b", re.IGNORECASE)
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
        for _, segment in recursive_shell_segments(job_name, step_index, command):
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


def controlled_segment(segment):
    normalized = [word.removeprefix("./") for word in segment]
    lowered = [word.casefold() for word in normalized]
    if any(is_cargo(word) for word in segment):
        return True
    if any(word == "git" or word.endswith("/git") for word in lowered):
        return True
    if any(
        word.endswith("scripts/check-adapter-build.sh")
        or word.endswith("scripts/check-adapter-pins.sh")
        or word.endswith("scripts/prepare-adapter-zvec.sh")
        for word in normalized
    ):
        return True
    if any(word_uses_cache(word) for word in segment):
        return True
    return any(
        marker in word
        for marker in (
            "GITHUB_ENV",
            "GITHUB_OUTPUT",
            "ZVEC_LIB_DIR",
            "LIBRARY_PATH",
            "LD_LIBRARY_PATH",
        )
        for word in segment
    )


def assert_no_failure_masking(job_name, body):
    for path, _ in walk(body):
        if path and path[-1] == "shell":
            raise AssertionError(
                f"{job_name}: protected adapter jobs must not override shell failure semantics"
            )
    for step_index, command in run_commands(job_name, body):
        for depth, nested_command in recursive_shell_commands(
            job_name, step_index, command
        ):
            tokens = shell_tokens(nested_command)
            segments = shell_segments(nested_command)
            has_controlled_command = any(
                controlled_segment(segment) for segment in segments
            )
            if "||" in tokens:
                raise AssertionError(
                    f"{job_name}: step {step_index} must not mask failures with ||"
                )
            if "&" in tokens and has_controlled_command:
                raise AssertionError(
                    f"{job_name}: step {step_index} must not background mandatory commands"
                )
            for segment in segments:
                executable = command_word(segment)
                name = executable_name(executable) if executable is not None else None
                if name == "set":
                    args = segment[command_position(segment) + 1 :]
                    if "+e" in args or (
                        "+o" in args
                        and any(arg.casefold() == "errexit" for arg in args)
                    ):
                        raise AssertionError(
                            f"{job_name}: step {step_index} must not disable errexit"
                        )
                if name == "trap":
                    raise AssertionError(
                        f"{job_name}: step {step_index} must not install shell traps"
                    )
                first_non_assignment = next(
                    (word for word in segment if not is_assignment(word)), None
                )
                if first_non_assignment == "!" and controlled_segment(segment):
                    raise AssertionError(
                        f"{job_name}: step {step_index} negates a mandatory command"
                    )
                if (
                    first_non_assignment in {"if", "until", "while"}
                    and controlled_segment(segment)
                ):
                    raise AssertionError(
                        f"{job_name}: step {step_index} conditionally masks "
                        "a mandatory command"
                    )
                if depth > 0 and controlled_segment(segment):
                    raise AssertionError(
                        f"{job_name}: step {step_index} hides a controlled command "
                        "inside a shell/eval wrapper"
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
    assert_no_failure_masking(job_name, body)

    job_env = body.get("env")
    if not isinstance(job_env, dict) or job_env.get("GIT_NO_REPLACE_OBJECTS") != "1":
        raise AssertionError(
            f"{job_name}: must propagate GIT_NO_REPLACE_OBJECTS=1"
        )

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
            if step.get("uses")
            == "webfactory/ssh-agent@dc588b651fe13675774614f8e6a936a468676387"
        ],
    )
    source_index = exactly_one(
        job_name,
        "canonical adapter checkout",
        [
            index
            for index, step in enumerate(job_steps)
            if step.get("uses")
            == "actions/checkout@11bd71901bbe5b1630ceea73d27597364c9af683"
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
    expected_pin_run = """./scripts/check-adapter-pins.sh
echo "sha=$(tr -d '\\n' < .symbiotic-memory-pin)" >> "$GITHUB_OUTPUT\""""
    if pin_step["run"].strip() != expected_pin_run:
        raise AssertionError(f"{job_name}: pin binding command must remain exact")

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

    expected_prepare_run = """export ZVEC_BUILD_JOBS="$(nproc)"
zvec_dir="$RUNNER_TEMP/symbiotic-memory-zvec"
./scripts/prepare-adapter-zvec.sh \\
  .adapter-source/symbiotic-memory \\
  "$zvec_dir" \\
  x86_64-unknown-linux-gnu
{
  echo "ZVEC_LIB_DIR=$zvec_dir"
  echo "LIBRARY_PATH=$zvec_dir${LIBRARY_PATH:+:$LIBRARY_PATH}"
  echo "LD_LIBRARY_PATH=$zvec_dir${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
} >> "$GITHUB_ENV\""""
    if job_steps[prepare_index]["run"].strip() != expected_prepare_run:
        raise AssertionError(
            f"{job_name}: verified zvec preparation and GITHUB_ENV export must remain exact"
        )

    for step_index, step in enumerate(job_steps):
        command = str(step.get("run", ""))
        if "$GITHUB_ENV" in command and step_index != prepare_index:
            raise AssertionError(
                f"{job_name}: step {step_index} injects unreviewed GITHUB_ENV state"
            )
        if "$GITHUB_OUTPUT" in command and step_index != pin_index:
            raise AssertionError(
                f"{job_name}: step {step_index} injects unreviewed GITHUB_OUTPUT state"
            )

    cargo_steps = []
    for step_index, command in run_commands(job_name, body):
        for _, segment in recursive_shell_segments(job_name, step_index, command):
            if any(is_cargo(word) for word in segment) or any(
                word.removeprefix("./").endswith("scripts/check-adapter-build.sh")
                for word in segment
            ):
                cargo_steps.append(step_index)
    if cargo_steps and min(cargo_steps) <= prepare_index:
        raise AssertionError(
            f"{job_name}: Cargo runs before verified zvec preparation and export"
        )


assert_global_policy()

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
