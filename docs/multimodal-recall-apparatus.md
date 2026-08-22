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
Every completed case carries hashes of its request and response plus branch candidate counts,
collapse count, applied oracle regions, reader media count, and retrieved-region fingerprint. A run
is invalid if these facts do not prove the requested arm fired.

Cell D receives gold evidence regions, never the gold answer. `oracle_gold=true` and
`leaderboard_eligible=false` are forced in run provenance. The checked-in held-out apparatus fixture
is also non-official and non-rankable.

## Fixture contract

`membench.multimodal_fixture.v1` stores media as `{locator, sha256, media_type}` pointers. It never
inlines base64 or duplicates source blobs. Derived text is a projection beside the source pointer.
Oracle evidence is typed:

- screenshot: trajectory, state index, locator, optional normalized bounds;
- page: document, one-based page number, optional normalized bounds;
- sheet: workbook, sheet name, A1 range;
- cell: workbook, sheet name, A1 cell.

[`fixtures/multimodal/v1/heldout-recall.json`](../fixtures/multimodal/v1/heldout-recall.json) covers
all four region types across an image-dependent LongMemEval-v2-shaped case, a PDF case, and an XLSX
case. Its source locators are content-addressed placeholders; the text projection control is fully
offline, while native arms require an adapter that can resolve those blobs.

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

`ExecutionBudget::offline()` allows zero provider calls and zero micro-USD. The runner aborts on the
first observed call or cost. No provider-backed phase was run while building this apparatus.

## Symbiotic Memory adapter seam

The adapter must translate `RecallRequest` into public product capabilities:

- content/media ingestion and source-region identity;
- text, native, and hybrid recall selection;
- collapse-to-representative diagnostics;
- transcription-reader or blob-reader materialization;
- oracle region filtering for ceiling runs only.

It returns `RecallResponse` plus `AdapterExecutionProof`. The harness validates identifiers and
regions against the fixture and rejects invented evidence. Product internals remain opaque.

Until the product exposes those public capabilities, only the bundled `TextProjectionBaseline`
runs. Native/hybrid attempts are expected to stop at the preflight capability gap.

## Validation

```bash
CARGO_TARGET_DIR=/tmp/symbiotic-mem-bench-target cargo fmt -- --check
CARGO_TARGET_DIR=/tmp/symbiotic-mem-bench-target cargo test
CARGO_TARGET_DIR=/tmp/symbiotic-mem-bench-target cargo clippy --all-targets -- -D warnings
```
