# Canonical Record Storage Task

## Status

In progress. The selected candidate is
`factconsol-thinkon-500-20260624` (437/500, `0.874` overall accuracy). Its scoring artifacts,
question-ID joins, provider-trace provenance, and recorded hashes passed the first independent
integrity audit. Final promotion still requires portable-record validation, the committed review
attestation, leaderboard export verification, and the mandatory opposite-model review.

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

1. Complete the no-cheating and public-hygiene review of the selected run.
2. Validate the safe-by-default portable `save-record`; native state is copied only with
   `--include-native-state`.
3. Add an `external-artifacts.json` schema if the native vault substrate will be retained for
   answer-only reuse.
4. Add a restore/check command or documented script for any retained external substrate.
5. Promote the selected record to `records/` and commit its review attestation.
6. Export and independently verify the ranked leaderboard snapshot.
7. If retained, upload the native-state bundle, verify hashes, and run an answer-only restore smoke.
