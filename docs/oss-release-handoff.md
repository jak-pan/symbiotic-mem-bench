# OSS Release Handoff — external decisions and blockers

Status ledger for taking this repository public. Everything here needs an owner decision or an
external action. Local macOS transfer recovery is green, but protected Ubuntu CI has not yet
certified the transferred dependency head and its target-matched native package.

## Blockers

1. **`symbiotic-sh/symbiotic-memory` is private** (ownership transferred 2026-07-25; GitHub API
   404 / `ls-remote` denied; `symbiotic-sh/symbiotic-foundation` is public). Measured
   impact: none for default + `server` builds — an anonymous, clean-`CARGO_HOME`
   `cargo check --features server` succeeds because Cargo does not fetch inactive optional
   git deps. But `--features symbiotic-memory-adapter` cannot build without access to that
   repo. Options: make it public, vendor the needed crates, or keep the adapter as an
   access-gated feature and say so in the README.
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
3. **Transferred kit needs fresh protected evidence.** The move from c22
   (`c22cfe30c9ccc7abcee28bf6f5abe6a7a659d74e`) to f6
   (`f6e406abeb13f2c734c4001fbc0fdf72ba43308a`) is a divergent squash-port that includes
   packaging and build-graph changes, not the same source revision at a new URL. Linux consumers
   must source-build and verify the target-matched zvec package using the f6 scripts before Cargo.
   The 2026-07-24
   K3/142-test attestation was earned against c22 and remains historical record-review evidence
   only. Current local macOS-arm64 f6 recovery evidence includes an observed count of 100
   adapter-enabled library tests, 51
   `membench` binary tests, 8 `benchmark_v2` contract tests, core/server checks, and production
   builds. A fresh protected trusted-repository Ubuntu CI run for the exact release head is still
   required.
4. **Upstream consumer ledger is stale.**
   `symbiotic-sh/symbiotic-memory/contracts/consumers.yaml` still records Membench as an expected
   failure for the removed `MemoryConfig` usage and pre-candidate lockfile. That no longer
   describes this branch. Updating the upstream contract requires a separate upstream PR after
   this commit is available to its committed-tree integration gate; this lane does not edit the
   upstream repository.

## Decisions pending

- **Optional external native state.** The portable canonical record is complete without
  `vaults/`, workflow state, provider queues, raw payloads, or debug state. Decide whether the
  much larger native substrate should also be retained for answer-only reuse and, if so,
  whether it belongs in a release asset, object-storage bucket, or Hugging Face dataset. This
  is not a blocker for the ranked public record; see
  `docs/canonical-record-storage-task.md`.
- **Landing deploy target.** The dashboard `dist/` is static-host ready with the snapshot
  fallback; choose host and domain, deploy only from a tagged commit.
- **Repository naming.** The crate is `membench`; the repo URL is still
  `symbiotic-mem-bench`. Rename or keep before announcing.
- **NOTICE/attribution.** LICENSE is Apache-2.0; decide whether a NOTICE file naming the
  copyright holder is wanted.
- **PostCSS tooling.** Any PostCSS dependency/tooling update is deliberately outside this
  transfer and must remain a separate reviewed change.

## Already done (for orientation)

- Clean-clone reproducibility for default + `server` features (pinned git deps, pinned
  toolchain, `Cargo.lock` committed).
- CI definition: fmt, clippy, core+server tests, release build, dashboard build, cargo-deny
  (advisories/licenses/sources), leaderboard contract canary (`canary/`), snapshot freshness
  vs `records/`, four-package git-dependency pin fixtures, and credentialed adapter checks with
  target-matched zvec preparation. Fresh protected results for the transferred f6 head remain
  pending.
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
