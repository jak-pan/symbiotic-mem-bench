# Dashboard Framework Comparison

## Parity target: egui (`http://127.0.0.1:8085`)
## DOM baseline: Svelte (`http://localhost:8090`)

## Port Map (all running)

```text
8084  iced           trunk serve (Rust/WASM)
8085  egui           trunk serve (Rust/WASM)  ← visual parity target
8087  Flutter+rinf   python static server (release build)
8088  Svelte+Tauri   node/vite (web dev)
8089  Flutter+FRB    python static server (release build)
8090  Svelte         node/vite (canonical DOM baseline)
8787  API backend    axum (membench-server)
```

## Visual Parity vs egui (agent vision audit)

| Feature | egui (target) | iced | Flutter+rinf | Flutter+FRB |
|---|---|---|---|---|
| Ring gauge 270° | ✅ centered text | ⚠️ text below ring | ✅ user confirmed | ❓ needs user check |
| Heat tiles | ✅ 6 per row | ❌ not visible | ✅ | ❓ |
| Table density | ✅ 43px rows | ❌ not visible | ✅ | ❓ |
| Comparison radar | ✅ side-by-side | ⚠️ hidden | ✅ | ❓ |
| Status bar | ✅ full | ⚠️ missing SYSTEMS+SRV | ✅ | ❓ |
| Tab overflow | ✅ single row | ❌ wraps vertically | ✅ | ❓ |
| Animations | ✅ built-in | ❌ none | ✅ | ❓ |

**Iced has 10 confirmed gaps vs egui.** The biggest: table+comparison not visible (layout/height bug).

## Bundle Size

| Framework | App code brotli | Renderer brotli | First-load (app+1 renderer) |
|---|---:|---:|---:|
| **Svelte** | **54 KB** | 0 (browser native) | **54 KB** |
| **iced** | **1,352 KB** | (included in wasm) | **1,352 KB** |
| **egui** | **1,518 KB** | (included in wasm) | **1,518 KB** |
| **Flutter+FRB** | **471 KB** + 31 KB rust | CanvasKit 2,215 KB / skwasm 1,192 KB | **2,665 KB** (CanvasKit) / **1,666 KB** (skwasm) |
| **Flutter+rinf** | **477 KB** | CanvasKit 2,215 KB / skwasm 1,192 KB | **2,692 KB** (CanvasKit) / **1,693 KB** (skwasm) |

Flutter ships 4 renderer variants in `build/web/`; the browser downloads only one based on capabilities. The `--wasm` build enables the smaller skwasm path for WasmGC-capable browsers.

FRB Rust bridge adds only **31 KB brotli** — negligible vs CanvasKit.

## Runtime Memory

**Measured via Chrome Task Manager (real tab memory footprint, not JS heap):**

| Framework | Port | Tab Memory | Source |
|---|---|---:|---|
| Flutter+rinf | 8087 | **422 MB** | User Chrome Task Manager |
| Flutter+FRB | 8089 | **pending** | — |
| iced | 8084 | **pending** | — |
| egui | 8085 | **pending** | — |
| Svelte | 8090 | **pending** | — |
| Svelte+Tauri | 8088 | **pending** | — |

Previous headless browser JS heap readings (3-10 MB) undercount badly — they exclude WASM linear memory, GPU buffers, and CanvasKit's Skia heap. Chrome Task Manager is the honest metric.

## Rust Bridge Comparison

| Feature | flutter_rust_bridge | rinf | Tauri commands | egui/iced (same language) |
|---|---|---|---|---|
| Version | 2.11.1 | 8.10.0 | 2.x | n/a |
| Existing investment | ✅ Symbiotic app | ❌ new | ❌ new | ✅ same crate |
| Web compatibility | ⚠️ needs wasm-pack + build-web | ⚠️ wasm threading fails | ✅ native | ✅ native |
| Bundle overhead | 31 KB brotli | 0 (no bridge wasm) | 0 | 0 |
| Codegen | `flutter_rust_bridge_codegen` | `rinf gen` | auto from Rust | n/a |
| Blocking risk | Low (async) | High (blocks runApp if awaited) | n/a | n/a |

## Mobile/App Story

| Framework | iOS | Android | Desktop | Web |
|---|---|---|---|---|
| Svelte+Tauri 2 | ✅ scaffold generated | ⚠️ needs NDK | ✅ 8.1 MB .app | ✅ PWA |
| Flutter+rinf | ✅ native | ✅ native | ✅ native | ⚠️ 2.7 MB load |
| Flutter+FRB | ✅ native | ✅ native | ✅ native | ⚠️ 2.7 MB load |
| egui | ❌ | ❌ | ❌ | ✅ only |
| iced | ❌ | ❌ | ❌ | ✅ only |

## Rendering Bugs Per Framework

| Framework | Bugs | Severity |
|---|---|---|
| Svelte | 0 | — |
| Tauri | 0 (wraps Svelte) | — |
| egui | 1 (constant repaint loop ~60fps) | Low |
| iced | 4 (WebGPU black, MSAA geometry vanish, canvas text, winit DPR panic) | Critical |
| Flutter+rinf | 1 (rinf blocks runApp — fixed) | Fixed |
| Flutter+FRB | 0 so far | — |

## Recommendation (draft)

**Unchanged from data:**
1. **Web dashboard:** Svelte (54 KB, zero bugs, full DOM)
2. **App wrapper:** Svelte+Tauri 2 (same frontend, 8.1 MB native, iOS path)
3. **If canvas required:** egui over iced (fewer bugs, better AA, better dev ergonomics)
4. **Flutter for web:** Not competitive (50× Svelte size, CanvasKit tax) — keep for mobile-native only
5. **Rust bridge:** FRB over rinf (matches existing Symbiotic investment, web-compatible, 31 KB overhead)

**Pending before final:**
- Iced→egui parity fixes (10 gaps, table visibility is blocker)
- Flutter RAM from user Chrome Task Manager (8087 + 8089)
- FRB visual verification by user on port 8089
