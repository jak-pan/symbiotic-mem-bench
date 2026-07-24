# Releasing

## Versioning

- The crate (`Cargo.toml`) and the dashboard (`dashboard/package.json`) share the release
  version. Pre-1.0, minor bumps may break contracts; patch bumps are fixes only.
- JSON contracts are versioned independently by schema id (`membench.report.v1`,
  `membench.leaderboard.v1`, …). Breaking a contract means a new schema id, not a silent
  field change — the CI canary (`canary/`) enforces this for the leaderboard export.

## Release checklist

1. Gates green (same set as CI):
   - `cargo fmt -- --check`, `cargo clippy --all-targets --features server -- -D warnings`
   - `cargo test` and `cargo test --features server`
   - `cargo build --release --features server`
   - `cargo deny check advisories licenses sources`
   - `cd dashboard && npm ci && npm run build`
   - canary diff: deterministic export over `canary/records` matches
     `canary/expected-leaderboard.json`
2. Snapshot fresh: `scripts/export-leaderboard-snapshot.sh` produces no diff (or commit the
   regenerated snapshot with the records change that caused it).
3. No stray state: `git status --short --ignored` shows only expected ignored paths
   (`runs/`, external target dir, local env files).
4. Bump versions in `Cargo.toml` + `dashboard/package.json`, update `Cargo.lock`, commit.
5. Tag `vX.Y.Z` and create a GitHub release with notes (contract changes called out
   explicitly).
6. Deploying the leaderboard landing (optional): publish `dashboard/dist/` to a static host.
   The bundle must have been built from the tagged commit so the snapshot provenance
   (exporter git sha) matches.

## What a release must never do

- Publish ranked scores that do not come from tracked records passing the review gate in
  `docs/longmemeval-methodology.md`.
- Include `runs/`, secrets, raw prompts, or provider payloads in any artifact.
