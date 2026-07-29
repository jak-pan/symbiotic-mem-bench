# OSS Release Handoff — external decisions and blockers

Status ledger for taking this repository public. Local macOS transfer recovery is green and all
six protected jobs passed on exact Membench head `d5808fb`. The v0.1.1 metadata and required
post-ledger repin will create a new candidate head, so those same six jobs must pass again before
tagging.

## Blockers

1. **The in-process Symbiotic Memory adapter is private; the harness is not.**
   `symbiotic-sh/symbiotic-memory` is private (ownership transferred 2026-07-25; GitHub API
   404 / `ls-remote` denied; `symbiotic-sh/symbiotic-foundation` is public). Measured
   impact: none for the public `membench` CLI, default library, leaderboard, or `server` builds —
   an anonymous, clean-`CARGO_HOME`
   `cargo check --features server` succeeds because Cargo does not fetch inactive optional
   git deps. The public CLI can import/explore/promote portable records without an in-process
   adapter. But `--features symbiotic-memory-adapter` cannot build without access to that repo.
   Options: make it public, vendor the needed crates, or keep the adapter as an access-gated
   owner integration and say so in the README. The last option permits an honest OSS harness +
   leaderboard release, but not a claim that the Symbiotic Memory executor itself is OSS.
   Consequence for CI: the owner repository has a scoped read-only
   `SYMBIOTIC_MEMORY_DEPLOY_KEY`. The adapter-enabled `rust` and `adapter-build` jobs are
   mandatory and explicitly reject a missing secret. Each uses that key for the exact canonical
   checkout, prepares and verifies the Linux zvec package from the pinned source before Cargo,
   and does not cache Cargo target, native-build, or private-derived outputs. Fork pull requests,
   where GitHub withholds the secret, therefore fail at key preflight before any Rust command and
   provide no Rust signal until a trusted same-repository run. The offline `adapter-pins` job proves
   exact manifest/lock/pin alignment for `symbiotic-memory`, `symbiotic-memory-config`, `zvec`,
   and `zvec-sys` without credentials. Release evidence still requires a trusted
   same-repository run (`RELEASING.md`).
2. ~~**Adapter APIs not yet published upstream.**~~ **Resolved 2026-07-24.** The kit APIs the
   adapter needs *are* on the pinned revision — they were renamed: what membench called
   `symbiotic_memory::MemoryConfig` (YAML `providers:` role bindings,
   `queue.resolve_provider_queue`) is `symbiotic_memory::EngineConfig` upstream, while
   `MemoryConfig` now names the newer layered TOML config in `symbiotic-memory-config`.
   `--features symbiotic-memory-adapter` builds and runs against the exact pins with no
   sibling checkout and no `.cargo/config.toml` override. The override block in
   `docs/environment.md` remains available for co-development, not as a requirement.
3. **Transferred kit needs exact-head protected evidence.** The move from c22
   (`c22cfe30c9ccc7abcee28bf6f5abe6a7a659d74e`) to f6
   (`f6e406abeb13f2c734c4001fbc0fdf72ba43308a`) is a divergent squash-port that includes
   packaging and build-graph changes, not the same source revision at a new URL. Linux consumers
   must source-build and verify the target-matched zvec package using the f6 scripts before Cargo.
   The 2026-07-24
   K3/142-test attestation was earned against c22 and remains historical record-review evidence
   only. Current local macOS-arm64 f6 recovery evidence includes an observed count of 100
   adapter-enabled library tests, 51
   `membench` binary tests, 8 `benchmark_v2` contract tests, core/server checks, and production
   builds. All six protected jobs passed on `d5808fb`. They must pass again after the v0.1.1
   metadata commit and after the required upstream repin; only the final exact head is release
   evidence.
4. **Upstream consumer ledger repair and repin are ordered.**
   The isolated upstream repair changes Membench from expected failure to expected pass and
   records the protected evidence at `d5808fb`. Landing that repair creates a new Symbiotic
   Memory revision, so this branch must then repin `.symbiotic-memory-pin`, both Cargo git
   dependencies, and all four locked packages to that merged revision before final protected CI.

## Decisions pending

- **Optional external native state.** The portable canonical record is complete without
  `vaults/`, workflow state, provider queues, raw payloads, or debug state. Decide whether the
  much larger native substrate should also be retained for answer-only reuse and, if so,
  whether it belongs in a release asset, object-storage bucket, or Hugging Face dataset. This
  is not a blocker for the ranked public record; see
  `docs/canonical-record-storage-task.md`.
- **Landing deploy target.** The dashboard `dist/` is static-host ready with the snapshot
  fallback. `release/release.json` binds it to `refs/tags/v0.1.1`; choose a host and deploy only
  from that exact tag after the tag-triggered landing gate passes. `dist/version.json` binds the
  actual tree to full commit/tag/version and both records and snapshot digests. A custom domain
  is not required for a canary.
- **Repository naming.** The crate is `membench`; the repo URL is still
  `symbiotic-mem-bench`. Rename or keep before announcing.
- **NOTICE/attribution.** LICENSE is Apache-2.0; decide whether a NOTICE file naming the
  copyright holder is wanted.
- **PostCSS tooling.** Any PostCSS dependency/tooling update is deliberately outside this
  transfer and must remain a separate reviewed change.

## Already done (for orientation)

- Clean-clone reproducibility for default + `server` features (pinned git deps, pinned
  toolchain, `Cargo.lock` committed).
- Public no-feature `membench` CLI for portable imports, exploration, record promotion, analytics,
  Trials, and vault management; native execution fails closed without an adapter.
- CI definition: fmt, clippy, core+server tests, release build, dashboard build, cargo-deny
  (advisories/licenses/sources), leaderboard contract canary (`canary/`), snapshot freshness
  vs `records/`, four-package git-dependency pin fixtures, and credentialed adapter checks with
  target-matched zvec preparation. All six protected jobs passed on exact head `d5808fb`;
  release metadata and the post-ledger repin must receive their own exact-head run.
- Ranking eligibility enforced in code (`src/eligibility.rs`) against bytes on disk, shared by
  the live API and the static export; cohorts partitioned by full comparability identity
  (benchmark, size, question set, judge, judge prompt mode).
- `membench.leaderboard.v1` export with per-row verification and recomputable
  `records_digest` provenance; truthful static leaderboard landing (explicit snapshot mode,
  no API polling and no error UI on a static host). The promoted
  `factconsol-thinkon-500-20260624` record is the verified rank-1 row at 437/500
  (`0.874`); the snapshot freshness gate validates it against `records/`.
- Safe-by-default portable promotion, public-hygiene validation, committed independent
  `review.json` attestation, and deterministic snapshot verification for the canonical 500Q
  record. Native state is intentionally excluded from the public record.
- OSS docs: `LICENSE`, `CONTRIBUTING.md`, `SECURITY.md`, `RELEASING.md`,
  `docs/longmemeval-methodology.md`.
