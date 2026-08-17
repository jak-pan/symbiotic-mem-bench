# Membench Dashboard

A Bloomberg-terminal-style benchmark and debugging cockpit for memory systems.
The v2 shell has four operator workspaces:

- **Leaderboard** (`F1`) — reviewed results within an exact dataset × scale ×
  judge × prompt-mode cohort, with category evidence and methodology context.
- **Runs** (`F2`) — ranked and held-back records, publication-gate failures,
  and the artifact-coverage boundary for deeper inspection.
- **Lab** (`F3`) — the evidence-producing run contract. Static deployments are
  deliberately read-only until a safe execution endpoint is connected.
- **Catalog** (`F4`) — observed systems, benchmarks and portable artifact
  classes from the loaded registry or publication snapshot.

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
--repo-root <PATH>     run ids are made relative to this (default: extracted bundle root, then crate root)
--root <PATH>          registry root(s) to scan (default: <repo>/runs and /records)
--dist <PATH>          built SPA directory (default: <repo>/dashboard/dist)
```

## Keyboard

`F1` leaderboard · `F2` runs · `F3` lab · `F4` catalog.
