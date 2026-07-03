# Sweep methodology

Rules for designing benchmark sweeps so arm deltas are attributable to the
knob under test. Established 2026-07-04 during the distill-windowing sweep;
owner-directed.

## Variance control comes before cost

Distillation (and reweave) are nondeterministic LLM phases. Two fresh ingests
at identical settings do NOT share a fact base — re-rolling distill inside a
comparison confounds the treatment with re-roll noise.

- **Fresh re-ingest** is correct only for arms whose treatment changes ingest
  inputs themselves: windowing mode (`distill.window_boundary`,
  `turns_per_window`), distill prompt, embedding model/dims.
- **Post-distill treatments** build from the SAME base vault:
  - reweave / re-embed / re-index: `SYMEM_REDO=<stage>`
    `--source-vault-root <base>/vaults` — invalidates just that manifest
    stage, reuses every valid upstream stage, keeps the fact base
    byte-identical;
  - recall/answer knobs (collapse, keep-facts, raw-only, rerank tuning):
    `--answer-only --source-vault-root <base>/vaults`. Answer-only runs over
    vaults built WITHOUT a consolidator need `--no-consolidate-briefs`, or
    the vault-completeness gate rejects every question.
- Prefer **within-vault comparisons** (answer arms over one vault) over
  cross-vault ones. Cross-vault deltas of 1–3 questions at n=50 are noise;
  patterns consistent across every vault (column-consistent) are signal.

The first windowing sweep violated this for its reweave arms (fresh ingests);
their −2..−6pt readings were unattributable and had to be re-measured with
redo-built vaults (`w50-*-rw2`).

## Config identity is the arm definition

Arms are declared as `SYMBIOTIC_MEMORY__*` config overrides (env-file or
process env); the bench resolves them through the kit's typed config and
records the hash + per-key provenance in `run-params.json` (`kit_config`).
Two runs with equal hashes ran the same kit configuration — cite the hash
when comparing runs. Harness-side toggles (`SYMEM_CONSOLIDATOR`,
`SYMEM_REDO`, `--memory-config` recall profiles) are recorded in run-params
alongside.

## Reference sweep

`scripts/run-windowing-sweep.sh` (vault matrix + answer arms) and
`scripts/run-windowing-sweep-answer-arms.sh` (idempotent answer-arm phase)
encode these rules; `scripts/score-run.sh <runs...>` prints the comparison
table.
