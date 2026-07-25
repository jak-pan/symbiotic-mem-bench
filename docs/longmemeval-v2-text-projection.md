# LongMemEval-V2 text projection

`longmemeval-v2-text` is an experimental, non-official projection of
[LongMemEval-V2](https://github.com/xiaowu0162/LongMemEval-V2). It exists so the current
text-only Symbiotic Memory adapter can exercise the released trajectory corpus safely while the
multimodal adapter boundary is built.

It is **experimental and never an official LongMemEval-V2 tier** — numbers from this lane must
not be reported as official LongMemEval-V2 results:

- questions with a non-null query `image` are excluded;
- trajectory screenshot locators are preserved in projected turns, but image bytes are not passed
  to memory or recall;
- run parameters record `official_equivalent: false`, `leaderboard_eligible: false`, the projection
  version, selected haystack tier, and any trajectory/state caps;
- the ranking gate rejects these records even if someone later adds a review attestation;
- `--benchmark longmemeval-v2` is intentionally unregistered so an official-looking score cannot
  silently use this projection.

The official harness calls memory with full trajectories and supplies the optional query image.
Use the upstream
[`evaluation/harness.py`](https://github.com/xiaowu0162/LongMemEval-V2/blob/main/evaluation/harness.py)
for official Small or Medium claims. The deterministic evaluator implementation follows the
released
[`evaluation/qa_eval_metrics.py`](https://github.com/xiaowu0162/LongMemEval-V2/blob/main/evaluation/qa_eval_metrics.py);
LLM checker heads remain deliberately unsupported here.

## Obtain the OSS dataset

Clone the upstream Apache-2.0 repository, then follow its data pipeline:

```sh
git clone https://github.com/xiaowu0162/LongMemEval-V2.git
cd LongMemEval-V2
python data/download_data.py --data-root data/longmemeval-v2
export DATA_ROOT="$(pwd)/data/longmemeval-v2"
python data/prepare_data.py --data-root "$DATA_ROOT" --mode symlink
python data/validate_data.py --data-root "$DATA_ROOT" --tier small
```

Point membench at the prepared directory containing `questions.jsonl`, `trajectories.jsonl`, and
`haystacks/`. Membench validates the selected rows, evaluator specifications, exact ordered shared
haystacks, referenced trajectories, and required text/metadata fields before provider setup.

## No-score execution

The full text projection contains evaluator heads that require the official LLM checker. Membench
does not substitute a generic judge. A full `--score` run therefore fails during preflight — before
provider construction, ingest, answering, or any run-root mutation, so an existing score bundle
survives the rejected launch untouched. Run it unscored to inspect recall behavior:

```sh
cargo run --features symbiotic-memory-adapter --bin membench -- \
  --system symbiotic-memory \
  --benchmark longmemeval-v2-text \
  --dataset /path/to/LongMemEval-V2/data/processed \
  --no-score
```

`--no-score` skips scoring only. Ingest, distillation, embedding, and answering still run through
the configured paid providers by default, so a `--no-score` run still spends provider money and
still takes the paid-run lock. Use `--smoke` for a local, no-network, no-cost pipeline check.

For bounded local pipeline checks only, `MEMBENCH_V2_MAX_TRAJ` and
`MEMBENCH_V2_MAX_STATES` accept positive integers. Any capped run remains explicitly non-promotable.
The Medium tier is currently rejected because its question-specific corpora do not match the
adapter's shared-corpus execution model.

## Score bundle publish

A scored run publishes `verdicts.jsonl`, `scored.json`, and `score-summary.json`. Each file is
staged fully, then renamed into place individually, with the hash-binding `score-summary.json`
renamed last as the bundle's commit point. The renames are sequential — the bundle as a whole is
**not** one atomic unit. The guarantee is fail-closed ranking, not atomicity: the eligibility gates
reject a record with a missing or empty scoring artifact, reject a `score-summary.json` whose
recorded hashes no longer match the artifacts on disk, and reject every `longmemeval-v2-text`
record categorically (`leaderboard_eligible: false`), so a partial or torn publish can never become
leaderboard-eligible.
