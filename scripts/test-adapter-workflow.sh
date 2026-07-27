#!/usr/bin/env bash
# File-backed hostile regression fixtures for check-adapter-workflow.sh.
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
checker="$repo_root/scripts/check-adapter-workflow.sh"
fixture_root="$(mktemp -d "${TMPDIR:-/tmp}/membench-workflow-fixtures.XXXXXX")"
trap 'rm -rf "$fixture_root"' EXIT

new_fixture() {
  local name="$1"
  local directory="$fixture_root/$name"
  mkdir -p "$directory"
  cp "$repo_root/.github/workflows/ci.yml" "$directory/ci.yml"
  echo "$directory/ci.yml"
}

replace_once() {
  local path="$1"
  local needle="$2"
  local replacement="$3"
  python3 - "$path" "$needle" "$replacement" <<'PY'
import sys
from pathlib import Path

path = Path(sys.argv[1])
needle = sys.argv[2]
replacement = sys.argv[3]
text = path.read_text()
if needle not in text:
    raise SystemExit("fixture mutation needle was not found")
path.write_text(text.replace(needle, replacement, 1))
PY
}

expect_failure() {
  local name="$1"
  local path="$2"
  local expected="${3:-}"
  if "$checker" --workflow "$path" > "$fixture_root/$name.out" 2>&1; then
    echo "FAIL: $name workflow fixture unexpectedly passed" >&2
    exit 1
  fi
  if [[ -n "$expected" ]] && ! grep -Fq -- "$expected" "$fixture_root/$name.out"; then
    echo "FAIL: $name fixture failed for the wrong reason" >&2
    cat "$fixture_root/$name.out" >&2
    exit 1
  fi
  echo "OK: $name workflow fixture rejected"
}

"$checker"

path="$(new_fixture job-if)"
replace_once "$path" $'  rust:\n' $'  rust:\n    if: false\n'
expect_failure job-if "$path"

path="$(new_fixture job-continue-on-error)"
replace_once "$path" $'  rust:\n' $'  rust:\n    continue-on-error: true\n'
expect_failure job-continue-on-error "$path"

path="$(new_fixture job-needs)"
replace_once "$path" $'  rust:\n' $'  rust:\n    needs: dashboard\n'
expect_failure job-needs "$path"

path="$(new_fixture step-if)"
replace_once \
  "$path" \
  $'      - name: prepare and verify target-matched zvec\n        run: |\n' \
  $'      - name: prepare and verify target-matched zvec\n        if: false\n        run: |\n'
expect_failure step-if "$path"

path="$(new_fixture step-continue-on-error)"
replace_once \
  "$path" \
  $'      - name: prepare and verify target-matched zvec\n        run: |\n' \
  $'      - name: prepare and verify target-matched zvec\n        continue-on-error: true\n        run: |\n'
expect_failure step-continue-on-error "$path"

path="$(new_fixture dynamic-cargo-executable)"
replace_once \
  "$path" \
  $'      - run: npm run build\n\n  deps:\n' \
  $'      - run: npm run build\n      - run: $CARGO_BIN test --features symbiotic-memory-adapter\n\n  deps:\n'
expect_failure dynamic-cargo-executable "$path"

path="$(new_fixture dynamic-cargo-arguments)"
replace_once \
  "$path" \
  $'      - name: fmt\n' \
  $'      - run: cargo test \"$CARGO_FLAGS\"\n      - name: fmt\n'
expect_failure dynamic-cargo-arguments "$path"

path="$(new_fixture quoted-cache-key)"
replace_once \
  "$path" \
  $'      - name: fmt\n' \
  $'      - name: quoted cache\n        env:\n          \"cache\": target\n        run: true\n      - name: fmt\n'
expect_failure quoted-cache-key "$path"

path="$(new_fixture inline-cache-key)"
replace_once \
  "$path" \
  $'      - name: fmt\n' \
  $'      - name: inline cache\n        env: { cache: target }\n        run: true\n      - name: fmt\n'
expect_failure inline-cache-key "$path"

path="$(new_fixture artifact-action)"
replace_once \
  "$path" \
  $'      - name: fmt\n' \
  $'      - uses: actions/upload-artifact@v4\n        with:\n          name: private-adapter-derived\n          path: .adapter-source\n      - name: fmt\n'
expect_failure artifact-action "$path"

path="$(new_fixture unreviewed-action)"
replace_once \
  "$path" \
  $'      - name: fmt\n' \
  $'      - uses: example/persistence@v1\n      - name: fmt\n'
expect_failure unreviewed-action "$path"

path="$(new_fixture wrapper-env)"
replace_once \
  "$path" \
  $'      CARGO_NET_GIT_FETCH_WITH_CLI: \"true\"\n' \
  $'      CARGO_NET_GIT_FETCH_WITH_CLI: \"true\"\n      RUSTC_WRAPPER: sccache\n'
expect_failure wrapper-env "$path"

path="$(new_fixture cargo-wrapper-env)"
replace_once \
  "$path" \
  $'      CARGO_NET_GIT_FETCH_WITH_CLI: \"true\"\n' \
  $'      CARGO_NET_GIT_FETCH_WITH_CLI: \"true\"\n      CARGO_BUILD_RUSTC_WRAPPER: /tmp/compiler-cache\n'
expect_failure cargo-wrapper-env "$path"

path="$(new_fixture git-dash-c-rewrite)"
replace_once \
  "$path" \
  $'      - name: fmt\n' \
  $'      - run: git -c url.ssh://git@github.com/.InStEaDoF=https://github.com/ status\n      - name: fmt\n'
expect_failure git-dash-c-rewrite "$path"

path="$(new_fixture dynamic-git-config)"
replace_once \
  "$path" \
  $'      - name: fmt\n' \
  $'      - run: git config --global \"$CONFIG_KEY\" https://github.com/\n      - name: fmt\n'
expect_failure dynamic-git-config "$path"

path="$(new_fixture git-config-env)"
replace_once \
  "$path" \
  $'      CARGO_NET_GIT_FETCH_WITH_CLI: \"true\"\n' \
  $'      CARGO_NET_GIT_FETCH_WITH_CLI: \"true\"\n      GIT_CONFIG_KEY_0: ${{ vars.CONFIG_KEY }}\n'
expect_failure git-config-env "$path"

path="$(new_fixture nested-shell-cargo)"
replace_once \
  "$path" \
  $'      - run: npm run build\n\n  deps:\n' \
  $'      - run: npm run build\n      - run: bash -c \'cargo test --features symbiotic-memory-adapter\'\n\n  deps:\n'
expect_failure \
  nested-shell-cargo "$path" \
  "every adapter-enabled Cargo/script job must use the protected native setup"

path="$(new_fixture nested-protected-cargo)"
replace_once \
  "$path" \
  $'      - name: fmt\n' \
  $'      - run: bash -c \'cargo test --features symbiotic-memory-adapter\'\n      - name: fmt\n'
expect_failure nested-protected-cargo "$path" "hides a controlled command"

path="$(new_fixture recursive-shell-cache)"
replace_once \
  "$path" \
  $'      - name: fmt\n' \
  $'      - run: bash -c \"sh -c \'sccache --start-server\'\"\n      - name: fmt\n'
expect_failure recursive-shell-cache "$path" "must not invoke cache tool sccache"

path="$(new_fixture nested-shell-git)"
replace_once \
  "$path" \
  $'      - name: fmt\n' \
  $'      - run: bash -c \'git config --global \"$CONFIG_KEY\" https://github.com/\'\n      - name: fmt\n'
expect_failure nested-shell-git "$path" "uses dynamic Git arguments"

path="$(new_fixture dynamic-shell-payload)"
replace_once \
  "$path" \
  $'      - name: fmt\n' \
  $'      - run: bash -c \"$COMMAND\"\n      - name: fmt\n'
expect_failure dynamic-shell-payload "$path" "uses a dynamic shell executable"

path="$(new_fixture dynamic-eval-payload)"
replace_once \
  "$path" \
  $'      - name: fmt\n' \
  $'      - run: eval \"$COMMAND\"\n      - name: fmt\n'
expect_failure dynamic-eval-payload "$path" "uses a dynamic shell executable"

path="$(new_fixture masked-prepare)"
replace_once \
  "$path" \
  $'            x86_64-unknown-linux-gnu\n' \
  $'            x86_64-unknown-linux-gnu || true\n'
expect_failure masked-prepare "$path" "must not mask failures with ||"

path="$(new_fixture masked-export)"
replace_once \
  "$path" \
  $'          } >> \"$GITHUB_ENV\"\n' \
  $'          } >> \"$GITHUB_ENV\" || true\n'
expect_failure masked-export "$path" "must not mask failures with ||"

path="$(new_fixture masked-cargo)"
replace_once \
  "$path" \
  $'        run: cargo test --locked --features symbiotic-memory-adapter --lib --bin membench --test benchmark_v2\n' \
  $'        run: cargo test --locked --features symbiotic-memory-adapter --lib --bin membench --test benchmark_v2 || true\n'
expect_failure masked-cargo "$path" "must not mask failures with ||"

path="$(new_fixture conditional-cargo)"
replace_once \
  "$path" \
  $'        run: cargo test --locked --features symbiotic-memory-adapter --lib --bin membench --test benchmark_v2\n' \
  $'        run: if cargo test --locked --features symbiotic-memory-adapter; then true; fi\n'
expect_failure conditional-cargo "$path" "conditionally masks a mandatory command"

path="$(new_fixture disabled-errexit)"
replace_once \
  "$path" \
  $'        run: cargo test --locked --features symbiotic-memory-adapter --lib --bin membench --test benchmark_v2\n' \
  $'        run: |\n          set +e\n          cargo test --locked --features symbiotic-memory-adapter\n'
expect_failure disabled-errexit "$path" "must not disable errexit"

path="$(new_fixture background-cargo)"
replace_once \
  "$path" \
  $'        run: cargo test --locked --features symbiotic-memory-adapter --lib --bin membench --test benchmark_v2\n' \
  $'        run: cargo test --locked --features symbiotic-memory-adapter &\n'
expect_failure background-cargo "$path" "must not background mandatory commands"

path="$(new_fixture shell-override)"
replace_once \
  "$path" \
  $'        run: cargo test --locked --features symbiotic-memory-adapter --lib --bin membench --test benchmark_v2\n' \
  $'        shell: bash {0}\n        run: cargo test --locked --features symbiotic-memory-adapter --lib --bin membench --test benchmark_v2\n'
expect_failure shell-override "$path" "must not override shell failure semantics"

path="$(new_fixture native-compiler-cache)"
replace_once \
  "$path" \
  $'      CARGO_NET_GIT_FETCH_WITH_CLI: \"true\"\n' \
  $'      CARGO_NET_GIT_FETCH_WITH_CLI: \"true\"\n      CMAKE_CXX_COMPILER_LAUNCHER: ccache\n'
expect_failure native-compiler-cache "$path" "must not set CMAKE_CXX_COMPILER_LAUNCHER"

echo "OK: hostile adapter workflow fixtures passed"
