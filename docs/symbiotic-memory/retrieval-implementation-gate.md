# Retrieval Implementation Gate

Status: required before the next paid full-quality benchmark.

The old task-152 artifacts are evidence, not implementation instructions. Do not copy historical
benchmark code or selector logic into this crate unless the behavior has been reduced to a generic
memory mechanism with tests, diagnostics, and a production-shaped data model.

## Why This Gate Exists

The current crate can ingest, queue, cache, answer, score, and preserve raw source artifacts. That is
infrastructure parity. It is not retrieval parity.

The product spec requires hybrid retrieval, structured evidence, lifecycle filtering, raw fallback,
artifact access, and answer support checks. The current SQLite recall path still uses dense-vector
similarity plus a simple token-hit ratio. That path can miss obvious evidence when a question and a
stored fact use different wording, even when both the distilled fact and raw turn are present in the
vault.

Running a full 500-question quality benchmark before this gap is closed mostly tests an incomplete
retrieval layer and spends tokens on known missing behavior.

## Reuse Rule

Historical benchmark code may be reused only as one of these:

| historical artifact | allowed use |
| --- | --- |
| frozen hypotheses and scores | parity target and regression comparison |
| context dumps and rank traces | failure forensics |
| scripts with guards | reference for run hygiene and no-cheating checks |
| routing or selector rules | inspiration only; must become typed recall policy or evidence logic |
| exact residual phrases, qid rules, gold-aware patches | never allowed |

When a task-152 mechanism appears useful, document:

1. the observed failure it fixed;
2. the generic memory primitive behind it;
3. the production data fields needed;
4. the non-benchmark test that proves it;
5. the benchmark gate that validates it without exact question or answer knowledge.

## Required Retrieval Parity Pieces

These are the minimum pieces needed before another paid full-quality run.

| piece | required behavior | current status |
| --- | --- | --- |
| Recall record shape | Store `search_text`, tags, aliases, entity labels, slot key, source role, lifecycle, and scope metadata. | Missing from `RecallIndexRecord`. |
| Sparse retrieval | Use BM25, FTS, or sparse vectors over fact and raw-turn search text. | Current token-hit ratio is too weak. |
| Hybrid fusion | Fuse dense, sparse, recency, lifecycle, and entity signals with inspectable scores. | Partial dense plus token-hit fusion only. |
| Candidate audit | Emit rank diagnostics for facts and raw turns before answer construction. | Needed to prevent blind top-k tuning. |
| Raw granularity | Benchmark atomic turns against local windows, episode/session cards, and Reweave tags. | Current raw recall indexes one message per unit only. |
| Rerank stage | Rerank a broad candidate pool with a cross-encoder, late-interaction model, or model-backed relevance scorer. | Not implemented in this crate. |
| Derived memory pass | Generate generic tags, aliases, current-state slot notes, count/item summaries, and graph links from source-backed memory. | Not implemented in this crate. |
| Context budgeter | Prefer compact, high-support evidence over raw 80-turn context when possible. | Current raw contexts can exceed 28k input tokens. |
| Backend shadowing | Compare SQLite debug recall with the candidate production backend on fixed fixtures. | zvec 0.4.1 hybrid now builds locally with the patched `zvec-sys` path. |

## Backend Decision

Backends are downstream of retrieval semantics. A backend swap must not be used to hide missing
memory behavior.

| backend | decision |
| --- | --- |
| SQLite | Keep as ledger and debug/parity backend. Add FTS/BM25 or a sidecar sparse index here first because it is inspectable. |
| zvec | First local production candidate using hybrid FTS/vector/scalar filtering, not dense search alone. |
| TurboVec | Experimental compressed vector adapter for scale. It does not replace sparse retrieval, tags, aliases, graph/entity signals, or reranking. |
| Qdrant/Pinecone | Later server/managed adapters behind the same `RecallIndexBackend` contract. |

## Build Order

1. Add evidence-rank audit output for fact and raw-turn retrieval.
2. Run the raw granularity decision experiment in `docs/symbiotic-memory/raw-granularity-decision.md`.
3. Extend `RecallIndexRecord` with production search metadata and raw unit metadata.
4. Implement sparse lexical retrieval using an inspectable local backend.
5. Add hybrid fusion and per-signal score traces.
6. Add a generic reranker interface and one local/provider-backed implementation.
7. Add the derived memory pass for aliases, tags, current-state slots, counts, and graph links.
8. Run fixed-fixture tests for lifecycle filtering, degree/education paraphrase recall,
   current-state recall, count/item recall, unavailable evidence, and raw artifact gating.
9. Run a 10-question provider smoke only to verify the full path still functions.
10. Run a stratified diagnostic slice to inspect paired flips, not to tune rules.
11. Run full 500 only after the above gates pass.

## Promotion Rule

A retrieval improvement may be promoted only when it is:

- generic across wording, languages, and entity names;
- implemented in the memory substrate rather than in benchmark post-processing;
- testable without gold answers;
- traceable in rank audits and context dumps;
- compatible with hosted Symbiotic policy enforcement;
- neutral or positive on a full cleaned 500 run.
