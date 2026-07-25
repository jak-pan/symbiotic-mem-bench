# Membench Dashboard

A Bloomberg-terminal-style terminal for memory-system benchmarks (in-app:
**MEMBENCH · MEMORY SYSTEM TERMINAL**). Two screens:

- **Leaderboard** (`F1`) — Geekbench-style ranking of systems/configs within a
  comparable cohort (same benchmark + size + question set + judge). Cohort
  selector, ranked field, per-category matrix, and a head-to-head compare rail.
- **Debugger** (`F2`) — per-run Overview, a filterable Questions browser,
  baseline Compare, model/memory/queue Traces, a Live monitor for in-flight
  native runs, and a **Tuner** that previews the exact `membench`/`symem`
  command (live execution is the next phase).

Frontend is a no-SSR Svelte 5 + Vite SPA. The backend is the Rust
`membench-server` binary, which serves the same `runs/` and `records/` files the
CLI reads.

Dashboard commands expect `node` and `npm` to resolve to the user `nvm` install from PATH. Do not
install or use Homebrew Node for this repo on this machine.

## Develop

```bash
# 1. backend (from repo root) — serves /api on :8787
cargo run --features server --bin membench-server

# 2. frontend dev server on :5173 (proxies /api to :8787)
cd dashboard && npm install && npm run dev
```

Open http://localhost:5173.

## Build + serve as one binary

```bash
cd dashboard && npm run build      # → dashboard/dist
cargo run --features server --bin membench-server   # serves dist + /api
```

Open http://localhost:8787.

## Static deploy (leaderboard landing)

The built SPA is also deployable to any static host with no Rust backend. When
`/api` is unreachable, the Leaderboard route falls back to the committed
`membench.leaderboard.v1` export bundled at `public/data/leaderboard.json`
(copied into `dist/data/` by Vite) and labels everything it shows as a
**STATIC SNAPSHOT** with its provenance (records root, exporter git sha,
generation time). Verified cohorts stay empty unless tracked records contain
fully-attested scored runs; unverified/meta records are listed separately with
the reason they are unranked. The UI never fabricates ranked scores.

Regenerate the snapshot after `records/` changes:

```bash
scripts/export-leaderboard-snapshot.sh   # from the repo root
```

Verify the static bundle locally by serving `dist/` without the API, e.g.:

```bash
cd dashboard && npm run build && python3 -m http.server 4174 -d dist
```

## Server options

```text
--port <PORT>          default 8787
--repo-root <PATH>     run ids are made relative to this (default: crate root)
--root <PATH>          registry root(s) to scan (default: <repo>/runs and /records)
--dist <PATH>          built SPA directory (default: <repo>/dashboard/dist)
```

## Keyboard

`/` focus command · `F1` leaderboard · `F2` debugger · click rows to stack for compare.
