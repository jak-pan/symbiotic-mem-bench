# Canonical Record Storage Task

## Status

Planned. Do not implement until a full 500-question run is selected for promotion.

## Problem

`records/` is the tracked, portable registry for curated benchmark results, but full native
Symbiotic Memory runs can be too large for GitHub when they include `vaults/`, provider queues, raw
debug files, and trace-heavy state. We still need a canonical record that future benchmark runs can
compare against and a canonical ingested vault substrate that answer-only reruns can reuse.

## Goal

Promote one selected LongMemEval S run into a canonical record while keeping the repository small and
reproducible.

The promoted record should provide:

- normalized public artifacts in the existing `records/{system}/{benchmark}/{limit}/{name}/` shape;
- enough metadata to identify the exact external native-state bundle;
- a reusable `source_vault_root` location for answer-only reruns;
- hashes, byte sizes, and restore instructions for external artifacts;
- dashboard compatibility through the existing record and run schema.

## Proposed Shape

Track this in Git:

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
  external-artifacts.json
```

Store externally:

```text
native-state.tar.zst
  vaults/
  workflow/
  provider-queue/
  raw/
```

The tracked `external-artifacts.json` should include:

- storage provider and URL or object key;
- SHA-256 digest and byte size;
- compression format;
- source run id;
- created timestamp;
- restore target path;
- whether the bundle contains raw prompts or local-only debug data;
- expected `source_vault_root` after restore.

## Acceptance Criteria

- The promoted record passes `src/eligibility.rs` — i.e. it appears in a ranked cohort of
  `cargo run --bin membench-leaderboard -- export --records-root records`, not in `unranked`.
  That requires the scoring artifacts on disk, provider traces, a full-scale question count,
  recorded cohort identity, and a `review.json` attestation written *after* an independent
  no-cheating review (`docs/longmemeval-methodology.md`).
- `membench explore` can show the tracked record without downloading external state.
- Dashboard can compare the tracked record to new runs from `artifacts/`.
- A documented restore command can place native state under ignored local storage.
- A documented answer-only command can use the restored vault root with `--source-vault-root`.
- The record contains no secrets, `.env` content, or unredacted raw provider credentials.
- If raw prompts or raw source data are included externally, the manifest marks the bundle
  local/private and not suitable for public mirroring.

## Open Decisions

- External storage target: release asset, object storage bucket, Hugging Face dataset, or other.
- Whether the full native bundle should include raw model prompts or only normalized traces.
- Whether `save-record` should gain a `--external-state` mode that writes
  `external-artifacts.json` automatically.
- Whether restored vault substrates should live under `runs/inputs/vault-roots/` or a separate
  ignored `state/` tree.

## Suggested Implementation Steps

1. Pick the canonical 500-question run only after quality and no-cheating review.
2. Add an `external-artifacts.json` schema to `docs/schemas.md`.
3. Extend `save-record` with optional external-state manifest support.
4. Add a restore/check command or documented script.
5. Promote the selected record to `records/`.
6. Upload the native-state bundle externally and verify hashes.
7. Run one answer-only smoke using the restored `source_vault_root`.
