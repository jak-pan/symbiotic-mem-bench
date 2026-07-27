# Releasing

## Versioning

- The crate (`Cargo.toml`) and the dashboard (`dashboard/package.json`) share the release
  version. Pre-1.0, minor bumps may break contracts; patch bumps are fixes only.
- JSON contracts are versioned independently by schema id (`membench.report.v1`,
  `membench.leaderboard.v1`, …). Breaking a contract means a new schema id, not a silent
  field change — the CI canary (`canary/`) enforces this for the leaderboard export.

## Release checklist

This transfer is a kit upgrade, not a repository-only URL change. The previous adapter pin
`c22cfe30c9ccc7abcee28bf6f5abe6a7a659d74e` and the transferred
`f6e406abeb13f2c734c4001fbc0fdf72ba43308a` revision are divergent: f6 is a squash-port that also
changes packaging and the build graph. Evidence earned against c22 does not certify f6.

1. Gates green (same set as CI):
   - `cargo fmt -- --check`, `cargo clippy --all-targets --features server -- -D warnings`
   - `cargo test` and `cargo test --features server`
   - `cargo build --release --features server`
   - `cargo deny check advisories licenses sources`
   - `cd dashboard && npm ci && npm run build`
   - `./scripts/check-adapter-pins.sh` — git deps pinned to exact revs, resolved by `Cargo.lock`
   - canary diff: deterministic export over `canary/records` matches
     `canary/expected-leaderboard.json`
   - `./scripts/check-leaderboard-snapshot.sh` — the bundled snapshot still matches `records/`
2. **Adapter CLI gate, credentialed until the dependency is public:**
   `./scripts/check-adapter-build.sh` on a machine with access to
   `symbiotic-sh/symbiotic-memory`. This is the only check that the documented `membench` CLI
   builds *and runs* against the pinned revisions; it also runs the
   `benchmark_v2` projection/evaluator contract test. The `rust`, `deps`, and
   `leaderboard-contract` jobs authenticate before their Rust gates and fail closed when a fork
   cannot read the private dependency; the conditional `adapter-build` job is skipped without
   its key. Therefore a fork run is not sufficient release evidence. Require a green protected,
   trusted same-repository run and do not tag from a keyless checkout.
   Local f6 recovery evidence currently consists of 100 adapter-enabled library tests, 51
   `membench` binary tests, 8 `benchmark_v2` contract tests, core/server checks, and production
   builds. Fresh protected CI for the release head remains pending.
3. No stray state: `git status --short --ignored` shows only expected ignored paths
   (`runs/`, external target dir, local env files).
4. Bump versions in `Cargo.toml` + `dashboard/package.json`, update `Cargo.lock`, commit.
5. Tag `vX.Y.Z` and create a GitHub release with notes (contract changes called out
   explicitly).
6. Deploying the leaderboard landing (optional): publish `dashboard/dist/` to a static host.
   Build from the tagged commit, and verify the deployed document by recomputing
   `source.records_digest` from that checkout — that hash, not the exporter git sha, is what
   proves the published board describes the records in the tag.

## What a release must never do

- Publish ranked scores that do not come from tracked records passing the review gate in
  `docs/longmemeval-methodology.md`. The gate is enforced in code, but a *new* gate condition
  that is only documented and not implemented must not be described as enforced.
- Publish a leaderboard built from `canary/records`. Those are synthetic fixtures; the export
  flags them (`source.contains_fixtures`, per-row `fixture: true`) and the landing page warns,
  but the fixtures exist to test the contract, never to populate a board.
- Include `runs/`, secrets, raw prompts, or provider payloads in any artifact.
