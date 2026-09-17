# The adapter boundary — memory systems are black boxes

Membench supplies a state directory, source records, questions, and an
explicit diagnostics policy. A memory-system adapter returns capture results,
answers, typed diagnostics, and trace events. The harness must not know how the
system stores, indexes, checkpoints, or rebuilds those values.

## Symbiotic Memory contract

The Symbiotic Memory adapter may depend on these public surfaces:

- `MemoryEngine`, `MemorySession`, their request/result contracts, capability
  negotiation, and typed errors;
- public source, scope, answer, and evidence DTOs used by that facade;
- public provider/configuration adapters needed to supply the engine's host
  seams;
- facade-owned structured/full diagnostic results and instrumentation events.

The application profile is consumed through `symbiotic_memory::profile`.
Membench must not depend directly on Symbiotic Memory's implementation config
crate.

The adapter must not import or construct:

- `IngestPipeline`, `RecallEngine`, a storage trait/backend, or `VaultStore`;
- zvec collections, SQLite files, archive internals, or manifest checkpoints;
- an alternate ingestion, embedding, retrieval, reranking, or answer path.

If a benchmark mode needs a capability the facade does not expose, the adapter
fails before ingest or provider calls. It does not approximate the missing
behavior. In particular, a second harness-side search is not a substitute for
the evidence used by native recall.

## Diagnostics and truth tiers

The benchmark may request the facade's protected full diagnostics for local
question-debug bundles. It may also consume structured diagnostics for safer
publication artifacts. Raw prompts, model responses, and reasoning remain
ignored local artifacts and are never copied into tracked records.

Evidence returned by Memory retains the facade's authority labels:

- raw source content is canonical source evidence;
- facts, plans, rankings, and summaries are rebuildable derived values;
- answer text is generated output.

Membench renders these values but does not reinterpret their authority.

## State reuse and maintenance

Answer-only reuse, redo stages, consolidation, supersession detection, and
diagnostic ingest stops are available only when Memory exposes them through a
public operation or capability. The harness never copies named index files,
edits manifests, or calls store maintenance methods to implement those modes.

## Multi-system adapter shape

Every system-specific adapter implements the same conceptual seam:

```text
open(state_dir, run_config) -> system handle + capabilities
ingest(source, operation_context) -> capture result
recall(question, diagnostics, operation_context) -> answer + evidence
trace() -> typed event stream
```

A system with no vault, vector store, or local process still fits this shape.
Benchmark loading, scoring, records, comparisons, and dashboard rendering stay
system-neutral.
