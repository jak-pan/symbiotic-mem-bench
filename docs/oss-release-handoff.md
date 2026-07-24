# OSS Release Handoff — external decisions and blockers

Status ledger for taking this repository public. Everything here needs an owner decision or an
external action; the code-side work is done and gated in CI.

## Blockers

1. **`jak-pan/symbiotic-memory` is private** (verified anonymously 2026-07-24: GitHub API
   404 / `ls-remote` denied; `symbiotic-sh/symbiotic-foundation` is public). Measured
   impact: none for default + `server` builds — an anonymous, clean-`CARGO_HOME`
   `cargo check --features server` succeeds because Cargo does not fetch inactive optional
   git deps. But `--features symbiotic-memory-adapter` cannot build without access to that
   repo. Options: make it public, vendor the needed crates, or keep the adapter as an
   access-gated feature and say so in the README.
2. **Adapter APIs not yet published upstream.** Building `--features
   symbiotic-memory-adapter` additionally requires kit APIs (YAML `providers:` role bindings,
   `queue.resolve_provider_queue`) that only exist in sibling checkouts. Until they land on
   the pinned branches, adapter builds need the `.cargo/config.toml` override block in
   `docs/environment.md` ("Dependency Sources").

## Decisions pending

- **Canonical 500Q record.** Which run to promote, and where its native-state bundle lives
  (release asset vs bucket vs HF dataset) — open decisions in
  `docs/canonical-record-storage-task.md`. Until promoted, the leaderboard publishes no
  ranked score (see `docs/longmemeval-methodology.md`).
- **Landing deploy target.** The dashboard `dist/` is static-host ready with the snapshot
  fallback; choose host and domain, deploy only from a tagged commit.
- **Repository naming.** The crate is `membench`; the repo URL is still
  `symbiotic-mem-bench`. Rename or keep before announcing.
- **NOTICE/attribution.** LICENSE is Apache-2.0; decide whether a NOTICE file naming the
  copyright holder is wanted.

## Already done (for orientation)

- Clean-clone reproducibility for default + `server` features (pinned git deps, pinned
  toolchain, `Cargo.lock` committed).
- CI: fmt, clippy, core+server tests, release build, dashboard build, cargo-deny
  (advisories/licenses/sources), leaderboard contract canary (`canary/`).
- `membench.leaderboard.v1` export with per-row verification levels; truthful static
  leaderboard landing (snapshot-labeled, verified cohorts empty until a record passes the
  review gate).
- OSS docs: `LICENSE`, `CONTRIBUTING.md`, `SECURITY.md`, `RELEASING.md`,
  `docs/longmemeval-methodology.md`.
