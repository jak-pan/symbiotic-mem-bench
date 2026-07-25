# Canonical Record Storage Task

## Status

Portable promotion complete. `factconsol-thinkon-500-20260624` is committed under `records/`
and is the verified rank-1 LongMemEval-S row at 437/500 (`0.874` overall accuracy; `0.888012499`
task-averaged accuracy). Its portable artifacts, question-ID joins, provider-trace provenance,
recorded hashes, public hygiene, and independent `review.json` attestation have been validated.
The deterministic leaderboard snapshot matches `records/` and publishes this row with no
eligibility failures.

The mandatory opposite-model/K3 release review approved the candidate on 2026-07-24 after
independently rebuilding the adapter, recounting all 500 verdicts, verifying the recorded hashes
and public-hygiene scan, reproducing the traversal refusal, and running the full 142-test suite.
Retaining the much larger native-state substrate externally is optional and is not required for
the public record or leaderboard.

## Problem

`records/` is the tracked, portable registry for curated benchmark results, while full native
Symbiotic Memory runs can be too large for GitHub when they include `vaults/`, provider queues, raw
debug files, and trace-heavy state. The canonical portable record now supports public comparison.
An external ingested vault substrate would additionally support answer-only reruns, but retaining
one is an optional follow-up.

## Goal

The public, portable goal is complete: one selected LongMemEval-S run is promoted as a canonical
record while keeping the repository bounded and reproducible.

The committed record provides:

- normalized public artifacts in the existing `records/{system}/{benchmark}/{limit}/{name}/` shape;
- artifact hashes, byte sizes, score provenance, and independent review metadata;
- dashboard compatibility through the existing record and run schema;
- no dependency on untracked native state for scoring, review, or ranking.

If native state is retained later, the optional follow-up should provide an exact external bundle
identity, hashes, byte sizes, restore instructions, and a reusable `source_vault_root`.

## Actual Tracked Shape

Tracked in Git:

```text
records/symbiotic-memory/long-mem-eval/500/{record-name}/
  run-params.json
  benchmark-report.json
  review.json            # membench.record_review.v1 — required before the record can rank
  artifacts/
    hypotheses.jsonl
    scored.json
    verdicts.jsonl
    partial-verdicts.jsonl
    provenance.jsonl
    score-summary.json
    memory-traces.jsonl
    model-traces.jsonl
    step-analytics.json
```

The promoted record intentionally has no `external-artifacts.json`: no external native-state
bundle is currently part of the release evidence.

An optional future external bundle could use this shape:

```text
native-state.tar.zst
  vaults/
  workflow/
  provider-queue/
  raw/
```

If created, a tracked `external-artifacts.json` should include:

- storage provider and URL or object key;
- SHA-256 digest and byte size;
- compression format;
- source run id;
- created timestamp;
- restore target path;
- whether the bundle contains raw prompts or local-only debug data;
- expected `source_vault_root` after restore.

## Acceptance Criteria

- [x] The promoted record passes `src/eligibility.rs` — it appears in a ranked cohort of
  `cargo run --bin membench-leaderboard -- export --records-root records`, not in `unranked`.
  That requires the scoring artifacts on disk, provider traces, a full-scale question count,
  recorded cohort identity, and a `review.json` attestation written *after* an independent
  no-cheating review (`docs/longmemeval-methodology.md`).
- [x] `membench explore` can show the tracked record without downloading external state.
- [x] Dashboard can compare the tracked record to new runs from `artifacts/`.
- [x] The record contains no secrets, `.env` content, unredacted provider credentials, absolute
  local paths, symlinks, or forbidden native-state directories under the bounded public scan.
- [x] The committed snapshot is deterministic, matches `records/`, and publishes the record as
  verified rank 1 at 437/500.
- [ ] **Optional, only if native state is retained:** a documented restore command can place
  native state under ignored local storage.
- [ ] **Optional, only if native state is retained:** a documented answer-only command can use
  the restored vault root with `--source-vault-root`.
- [ ] **Optional, only if native state is retained:** if raw prompts or raw source data are
  included externally, the manifest marks the bundle
  local/private and not suitable for public mirroring.

## Remaining Decisions

- External storage target: release asset, object storage bucket, Hugging Face dataset, or other.
- Whether the full native bundle should include raw model prompts or only normalized traces.
- Whether `save-record` should gain a `--external-state` mode that writes
  `external-artifacts.json` automatically.
- Whether restored vault substrates should live under `runs/inputs/vault-roots/` or a separate
  ignored `state/` tree.

The four native-state decisions above are optional unless answer-only reuse of this exact substrate
becomes a release requirement.

## Completion Ledger

1. [x] Complete the no-cheating and public-hygiene review of the selected run.
2. [x] Validate the safe-by-default portable `save-record`; native state is copied only with
   `--include-native-state`.
3. [x] Promote the selected record to `records/` and commit its review attestation.
4. [x] Export and independently verify the ranked leaderboard snapshot.
5. [x] Obtain the mandatory opposite-model/K3 release approval.
6. [ ] **Optional:** add an `external-artifacts.json` schema if the native vault substrate will
   be retained for answer-only reuse.
7. [ ] **Optional:** add a restore/check command or documented script for any retained external
   substrate.
8. [ ] **Optional:** upload the native-state bundle, verify hashes, and run an answer-only
   restore smoke.
