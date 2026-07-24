## Rebuild instructions (reproducible)

The 2.13.0-beta.4 codegen is **NOT** installed globally — the global `~/.cargo/bin/flutter_rust_bridge_codegen` is still 2.12.0. Use the isolated install at `/tmp/frb213-cargo/bin/`. A durable script (`scripts/build-web.sh`) is not yet committed; manual rebuild:

```bash
# 0. (One-time, if missing) isolated codegen 2.13.0-beta.4
cargo install --root /tmp/frb213-cargo \
  flutter_rust_bridge_codegen --version 2.13.0-beta.4 --locked
export PATH="$HOME/flutter-3.44.4/bin:/tmp/frb213-cargo/bin:$HOME/.cargo/bin:$PATH"

# 1. Lock to beta.4 (already done in this spike)
#    pubspec.yaml:  flutter_rust_bridge: 2.13.0-beta.4
#    rust/Cargo.toml: flutter_rust_bridge = "=2.13.0-beta.4"

# 2. Resolve + regen bindings
cd spikes/flutter-frb-debugger
flutter pub get
cargo update --manifest-path rust/Cargo.toml -p flutter_rust_bridge --precise 2.13.0-beta.4
flutter_rust_bridge_codegen generate

# 3. Build wasm package
flutter_rust_bridge_codegen build-web \
  --dart-root . --rust-root rust --output ../web --release

# 4. Build Flutter web
flutter build web --release

# 5. Serve with COOP/COEP headers (SharedArrayBuffer requires cross-origin isolation)
python3 /tmp/hermes-static-server.py 8098 "$(pwd)/build/web"
```

## Open ports

- `8787` — `membench-server` (API, started earlier in session)
- `8098` — flutter-frb-debugger on the in-place spike's `build/web`
- `8093`, `8094` — earlier-session spikes kept running per operator

## Known limitations / caveats

1. **beta.4 is a beta.** pub.dev lists it as prerelease. A future 2.13.0-stable could shift behavior. Do not call this "stable frb web" without re-testing.
2. **The `--wasm` Flutter web build path is still unproven.** frb 2.12.0+ ships a Dart-WASM lint violation (`invalid_runtime_check_with_js_interop_types` in `_web.dart:44`) that blocks the smaller `--wasm` build. The default JS build works.
3. **8 deprecation warnings per page load.** frb's own `WorkerPool` workers still call `__wbg_init` with the old 2-arg form. Works today, but means frb internals are mid-migration.
4. **`build_live` fix has no unit test.** The rinf spike doesn't have one either. A 5-line test in `debugger.rs` calling `build_live` on a sample `LiveResponse` with populated `detail` would close this gap.
5. **Bundle size unchanged from 2.12.0.** ~907KB wasm + 47KB JS shim = ~954KB. CanvasKit add-on is unchanged (~2.2MB). Bundle remains ~50× Svelte (54KB). This is not a regression but it also isn't a fix.
6. **`/tmp` volatility.** The isolated codegen install at `/tmp/frb213-cargo/` survives normal reboots in `/tmp` only if `/tmp` is persistent on this machine (usually not). A reboot will require re-running step 0.
7. **Live proof used a stalled run, not active streaming.** `merged-qwen` is `STALLED` (status=stalled, updated 5h 49m ago), so the proof demonstrates the data-display path but not continuous streaming during an actively running provider job. Proving the streaming path would require starting a fresh benchmark, which costs provider budget.

## Why this report is the right place to stop

This spike answered the original operator question — "can frb drive the debug dashboard on web" — with empirical evidence. The answer is **yes, in beta.4**. The matrix's pre-existing recommendation ("Flutter for web: Not competitive") is still defensible on bundle size grounds even with this fix, and should be updated separately by whoever owns the cross-cutting framework-comparison doc (`spikes/DASHBOARD-MATRIX.md`).

This spike is **not committed** — `spikes/` is untracked at the repo root. The user has been operating on a "keep, don't commit" cadence. Per repo policy (`CONTEXT.md` rule: "Never use raw `git commit`. Pushes require explicit user instruction.") the working state lives on disk only.
