# The adapter boundary — membench treats memory systems as black boxes

Owner principle (2026-07-04): membench is a benchmark **harness**. A memory
system under test is a black box that gets (1) a directory to keep its state
in, (2) conversations to ingest, (3) questions to answer, and exposes
(4) tracing/metrics. The harness must not understand the system's
underpinnings — file names, storage backends, index caches, rebuild
mechanics. Anything the harness knows about internals is coupling that
breaks the moment we bench a second system (mem0, hymem, …) or a second
benchmark (LoCoMo next to LongMemEval).

For distribution, the portable artifact/import contract is also an adapter
boundary. An external implementation can run out of process and import its
normalized artifacts into the public no-feature CLI without linking its source.
The in-process Symbiotic Memory adapter is an optional, access-gated
implementation of the same product boundary. See
[`public-distribution-boundary.md`](public-distribution-boundary.md).

## What the symbiotic-memory adapter may use

Exactly the kit's public facade:

- `Vault::open_with_report(dir, &profile)` — directory + profile in, store
  out. The profile (`storage.backend`, `storage.vector_dimensions`) is
  config plumbing, not internals knowledge: the bench forwards its run
  configuration, the kit decides what to do with it.
- The store's trait surface (`MemoryStore`, `GraphStore`, `FactLifecycle`)
  and the kit's ingest/recall engines.
- `VaultOpenReport` and trace events for metrics. Reading metrics out is
  fine; *acting* on internals is not.

Not allowed, with history:

- **File names inside the vault** — the adapter once renamed
  `memory.sqlite`→`vault.db` in code (`migrate_legacy_vault_layout`,
  removed). Converging old data is a one-time batch operation over the runs
  tree, not harness code.
- **Backend types** — the adapter once matched on
  `SqliteStore | ZvecHybridIndexedSqliteStore` (removed; it holds the
  opaque `VaultStore`).

## Known remaining debt (dies with the hybrid backend) — PAID 2026-07-07

The index-cache logic (`ensure_recall_index`, the per-vault index manifest
with the ledger sha) was DELETED in the §12 step-3 cutover, together with
the kit's maintenance surface it spoke through: the sqlite and zvec-hybrid
backends no longer exist, the collections ARE the store, and consistency on
open is the kit's own job (retire journal + reconcile). `--store` accepts
`zvec` (default) and `memory`; stale run markers naming a deleted backend
refuse loudly. Vault staging copies every `*.zvec` collection directory —
never symlinks them (the engine takes exclusive per-collection locks and may
write on open) — and links only the read-only L0 archive.

## Redesign target (multi-system, multi-benchmark)

The seam to grow toward, so mem0/hymem/others and LoCoMo/LongMemEval/others
compose:

```text
trait MemorySystemAdapter {
    prepare(state_dir, run_config) -> System;   // black box gets a home
    ingest(conversation) -> IngestReceipt;      // + optional maintenance tick
    recall(question) -> Answer + Evidence;      // what gets judged
    trace() -> impl Iterator<TraceEvent>;       // tokens, latency, stages
}
```

- One implementation per system (symbiotic-memory today; mem0 via its HTTP
  API; hymem via its SDK) behind cargo features — a system with no notion
  of "vaults" still fits, because the harness only ever handed over a
  directory and conversations.
- Benchmarks are data-plane modules (question sets + gold + judge prompts)
  that feed any adapter; scoring, registry, and the dashboard stay shared.
- The judge/scorer must not assume adapter internals either (it already
  operates on answers + evidence text only — keep it that way).

Today's `symbiotic_memory_adapter.rs` is the only implementation and should
keep drifting toward this shape whenever it is touched; carving the trait
out is worth doing at the moment a second system or second benchmark
actually lands.
