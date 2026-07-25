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
   - `./scripts/check-adapter-pins.sh` — git deps pinned to exact revs, resolved by `Cargo.lock`
   - canary diff: deterministic export over `canary/records` matches
     `canary/expected-leaderboard.json`
   - `./scripts/check-leaderboard-snapshot.sh` — the bundled snapshot still matches `records/`
2. **Adapter CLI gate, manual until the dependency is public:**
   `./scripts/check-adapter-build.sh` on a machine with access to
   `jak-pan/symbiotic-memory`. This is the only check that the documented `membench` CLI
   builds *and runs* against the pinned revisions; it also runs the mandatory
   `benchmark_v2` projection/evaluator contract test. CI runs it automatically only when the
   repository has a read-only `SYMBIOTIC_MEMORY_DEPLOY_KEY` secret; without that
   secret the job is skipped and the path is unverified in CI. Do not tag a release
   without running it somewhere.
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
