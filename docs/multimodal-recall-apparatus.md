# Multimodal recall apparatus

Status: executable offline contract; native Symbiotic Memory capability not wired yet.

This apparatus turns the design in
[`redesign/06-v2-multimodal-recall-experiment.md`](redesign/06-v2-multimodal-recall-experiment.md)
into provider-neutral benchmark code. The executable contract is
[`src/multimodal.rs`](../src/multimodal.rs). It does not copy Symbiotic Memory parsing, storage,
embedding, or recall logic. A product adapter must implement `MultimodalRecallAdapter` and advertise
its actual capabilities.

## Experiment cells

| Cell | Retrieval | Reader | Interpretation |
|---|---|---|---|
| A | text projection only | text | deterministic no-network control |
| B | native media only | text projection of hit | native retrieval treatment |
| C | text plus native, collapsed by source region | text | product-shaped hybrid arm |
| D | gold evidence region selects text/native/hybrid | text | oracle ceiling; always non-rankable |
| E | cell C retrieval | transcription versus source blob | isolates reader modality |

The runner preflights the complete fixture before the first adapter call. A text-only adapter asked
to run B, C, D, or blob-reader E returns a named capability gap; it cannot silently flatten media.
Every completed case carries the actual per-branch candidates, product-shaped captured pointers,
collapse clusters, reader inputs, request/response hashes, and fingerprints. The harness recomputes
branch, collapse, projection, and source-blob invariants. Reader bytes can enter the adapter only
through the harness-supplied binding resolver, which journals the authorized binding, exact byte
count, and digest; adapter-owned counters or self-attested blob claims are not evidence.

Cell D receives gold evidence regions, never the gold answer. `oracle_gold=true` and
`leaderboard_eligible=false` are forced in run provenance. The checked-in held-out apparatus fixture
is also non-official and non-rankable.

## Fixture contract

`membench.multimodal_fixture.v1` stores import-only, dataset-relative media paths plus SHA-256,
byte length, and media type. Before the adapter runs, the harness rejects missing, linked,
out-of-root, over-limit, size-mismatched, or digest-mismatched bytes. `max_import_asset_bytes` is a
required positive per-asset ceiling. The adapter imports those bytes once and
returns product-compatible binding, blob, region, projection, truth-tier, and retrieval metadata.
Captured source artifacts retain the raw source binding/blob and raw truth tier. Text branch hits
carry the registered projection output binding/blob and deterministic-projection truth tier; native
hits carry the source identity. Hybrid collapse therefore has two real product-shaped inputs.
The harness passes exact verified bytes—not filesystem paths—across the import seam. Only the
captured binding is read authority after import; paths and digests are not.
Oracle evidence is typed:

- screenshot: trajectory, state index, locator, optional normalized bounds;
- page: document, one-based page number, optional normalized bounds;
- sheet: workbook, sheet name, A1 range;
- cell: workbook, sheet name, A1 cell.

[`fixtures/multimodal/v1/heldout-recall.json`](../fixtures/multimodal/v1/heldout-recall.json) covers
all four region types across an image-dependent LongMemEval-v2-shaped case, a PDF case, and a CSV
case. The fixture includes local SVG, PDF, and CSV source assets plus real distractors. Each case
queries a full corpus; oracle annotations are stored separately and enter requests only for cell D.
The checked source digest is recomputed from the sorted relative asset names and exact bytes.

[`product-contract-snapshot.json`](../fixtures/multimodal/v1/product-contract-snapshot.json) was
serialized by Symbiotic Memory commit `12a7a57ad00f75eb8afd29700ec5a51b55e349a6` using the real
public `ArtifactEvidence` types and `collapse_artifact_evidence`. It records the generating product
source hashes, an actual wire specimen, and adversarial collapse vectors. The drift test pins the
snapshot bytes, product revision/source hashes, wire decode, and parity results, including `Whole`,
transitive ancestry, sheet/range, `B2:C4` versus `$C$4`, disjoint A1 ranges, rectangle IoU on both
sides of the 0.5 boundary, time/byte overlap thresholds, named-locator equality, and distinct
bindings. This replaces a bench-authored specimen. The
benchmark envelope keeps its evidence ID and verified projection-output blob metadata outside that
product wire object.

The official LongMemEval-v2 loader is annotation-driven:

1. A human-reviewed `membench.longmemeval_v2_image_annotations.v1` file declares the
   image-dependent subset, rationale, oracle lane, and screenshot state regions.
2. `load_longmemeval_v2_image_subset` checks each trajectory belongs to the question haystack,
   checks state indices and domains, reads and hashes the question/trajectory images, and preserves
   the accessibility tree as the text projection.
3. Official evaluator strings remain `ScoringRule::External`. A missing upstream scorer fails
   explicitly; core never substitutes a convenient local grader.

This avoids the invalid shortcut of treating every question with an image path as image-dependent.
The released dataset does not supply gold evidence labels, so the reviewed annotation is required.

## Cost ladder

| Step | Allowed work | Promotion gate |
|---|---|---|
| `validate_apparatus` | schema, hash, capability, scorer checks | all fixtures fail closed correctly |
| `projection_control` | deterministic lexical cell A | request/response reproducible, $0 |
| `offline_native` | local or stored-output B–E adapter | fired proofs differ as preregistered, $0 |
| `provider_pilot` | smallest image/PDF/sheet pilot | explicit call and USD budget |
| `stratified_medium` | diverse subset | pilot improves outside the control floor |
| `full_benchmark` | complete comparable run | only after prior gate and variance plan |

`ExecutionBudget::offline()` allows zero provider calls and zero micro-USD. Every non-provider
cost-ladder step categorically rejects nonzero provider maxima before descriptor, import, or recall,
and its `SpendJournal` rejects reservation even if constructed with numeric capacity. Every provider
call must first obtain a reservation from the harness-owned journal. The journal appends and fsyncs
the reservation before returning control to the adapter, then requires a terminal spend event.
Terminal events append and fsync before the open reservation is removed; a persistence failure
leaves it open so finalization can record a failed event at the reserved ceiling.
When recall returns with an unfinished reservation, the harness durably closes it as failed at the
reserved ceiling before propagating the error; missing usage is never interpreted as zero. No
provider-backed phase was run while building this apparatus. Every ledger event includes the unique
run-instance ID and effective-config digest so concurrent or repeated runs cannot merge provenance.
Before any adapter work, the runner claims that identity in a harness-owned registry using
create-new semantics; a duplicate run instance fails closed.

Cell E uses `run_reader_modality_pair`: C's retrieval and collapse output is frozen once and its
serialized artifact must remain byte-identical for the text-projection and source-blob readers.
The text arm must resolve the registered projection bytes and the blob arm must resolve the raw
source bytes through the journaled resolver.

## Symbiotic Memory adapter seam

The adapter must translate `RecallRequest` into public product capabilities:

- content/media ingestion and source-region identity;
- text, native, and hybrid recall selection;
- collapse-to-representative diagnostics;
- transcription-reader or blob-reader materialization;
- oracle region filtering for ceiling runs only.

It returns `RecallResponse` plus evidence-bearing `AdapterExecutionProof`. The harness validates
captured bindings, blob hashes, projection lineage, branch membership, and the exact product
collapse result (including merged truth tier, pointer, region, and retrieval scores). Reader proof
comes only from the harness resolver and verified fixture. Product internals remain opaque.

All results from this pre-release apparatus are categorically non-rankable. Promotion requires a
canonical record that passes the repository's normal eligibility, trace, full-scale, artifact-hash,
and independent-review gates. The fixture schema has no ranking-eligibility claim to self-declare.

Until the product exposes those public capabilities, only the bundled `TextProjectionBaseline`
runs. Native/hybrid attempts are expected to stop at the preflight capability gap.

## Validation

```bash
CARGO_TARGET_DIR=/tmp/symbiotic-mem-bench-target cargo fmt -- --check
CARGO_TARGET_DIR=/tmp/symbiotic-mem-bench-target cargo test
CARGO_TARGET_DIR=/tmp/symbiotic-mem-bench-target cargo clippy --all-targets -- -D warnings
```
