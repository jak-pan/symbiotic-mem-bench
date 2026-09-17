# DeepSeek cost accounting

The June-only fallback table underpriced newer calls and did not recognize
`deepseek-flash` or `deepseek-v4-flash-vision-exp`. Missing cache counters became
cache misses, and replayed responses could charge their original cost a second
time. The rollup now separates those cases without changing saved run evidence.

## Rates and time

`official-pricing-2026-09-17` is a versioned estimator, not an invoice. It selects
the native DeepSeek tariff using a valid provider `created` Unix timestamp
when present, otherwise the trace's RFC 3339 timestamp converted to UTC. A
provider timestamp outside the representable calendar range is unusable, so the
trace timestamp is the fallback. Trace time can be completion time; neither
fallback time nor provider creation time proves an invoice's exact billing
instant at a tariff boundary. Provider-reported cost takes precedence.

Pricing uses provider `served_model` when present, retaining requested
model identity for report grouping. An unknown returned model stays unpriced
without a reported cost; it is never silently priced as the requested Flash
model. Both Memory `usage.provider` and normalized Foundation `metadata.provider`
are supported. Their complementary optional fields can be combined only when
overlapping values agree. A conflict in response identity, served model, creation
time, reasoning count, or reported cost leaves the call unpriced across all cost
sources; local response replay still has zero new cost. Older traces without
metadata keep their original model/time fallback.

| Interval/model | Uncached input / M | Cached input / M | Output / M |
| --- | ---: | ---: | ---: |
| V4 Flash: June 23 until August 16 16:00 UTC (exclusive) | $0.14 | $0.0028 | $0.28 |
| V4 Pro: June 23 until August 16 16:00 UTC (exclusive) | $0.435 | $0.003625 | $0.87 |
| Flash from September 10 04:00 UTC, peak | $0.30 | $0.006 | $1.20 |
| Flash from September 10 04:00 UTC, off-peak | $0.15 | $0.003 | $0.60 |

Dates are in 2026. Peak means Monday–Friday, 01:00–04:00 and 06:00–10:00 UTC;
the right boundary is excluded. Weekends and all other hours are off-peak.
Current Flash rates cover `deepseek-flash`, `deepseek-v4-flash`, and
`deepseek-v4-flash-vision-exp`. Historical June rates only cover the original V4
Flash and Pro identifiers; new identifiers are not retroactively priced.
The June snapshot is eligible from June 23 00:00 UTC, the earliest retained
calendar-date snapshot, not an asserted launch or tariff-change instant. Earlier
traces remain unpriced unless they carry a provider-reported cost.

The prior V4 tariff changed on August 16. Its intervening USD snapshot is not in
this catalog, so that interval is explicitly unpriced. Pro calls after that date
also remain unpriced: Pro must not be silently relabeled Flash. Missing or invalid
timestamps and unknown model names remain unpriced unless the provider gave a
cost. The catalog records the June snapshot as historical evidence, rather than
claiming its price still applies today.

Sources checked September 17, 2026:

- [Current official prices and peak hours](https://api-docs.deepseek.com/quick_start/pricing/).
- [September 10 announcement: exact Flash pricing transition](https://api-docs.deepseek.com/news/news260910/).
- [August announcement: preceding tariff transition](https://api-docs.deepseek.com/news/news260813/).
- [Updated change log: Pro continues separately](https://api-docs.deepseek.com/updates/).

## Cache and incomplete accounting

Provider prefix caching discounts input tokens. An explicit hit count or miss
count plus the input total is sufficient to derive the other bucket. State-only
`prompt_cache` labels are ignored because older normalized trace emitters derive
`miss` from absent counters; numeric counters are required. Missing or
contradictory counters stay unknown; they do not increment prompt-cache misses.
`unknown_cache_input_tokens` exposes the unknown volume. A cache-discounted
estimate requires a trustworthy split and complete token usage.

Local response replay is marked by shared Memory `usage.response_cache_hit: true`
or normalized `cache.response_cache: "hit"`. It makes no new provider call, so its new cost is zero even
when the saved usage contains an old provider-reported cost. For compatibility,
token totals and cache counters still describe all trace invocations, including
saved usage on local replays; they must not be read as newly billed token totals.

`unpriced_calls` is exposed at run, model, and role levels. If any call in a group
is unpriced, that group's `cost_micro_usd` is null, rather than a misleading sum
of only the priced calls. Reported costs retain precedence over missing usage or
tariffs. Estimates retain their tariff source and the catalog version. Top-level
`cost_micro_usd` on provider-queue rows is a producer estimate from configured
rates, not a provider receipt. The rollup recalculates it from timestamp and
numeric token/cache counters; unsupported periods or incomplete evidence stay
unpriced. Usage-level reported costs retain precedence. The optional
`usage.provider.reported_cost_usd` or normalized `metadata.provider.reported_cost_usd`
decimal string is accepted as a provider
receipt after validation of finite, nonnegative, representable micro-USD. Values
are rounded to the nearest micro-dollar. Source precedence is: local response
replay zero; coherent existing `usage.cost_micro_usd`; provider decimal receipt; legacy
non-queue top-level cost; dated estimate. The shared adapter populates provider
receipt metadata only from the provider's reported cost, never a queue estimate.

## Verification and handoff — September 17, 2026

This is a supporting cost-accounting fix for the RabbitHole investigation, not a
benchmark campaign. It touches `src/cost.rs`, the README cost section, and this
document only. No provider calls, dataset exports, or saved-run changes were made.

Four new regression tests first failed against the previous implementation.
After the fix, `cargo test --release --offline` passed all 48 core tests, including
13 cost tests for tariff boundaries, aliases, historical preservation, missing
timestamps/usage/cache, inconsistent counters, reported costs, response replay,
and incomplete aggregate totals. `rustfmt --edition 2024 --check src/cost.rs`
and `git diff --check` passed.

Full repository formatting is blocked by the existing manifest reference to the
absent `src/bin/trace_pb_bench.rs` and unrelated formatting drift. Strict library
Clippy remains blocked by pre-existing warnings outside `src/cost.rs` in
`live.rs`, `registry.rs`, and `runner.rs`; the changed module is warning-free.
The checkout's existing lockfile also requires refresh against current local
optional path dependencies. Validation used Cargo's local refresh without
including that unrelated lockfile change in this patch. This initial qualification
covered core tests; feature-check follow-up is recorded below. Deployment is not
claimed.

The parent task owns delivery/review of this work branch. No main-branch merge or
production deployment is part of this lane.


## Review follow-up — September 17, 2026

Historical pricing now has an explicit lower coverage boundary at the June 23
snapshot date. A regression test first reproduced the incorrect January pricing,
then passed with dates before that snapshot left unpriced. The synthetic queue
fixture now uses a date inside the supported historical interval.

All `ModelStat`, `RoleStat`, and `ModelTraceRollup` constructors were checked in
both the clean branch and the ongoing dashboard checkout. The server/CLI on this
branch serialize the rollup directly, so no constructor changes are needed.
The ongoing protobuf dashboard constructs separate generated `pb::ModelStat`
and `pb::ModelRollup` types from JSON; those types remain source-compatible and
retain null cost, but their existing schema does not yet transport the new
unknown-token/unpriced-call fields. Updating that unrelated in-progress schema
belongs to its dashboard integration lane.

`cargo check --release --offline --features server --bin membench-server` passed.
The optional adapter check reached source and failed on unchanged API drift in
`src/symbiotic_memory_adapter.rs`: removed `IngestDiagnosticMode`/`IngestPipeline`
imports, removed `MemoryArchiveWriter`, and an obsolete third argument to
`detect_and_apply_supersessions`. None of those files or dependency APIs were
changed here. No constructor error was found. The final core suite passed all
49 tests (14 cost tests), and changed-file formatting/diff checks passed.


## Producer-provenance review — September 17, 2026

Two additional real-shape regressions first reproduced misleading certainty:
normalized traces labeled absent counters as `miss`, and queue traces supplied a
top-level cost calculated from a potentially stale configured tariff. Cache
splits now require numeric counters, and queue-level cost estimates are
recalculated using this catalog instead of being marked provider-reported.
Usage-level reported cost and zero new cost for local response replay retain
precedence. The final core suite passed all 51 tests, including 16 cost tests.
Changed-file formatting and diff checks passed. No provider call was needed.


## Shared provider metadata — September 17, 2026

The cost parser now reads optional `Usage.provider.reported_cost_usd` emitted by
the shared Memory adapter. Two regressions first failed before the parser change;
all 53 core tests (18 cost tests) then passed. They verify metadata-only receipts
without token usage, rounding, malformed/non-finite/negative/out-of-range values,
existing micro-USD precedence, and replay zero. Missing optional metadata leaves
old traces compatible. No paid API call was made.


## Returned-model and time provenance — September 17, 2026

Five regressions reproduced incorrect requested-model pricing after a returned
model mismatch, incorrect completion-time tariffs, dropped normalized Foundation
metadata, ignored complementary fields, and masked conflicts. All now pass using
the documented metadata reconciliation and precedence. The final core suite
passed 58 tests, including 23 cost tests; no provider call was made. Coherent
reported receipts continue to override estimation even when the served model is
outside this catalog.


## Shared replay marker — September 17, 2026

The settled shared adapter contract adds `Usage.response_cache_hit`, default false
and omitted when false. The parser treats it like the existing normalized
response-cache marker, counts it at run/model/role levels, and preserves original
usage while recording zero new cost. A queue-shaped regression first showed the
old provider receipt being charged again, then passed. All 59 core tests (24 cost
tests) passed. No additional top-level cache flag is defined by the active shared
queue helper, so no unverified field is treated as a free replay.
