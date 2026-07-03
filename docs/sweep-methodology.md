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
  - reweave / re-embed / re-index: `MEMBENCH_REDO=<stage>`
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
when comparing runs. Harness-side toggles (`MEMBENCH_CONSOLIDATOR`,
`MEMBENCH_REDO`, `--memory-config` recall profiles) are recorded in run-params
alongside.

## Benchmarking derivation invalidation (staged-ingest protocol — NOT built yet)

Supersession-triggered re-derivation (kit docs/DISTILL-PASSES.md,
"Derivation invalidation") cannot be measured by one-shot ingest: reweave
runs after every supersession has already settled, so a brief never sits on
top of a fact that gets superseded LATER. The protocol needs the timeline
staged manually:

1. **Staged ingest mode** (new harness feature): split each question's
   haystack sessions chronologically into N stages. Per stage: ingest →
   reweave → continue. Facts distilled at stage k get superseded by stage
   k+1 sessions; briefs derived at stage k are now stale — exactly the
   production shape (periodic light passes + nightly dreaming over a living
   store).
2. **Flagged-fact assertions, not just accuracy**: the store already flags
   everything needed — `status: Superseded`, `superseded_by` chains, and
   facts-mode briefs cite fact memory_ids in their `source_refs`. After the
   final stage, query memory.sqlite directly and assert per vault:
   (a) *stale-brief exposure*: briefs whose cited facts are superseded and
   which still appear in a question's recall trace / answer context;
   (b) with invalidation ON: those briefs were stale-marked and re-derived
   (fresh distillery_version timestamp, no superseded citations).
   This is deterministic — no judge noise.
3. **Probe questions**: knowledge-update (ku) questions are the natural
   probes (gold value changes across sessions), but the 50q sample has only
   8; use the full ku slice or the hard sets for power, and consider a small
   synthetic update-probe set (controlled value-change session pairs) where
   ground truth for "which fact must be superseded" is known by
   construction.
4. Arms: staged + invalidation OFF (stale briefs persist) vs staged +
   invalidation ON vs one-shot control. The OFF arm quantifies the damage
   the trigger machinery prevents; without it, an accuracy-only comparison
   can't attribute gains.

## Reference sweep

`scripts/run-windowing-sweep.sh` (vault matrix + answer arms) and
`scripts/run-windowing-sweep-answer-arms.sh` (idempotent answer-arm phase)
encode these rules; `scripts/score-run.sh <runs...>` prints the comparison
table.
