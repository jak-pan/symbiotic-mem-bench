# Releasing

## Versioning and distribution

- The Rust package (`Cargo.toml`) and dashboard (`dashboard/package.json`) share one release
  version. JSON contracts keep independent schema ids (`membench.report.v1`,
  `membench.leaderboard.v1`, and so on).
- Membench is distributed through GitHub source archives and attached product bundles. The Cargo
  package is deliberately `publish = false`. The Symbiotic Memory adapter lives in a separate,
  non-workspace package because its private Git-only crates cannot be a hidden public dependency.
- Public product bundles are self-contained and read-only: `membench-server`,
  `membench-leaderboard`, the built v2 dashboard, and portable tracked records. They never contain
  the credentialed adapter binary, private source, provider credentials, local runs, or native
  state.

## Pre-tag checklist

Run from an exact clean checkout of the intended release commit:

1. Verify version and release contract:
   `./scripts/check-release-version.sh vX.Y.Z`.
2. Run the same source gates as CI:
   - `cargo fmt -- --check`
   - `cargo clippy --locked --all-targets --features server -- -D warnings`
   - `cargo test --locked` and `cargo test --locked --features server`
   - `cargo build --locked --release --features server`
   - `cargo deny check advisories licenses sources`
   - `cd dashboard && npm ci && npm run build`
   - `./scripts/check-adapter-pins.sh`
   - `python3 scripts/generate-third-party-notices.py --check`
   - `python3 -m unittest scripts.tests.test_package_release`
   - deterministic canary export matches `canary/expected-leaderboard.json`
   - `./scripts/check-leaderboard-snapshot.sh`
   - tracked-tree secret scan and release-bundle forbidden-path scan
3. Maintainers with read access must run `./scripts/check-adapter-build.sh`. The gate resolves the
   exact locked Memory checkout and stages its verified prebuilt for the host target before Cargo;
   it proves the private integration remains compatible with the exact pins but does **not** put
   that adapter in the public asset.
4. Build a local platform bundle with `scripts/package-release.py`, build it a second time with the
   same inputs/epoch, and require identical SHA-256 hashes. Extract it away from the checkout, start
   `./membench-server`, and verify `/api/health`, `/api/runs`, `/api/leaderboard`, the v2 shell,
   Questions, and Traces.
5. Confirm the release notes identify product UI v2 separately from the experimental,
   non-promotable `longmemeval-v2-text` score lane.

Do not create `vX.Y.Z` until every item above is green on the exact commit.

## Tag automation

Push an annotated `vX.Y.Z` tag only after the pre-tag checklist. `.github/workflows/release.yml`
then fails closed unless the tag, Cargo version, dashboard version, dashboard lockfile version, and
Cargo lockfile package version match exactly. It reruns the release gates, builds the supported
platform bundles, smoke-tests each extracted archive, publishes checksums and provenance, and
creates a **draft** GitHub release. It never publishes the release automatically.

Before promoting that draft:

1. Verify every asset against `SHA256SUMS`.
2. Confirm each `PROVENANCE.json` names the tag commit, target, records digest, UI tree digest, and
   binary hashes.
3. Repeat one unpack-and-serve smoke from the downloaded asset, not a workspace build.
4. Deploy any production/canary service from that exact downloaded asset and verify the same commit,
   binary hash, UI bundle hash, and records digest.
5. Publish the release only after the server-backed canary passes. A static-only dashboard is a
   leaderboard fallback, not the complete v2 product.

## What a release must never do

- Publish ranked scores not derived from tracked records passing the implemented review gate in
  `docs/longmemeval-methodology.md`.
- Populate a public board from `canary/records`; those are synthetic contract fixtures.
- Include `runs/`, `.debug-session/`, secrets, raw prompts, provider payloads, `raw/`, `vaults/`,
  `workflow/`, `provider-queue/`, SQLite state, or private adapter source in a product bundle.
- Omit `THIRD_PARTY_NOTICES.md`, or ship a notice inventory that does not match the locked Cargo and
  dashboard graphs or the redistributed LongMemEval-derived artifacts.
- Claim the `longmemeval-v2-text` projection is an official LongMemEval-V2 score.
- Publish a tag or release from a dirty checkout, a commit other than the reviewed head, or an asset
  that has not passed the extracted-bundle smoke.
