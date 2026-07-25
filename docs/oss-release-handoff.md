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
   Consequence for CI: the `adapter-build` job runs only where an `ADAPTER_DEPS_TOKEN`
   secret is configured, so on a public fork the documented `membench` CLI is **not**
   verified by CI. `scripts/check-adapter-build.sh` is the mandatory manual release gate
   until this is resolved (`RELEASING.md`).
2. ~~**Adapter APIs not yet published upstream.**~~ **Resolved 2026-07-24.** The kit APIs the
   adapter needs *are* on the pinned revision — they were renamed: what membench called
   `symbiotic_memory::MemoryConfig` (YAML `providers:` role bindings,
   `queue.resolve_provider_queue`) is `symbiotic_memory::EngineConfig` upstream, while
   `MemoryConfig` now names the newer layered TOML config in `symbiotic-memory-config`.
   `--features symbiotic-memory-adapter` builds and runs against the exact pins with no
   sibling checkout and no `.cargo/config.toml` override. The override block in
   `docs/environment.md` remains available for co-development, not as a requirement.
3. ~~**Mandatory opposite-model release approval.**~~ **Resolved 2026-07-24.** K3 independently
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
