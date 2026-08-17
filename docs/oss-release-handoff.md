# OSS Release Handoff — external decisions and blockers

Status ledger for taking this repository public. Everything here needs an owner decision or an
external action; the code-side work is done and gated in CI.

## Blockers

1. **`symbiotic-sh/symbiotic-memory` is private** (verified through repository metadata on
   2026-08-18 after its transfer from `jak-pan`). Measured impact: none for default + `server`
   builds because the public root manifest contains no private Git dependencies. The public v2
   product bundle therefore contains the server, v2 dashboard, leaderboard exporter, and portable
   records, but deliberately excludes the private adapter binary. The isolated
   `adapters/symbiotic-memory/Cargo.toml` package requires explicit repository access and is not
   part of the public install contract.
   The pinned adapter revision's native zvec artifact is macOS arm64 only. Adapter compatibility is
   therefore gated on macOS 14 arm64; the independent public server product remains cross-platform.
   Do not vendor private source or credentials into Membench. The adapter can enter the public
   release only after its complete pinned dependency and zvec runtime are publicly reproducible.
2. ~~**Adapter APIs not yet published upstream.**~~ **Resolved 2026-07-24.** The kit APIs the
   adapter needs *are* on the pinned revision — they were renamed: what membench called
   `symbiotic_memory::MemoryConfig` (YAML `providers:` role bindings,
   `queue.resolve_provider_queue`) is `symbiotic_memory::EngineConfig` upstream, while
   `MemoryConfig` now names the newer layered TOML config in `symbiotic-memory-config`.
   the isolated adapter package builds and runs against the exact pins with no
   sibling checkout and no `.cargo/config.toml` override. The override block in
   `docs/environment.md` remains available for co-development, not as a requirement.
3. ~~**Mandatory opposite-model v0.1.0 release approval.**~~ **Resolved 2026-07-24.** K3 independently
   rebuilt the adapter, reproduced the traversal refusal, recounted 437/500, matched the three
   review hashes, verified the public-hygiene and leaderboard snapshot gates, and ran all 142
   tests before returning `K3_APPROVE`.

## Decisions pending

- **Optional external native state.** The portable canonical record is complete without
  `vaults/`, workflow state, provider queues, raw payloads, or debug state. Decide whether the
  much larger native substrate should also be retained for answer-only reuse and, if so,
  whether it belongs in a release asset, object-storage bucket, or Hugging Face dataset. This
  is not a blocker for the ranked public record; see
  `docs/canonical-record-storage-task.md`.
- **v2 production promotion.** A server-backed canary exists, but production promotion must use the
  exact tagged product bundle and verify both the API-backed evidence views and the static
  leaderboard fallback. A static-only host is not the complete v2 product.
- **Repository naming.** The crate is `membench`; the repo URL is still
  `symbiotic-mem-bench`. Rename or keep before announcing.
- **NOTICE/attribution.** LICENSE is Apache-2.0; decide whether a NOTICE file naming the
  copyright holder is wanted.

## v2.0.0 release boundary

- The crate and dashboard versions are `2.0.0`; the Cargo package is intentionally
  `publish = false`. `cargo package`/crates.io are not the distribution path while adapter crates
  remain Git-only.
- GitHub source archives are the source distribution. Release automation produces deterministic
  platform bundles for the self-contained, read-only server-backed product and checksum/provenance
  files. The release starts as a draft and is promoted only after unpack-and-serve smoke checks.
- The private adapter is an additional credentialed source-build gate for maintainers. Its success
  proves integration compatibility but does not make it a public release asset.

## Already done (for orientation)

- Clean-clone reproducibility for default + `server` features (pinned git deps, pinned
  toolchain, `Cargo.lock` committed).
- CI: fmt, clippy, core+server tests, release build, dashboard build, cargo-deny
  (advisories/licenses/sources), leaderboard contract canary (`canary/`), snapshot freshness
  vs `records/`, git-dependency pin check, and (token-gated) the adapter CLI build.
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
