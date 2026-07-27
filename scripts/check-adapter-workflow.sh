#!/usr/bin/env bash
# Static fail-closed guard for the native adapter setup in CI.
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"

python3 - "$repo_root/.github/workflows/ci.yml" <<'PY'
import re
import shlex
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


def run_commands(body):
    lines = body.splitlines()
    commands = []
    index = 0
    while index < len(lines):
        match = re.match(
            r"^(?P<indent>\s+)(?:-\s+)?run:\s*(?P<value>.*)$",
            lines[index],
        )
        if not match:
            index += 1
            continue

        value = match.group("value").strip()
        if value not in {"|", "|-", "|+", ">", ">-", ">+"}:
            commands.append(value)
            index += 1
            continue

        base_indent = len(match.group("indent"))
        block = []
        index += 1
        while index < len(lines):
            line = lines[index]
            indentation = len(line) - len(line.lstrip())
            if line.strip() and indentation <= base_indent:
                break
            block.append(line)
            index += 1

        nonempty_indents = [
            len(line) - len(line.lstrip()) for line in block if line.strip()
        ]
        trim = min(nonempty_indents, default=0)
        commands.append("\n".join(line[trim:] for line in block))

    return commands


def shell_segments(command):
    command = re.sub(r"\\\s*\n\s*", " ", command)
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


def feature_value_enables_adapter(value):
    if "$" in value or "`" in value:
        raise AssertionError(
            "dynamic Cargo feature value can enable symbiotic-memory-adapter; "
            "use an explicit feature list"
        )
    return "symbiotic-memory-adapter" in re.split(r"[\s,]+", value)


def segment_enables_adapter(segment):
    normalized = [word.removeprefix("./") for word in segment]
    if any(word.endswith("scripts/check-adapter-build.sh") for word in normalized):
        return True

    for cargo_index, word in enumerate(segment):
        if word != "cargo" and not word.endswith("/cargo"):
            continue
        args = segment[cargo_index + 1 :]
        if any("$" in arg or "`" in arg for arg in args):
            raise AssertionError(
                "dynamic Cargo arguments can enable symbiotic-memory-adapter; "
                "use explicit arguments"
            )
        index = 0
        while index < len(args):
            arg = args[index]
            if arg == "--all-features":
                return True
            if arg.startswith("--all-features="):
                raise AssertionError(f"ambiguous Cargo all-features argument: {arg}")
            if arg in {"--features", "-F"}:
                if index + 1 >= len(args):
                    raise AssertionError(f"missing value for Cargo argument {arg}")
                if feature_value_enables_adapter(args[index + 1]):
                    return True
                index += 2
                continue
            if arg.startswith("--features="):
                if feature_value_enables_adapter(arg.split("=", 1)[1]):
                    return True
            elif arg.startswith("-F="):
                if feature_value_enables_adapter(arg.split("=", 1)[1]):
                    return True
            elif arg.startswith("-F") and len(arg) > 2:
                if feature_value_enables_adapter(arg[2:]):
                    return True
            index += 1
    return False


def body_enables_adapter(body):
    return any(
        segment_enables_adapter(segment)
        for command in run_commands(body)
        for segment in shell_segments(command)
    )


def assert_no_cache(name, body):
    for action in re.findall(
        r"^\s*(?:-\s+)?uses:\s*[\"']?([^\"'\s#]+)",
        body,
        re.MULTILINE | re.IGNORECASE,
    ):
        if "cache" in action.casefold():
            raise AssertionError(
                f"{name}: adapter-enabled jobs must not use cache action {action}"
            )

    cache_key = re.search(
        r"^\s+[A-Za-z0-9_-]*cache[A-Za-z0-9_-]*\s*:",
        body,
        re.MULTILINE | re.IGNORECASE,
    )
    if cache_key:
        raise AssertionError(
            f"{name}: adapter-enabled jobs must not configure workflow caches: "
            f"{cache_key.group(0).strip()}"
        )

    for command in run_commands(body):
        for segment in shell_segments(command):
            for word in segment:
                executable = word.rsplit("=", 1)[-1].rsplit("/", 1)[-1].casefold()
                if executable in {"sccache", "cachepot", "cargo-cache"}:
                    raise AssertionError(
                        f"{name}: adapter-enabled jobs must not invoke {executable}"
                    )


def assert_no_git_url_rewrites(name, body):
    for command in run_commands(body):
        for segment in shell_segments(command):
            lowered = [word.casefold() for word in segment]
            executables = [word.rsplit("/", 1)[-1] for word in lowered]
            if "git" not in executables or "config" not in lowered:
                continue
            for word in lowered:
                key = word.split("=", 1)[0]
                if key.endswith(".insteadof") or key.endswith(".pushinsteadof"):
                    raise AssertionError(
                        f"{name}: Git URL rewrite key {key} is forbidden"
                    )


def fixture_body(command):
    return f"      - run: |\n          {command}\n"


for fixture in (
    "cargo test --all-features",
    "cargo test --features=symbiotic-memory-adapter",
    "cargo test -F symbiotic-memory-adapter",
    "cargo test -Fsymbiotic-memory-adapter",
    "cargo test --features=cli,symbiotic-memory-adapter",
    'cargo test --features "cli symbiotic-memory-adapter"',
    "./scripts/check-adapter-build.sh",
):
    if not body_enables_adapter(fixture_body(fixture)):
        raise AssertionError(f"adapter detector self-test missed: {fixture}")

for fixture in (
    "cargo test",
    "cargo test --features cli",
    "./scripts/check-adapter-pins.sh",
):
    if body_enables_adapter(fixture_body(fixture)):
        raise AssertionError(f"adapter detector self-test false positive: {fixture}")

for fixture in (
    'cargo test --features "$FEATURES"',
    'cargo test "$CARGO_FLAGS"',
):
    try:
        body_enables_adapter(fixture_body(fixture))
    except AssertionError:
        pass
    else:
        raise AssertionError(f"adapter detector must fail closed for: {fixture}")

for fixture in (
    "      - uses: actions/cache@v4\n"
    "        with:\n"
    "          path: |\n"
    "            target\n"
    "            .adapter-source\n"
    "            $RUNNER_TEMP/symbiotic-memory-zvec\n",
    "      - uses: Swatinem/rust-cache@v2\n",
    "      - uses: mozilla-actions/sccache-action@v0.0.9\n",
    "      - uses: actions/setup-node@v4\n        with:\n          cache: npm\n",
    "      - run: sccache --start-server\n",
    "      - run: RUSTC_WRAPPER=sccache cargo test\n",
):
    try:
        assert_no_cache("fixture", fixture)
    except AssertionError:
        pass
    else:
        raise AssertionError("cache guard self-test missed a forbidden cache form")

for key in (
    "url.ssh://git@github.com/.insteadOf",
    "url.ssh://git@github.com/.INSTEADOF",
    "url.ssh://git@github.com/.pushInsteadOf",
    "url.ssh://git@github.com/.InsteadOf=https://github.com/",
):
    try:
        assert_no_git_url_rewrites(
            "fixture",
            fixture_body(f"/usr/bin/git config --global {key} https://github.com/"),
        )
    except AssertionError:
        pass
    else:
        raise AssertionError(f"Git rewrite guard self-test missed key: {key}")


jobs = {
    match.group(1): job(match.group(1))
    for match in re.finditer(r"^  ([a-zA-Z0-9_-]+):\n", workflow, re.MULTILINE)
}
adapter_feature_jobs = {
    name for name, body in jobs.items() if body_enables_adapter(body)
}
if adapter_feature_jobs != {"rust", "adapter-build"}:
    raise AssertionError(
        "every adapter-enabled Cargo/script job must use the protected native setup; "
        "found " + ", ".join(sorted(adapter_feature_jobs))
    )

for name in sorted(adapter_feature_jobs):
    body = jobs[name]
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
        "runner-core zvec build parallelism": 'export ZVEC_BUILD_JOBS="$(nproc)"',
        "target match": "x86_64-unknown-linux-gnu",
        "zvec build path": 'echo "ZVEC_LIB_DIR=$zvec_dir"',
        "linker path": 'echo "LIBRARY_PATH=$zvec_dir',
        "runtime linker path": 'echo "LD_LIBRARY_PATH=$zvec_dir',
        "cross-step export": '>> "$GITHUB_ENV"',
    }
    for description, needle in required.items():
        if needle not in body:
            raise AssertionError(f"{name}: missing {description}")
    assert_no_cache(name, body)
    assert_no_git_url_rewrites(name, body)
    first_cargo = body.find("cargo ")
    prepare = body.find("./scripts/prepare-adapter-zvec.sh")
    jobs_export = body.find('export ZVEC_BUILD_JOBS="$(nproc)"')
    if jobs_export > prepare:
        raise AssertionError(f"{name}: ZVEC_BUILD_JOBS must be set before zvec preparation")
    if first_cargo != -1 and first_cargo < prepare:
        raise AssertionError(f"{name}: Cargo runs before verified zvec preparation")

if "\n  adapter-key:" in workflow:
    raise AssertionError("adapter build must fail on a missing key, not be conditionally skipped")

print("OK: every adapter-enabled Ubuntu job has fail-closed pinned zvec preparation")
PY
