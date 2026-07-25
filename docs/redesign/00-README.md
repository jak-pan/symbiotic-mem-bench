# membench v2 — Redesign Sprint

A greenfield redesign of membench from "the symbiotic-memory harness" into an **agnostic
benchmark + debug cockpit** for any memory / hybrid / multi-prong / agentic system, across
any benchmark (LongMemEval today; multimodal + others next).

Read in this order:

| # | Doc | What it is |
|---|-----|------------|
| 00 | **This file** | Map + the decisions that are locked |
| 05 | [05-product-and-data-model.md](05-product-and-data-model.md) | **The capstone**: product thesis, 4-workspace IA, the unified data model, migration |
| 01 | [01-status-quo.md](01-status-quo.md) | Definitive ground-truth of the current system (evidence-anchored) |
| 02 | [02-adapter-agnostic-core.md](02-adapter-agnostic-core.md) | AdapterCapabilityManifest, generic StepEnvelope, composable tuning |
| 03 | [03-best-practices.md](03-best-practices.md) | Observability schema, transport, eval-product patterns, IA/UX |
| 04 | [04-benchmark-multimodal-core.md](04-benchmark-multimodal-core.md) | BenchmarkManifest, (adapter × benchmark) matrix, Content spine |
| 06 | [06-v2-multimodal-recall-experiment.md](06-v2-multimodal-recall-experiment.md) | **The experiment on 04's plumbing**: LongMemEval-v2 lanes (text-projection × native-image), cell matrix (A/B/C/D/E), locked free model slots, collapse rule, read-time router, phase ladder |
| — | [notes/redesign-requirements.md](notes/redesign-requirements.md) | The hard requirements from the user |

Hi-fi prototypes live in [`prototype/`](prototype/) — a clickable terminal cockpit
(`index.html`) plus per-screen files (`runs.html`, `leaderboard.html`, `lab.html`,
`catalog.html`, `steptrace.html`) sharing `kit.css`.

## Decisions locked

1. **Two genericity axes, resolved by manifests.** `(adapter × benchmark)` decides live
   knobs, active components, modalities. Adapter and benchmark each ship a typed manifest;
   the host hardcodes the envelope + renderers, never the content (the LSP/CRD/OTel move).
2. **Source-of-truth ≠ views.** One generic append-only `StepEnvelope` (uniform header +
   typed `oneof` payload + `Link` DAG) is the ledger. Every waterfall/coverage/cost/leaderboard
   is a materialized view with a provenance header, invalidated by source-hash. The current
   `debugger.proto` demotes to one family of views.
3. **Four persona workspaces:** `F1 LEADERBOARD` (evaluator) · `F2 RUNS` (engineer) ·
   `F3 LAB` (operator) · `F4 CATALOG` (the genericity spine). URL-addressed; every tile a
   filtered drilldown.
4. **The differentiator:** per-question gold-evidence **coverage matrix** + the 4-lane
   **recall drop-out** view + **oracle-vs-live** — the axis the observability market can't render.
5. **Aesthetic:** keep the dense Bloomberg terminal for engineer/operator; add a calm
   **Report** projection for the buyer.
6. **Transport:** length-delimited protobuf frames; Connect server-stream for live tailing,
   cursor-paged fetch by record-type for history. No double-encoding, no JSON-in-string.
