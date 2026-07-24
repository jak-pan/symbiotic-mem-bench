# MEMBENCH Flutter HTTP Debugger Spike

Canonical Flutter candidate after the FRB/Rust frontend experiment.

## Decision

Use this spike as the active Flutter candidate:

```text
Flutter Web --wasm / skwasm
+ no frontend Rust
+ no flutter_rust_bridge
+ direct backend API calls
+ backend owns compute/storage/schema
```

Keep the native Svelte dashboard as the production/baseline implementation while this Flutter route matures.

## Why this exists

The earlier Flutter spike used:

```text
Flutter UI → flutter_rust_bridge → Rust wasm → HTTP/JSON → membench-server
```

Inspection showed the Rust frontend layer only exposed seven view-model/data-loading functions:

```text
bridgeHealth
loadRegistry
loadOverview
loadTraces
loadQuestions
loadQuestionDebug
loadLive
```

That Rust layer mostly performed HTTP calls, JSON deserialization, grouping/filtering, and formatting. It did not contain the benchmark engine, memory/retrieval logic, scoring, model execution, or heavy local compute. The same UI can call the backend directly from Dart.

This spike uses:

```text
Flutter UI → Dart HTTP → membench-server
```

## Current architecture

- App source: `lib/main.dart`
- Data/API adapter: `lib/src/api/debugger.dart`
- Backend default: `http://127.0.0.1:8787/api`
- API override:

```bash
flutter build web --release --wasm \
  --dart-define=MEMBENCH_API=http://127.0.0.1:8787/api
```

## Build

Preferred build for this spike:

```bash
cd /Users/k/p/symbiotic-mem-bench/spikes/flutter-http-debugger
~/flutter-3.44.4/bin/flutter pub get
~/flutter-3.44.4/bin/flutter build web --release --wasm
```

Default CanvasKit build for comparison only:

```bash
~/flutter-3.44.4/bin/flutter build web --release
```

## Serve locally

Backend:

```bash
cd /Users/k/p/symbiotic-mem-bench
cargo run --features server --bin membench-server
```

Compressed static server example:

```bash
python3 /tmp/compressed_static_proxy.py \
  --port 8103 \
  --root /path/to/build/web \
  --coop \
  --spa
```

Validated test URL:

```text
http://127.0.0.1:8103/?run=runs%2Fsymbiotic-memory%2Flong-mem-eval%2F500%2Fcollapse-pplx&tab=questions
```

## Verified behavior

Validated against:

```text
runs/symbiotic-memory/long-mem-eval/500/collapse-pplx
```

Verified tabs:

- Overview: score/cards/model data render
- Questions: 500 rows load, filters/search/render cap work
- Traces: memory timing + bottleneck panels render
- Live: completed run summary, queues, pipeline drilldown, activity/errors render

Browser verification showed the `--wasm` build loads:

```text
/canvaskit/skwasm.wasm
/canvaskit/skwasm.js
/main.dart.mjs
/main.dart.wasm
```

No FRB/Rust resources are requested by this spike.

## Size evidence

All transfer sizes below are browser `ResourceTiming.transferSize` from Brotli-compressed local servers unless noted.

### Native Svelte baseline

Production Svelte dashboard static transfer:

| Resource | Transfer |
|---|---:|
| `/assets/index-CtkwRYfK.js` | 56,253 B |
| `/assets/index-b6d7qXvL.css` | 11,200 B |
| `/version.json` | 385 B |

Total static:

```text
67,838 B = 66.2 KB
```

Observed initial API transfer in the same load:

| Resource | Transfer |
|---|---:|
| `/api/runs` | 23,978 B |
| `/api/pending` | 1,544 B |
| `/api/health` | 388 B |

Svelte static + observed API:

```text
93,748 B = 91.6 KB
```

### Flutter no-Rust CanvasKit

Built with:

```bash
flutter build web --release
```

Bootstrap:

```text
compileTarget: dart2js
renderer: canvaskit
mainJsPath: main.dart.js
```

Browser transfer with local CanvasKit forced:

| Resource | Decoded | Transfer |
|---|---:|---:|
| `/canvaskit/chromium/canvaskit.wasm` | 5,760,502 B | 1,934,271 B |
| `/main.dart.js` | 2,604,789 B | 689,893 B |
| `/canvaskit/chromium/canvaskit.js` | 86,496 B | 26,777 B |
| `/flutter_bootstrap.js` | 10,055 B | 3,953 B |

Total static:

```text
2,654,894 B = 2.53 MB
```

### Flutter no-Rust `--wasm` / skwasm

Built with:

```bash
flutter build web --release --wasm
```

Bootstrap includes the preferred wasm path:

```text
compileTarget: dart2wasm
renderer: skwasm
mainWasmPath: main.dart.wasm
jsSupportRuntimePath: main.dart.mjs
```

It also contains a CanvasKit/dart2js fallback.

Browser transfer with local skwasm forced:

| Resource | Decoded | Transfer |
|---|---:|---:|
| `/canvaskit/skwasm.wasm` | 3,580,947 B | 1,395,942 B |
| `/main.dart.wasm` | 2,238,182 B | 739,497 B |
| `/canvaskit/skwasm.js` | 63,316 B | 16,674 B |
| `/main.dart.mjs` | 33,573 B | 7,630 B |
| `/flutter_bootstrap.js` | 10,172 B | 3,976 B |

Total static:

```text
2,163,719 B = 2.06 MB
```

Post-Protobuf Flutter measurement after adding the generated Dart client and switching health/runs/pending/questions to `.pb` with JSON fallback:

| Resource | Decoded | Transfer |
|---|---:|---:|
| `/canvaskit/skwasm.wasm` | 3,580,947 B | 1,395,942 B |
| `/main.dart.wasm` | 2,267,709 B | 754,667 B |
| `/canvaskit/skwasm.js` | 63,316 B | 16,674 B |
| `/main.dart.mjs` | 33,573 B | 7,630 B |
| `/flutter_bootstrap.js` | 10,174 B | 3,978 B |

Post-Protobuf total static:

```text
2,178,891 B = 2.08 MB
```

The Dart Protobuf runtime/generated client increased Flutter wasm static transfer by about `15,172 B` versus the no-Protobuf wasm measurement.

Delta vs no-Rust CanvasKit:

```text
491,175 B saved = 479.7 KB saved
18.5% smaller transfer
```

### Prior Flutter + Rust/FRB CanvasKit

Artifact Brotli sum, equivalent to browser transfer if served locally with CanvasKit forced:

| Resource | Brotli q6 |
|---|---:|
| `flutter_bootstrap.js` | 3,653 B |
| `main.dart.js` | 670,229 B |
| `pkg/flutter_frb_debugger_rust.js` | 8,197 B |
| `pkg/flutter_frb_debugger_rust_bg.wasm` | 319,852 B |
| `canvaskit/chromium/canvaskit.js` | 26,477 B |
| `canvaskit/chromium/canvaskit.wasm` | 1,933,971 B |

Total static:

```text
2,962,379 B = 2.83 MB
```

Removing FRB/Rust saved approximately:

```text
2.83 MB - 2.53 MB = 0.30 MB compressed
```

`--wasm` plus no-Rust saved approximately:

```text
2.83 MB - 2.06 MB = 0.77 MB compressed
```

## Ratios vs Svelte baseline

| Build | Static transfer | vs Svelte static |
|---|---:|---:|
| Svelte native | 66.2 KB | 1× |
| Flutter no-Rust CanvasKit | 2.53 MB | 39.1× |
| Flutter no-Rust `--wasm` / skwasm | 2.06 MB | 31.9× |
| Flutter + Rust/FRB CanvasKit | 2.83 MB | 44.7× |

Despite the larger payload, the `--wasm` build was judged acceptable by interactive performance testing.

## Typed API boundary

Do not reintroduce frontend Rust/FRB. The API contract is now the shared Protobuf schema:

```text
proto/membench/dashboard/v1/debugger.proto
```

Generation paths:

```text
Rust backend  → prost-build in ../../build.rs
Svelte/TS     → protobuf-es via ../../dashboard/package.json `npm run proto:gen`
Flutter/Dart  → official Dart protoc plugin via scripts/gen-proto.sh
```

Flutter generation command:

```bash
dart pub global activate protoc_plugin
export PATH="$HOME/.pub-cache/bin:$PATH"
./scripts/gen-proto.sh
```

Current Flutter data layer status:

- Protobuf-first with JSON fallback for:
  - `health.pb`
  - `runs.pb`
  - `pending.pb`
  - `run/questions.pb`
- Still JSON for detail/traces/live/question-debug/artifacts until those messages are added to the schema.

## Cleanup policy

- Keep `dashboard/` as production/baseline.
- Keep `spikes/flutter-http-debugger/` as canonical Flutter candidate.
- Archive/remove old FRB/Rust spike outputs after preserving evidence.
- Do not commit generated measurement temp dirs or old build outputs.
