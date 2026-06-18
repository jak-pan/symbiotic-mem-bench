# Membench Dashboard

A Bloomberg-terminal-style dashboard for memory-system benchmarks. Two screens:

- **Leaderboard** — Geekbench-style ranking of systems/configs within a comparable
  cohort (same benchmark + size + question set + judge). Cohort selector, ranked
  field, per-category matrix, and a head-to-head compare rail.
- **Debugger / Tuner / Runner** — per-run overview, a filterable question browser,
  baseline comparison, model/memory/queue traces, and a parameter tuner that
  previews the exact `membench`/`symem` command (live execution is the next phase).

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

## Server options

```text
--port <PORT>          default 8787
--repo-root <PATH>     run ids are made relative to this (default: crate root)
--root <PATH>          registry root(s) to scan (default: <repo>/runs and /records)
--dist <PATH>          built SPA directory (default: <repo>/dashboard/dist)
```

## Keyboard

`/` focus command · `F1` leaderboard · `F2` debugger · click rows to stack for compare.
