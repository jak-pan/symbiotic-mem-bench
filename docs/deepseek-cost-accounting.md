# DeepSeek cost accounting

The June-only fallback table underpriced newer calls and did not recognize
`deepseek-flash` or `deepseek-v4-flash-vision-exp`. Missing cache counters became
cache misses, and replayed responses could charge their original cost a second
time. The rollup now separates those cases without changing saved run evidence.

## Rates and time

`official-pricing-2026-09-17` is a versioned estimator, not an invoice. It selects
the native DeepSeek tariff using each trace's RFC 3339 timestamp, converted to UTC.
That trace timestamp can be a completion timestamp; it does not prove the exact
billing instant of a request that crosses a tariff boundary. A provider-reported
cost always takes precedence over an estimate.

| Interval/model | Uncached input / M | Cached input / M | Output / M |
| --- | ---: | ---: | ---: |
| June snapshot: V4 Flash, before August 16 16:00 UTC | $0.14 | $0.0028 | $0.28 |
| June snapshot: V4 Pro, before August 16 16:00 UTC | $0.435 | $0.003625 | $0.87 |
| Flash from September 10 04:00 UTC, peak | $0.30 | $0.006 | $1.20 |
| Flash from September 10 04:00 UTC, off-peak | $0.15 | $0.003 | $0.60 |

Dates are in 2026. Peak means Monday–Friday, 01:00–04:00 and 06:00–10:00 UTC;
the right boundary is excluded. Weekends and all other hours are off-peak.
Current Flash rates cover `deepseek-flash`, `deepseek-v4-flash`, and
`deepseek-v4-flash-vision-exp`. Historical June rates only cover the original V4
Flash and Pro identifiers; new identifiers are not retroactively priced.

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
count plus the input total is sufficient to derive the other bucket. Missing or
contradictory counters stay unknown; they do not increment prompt-cache misses.
`unknown_cache_input_tokens` exposes the unknown volume. A cache-discounted
estimate requires a trustworthy split and complete token usage.

Local response replay makes no new provider call, so its new cost is zero even
when the saved usage contains an old provider-reported cost. For compatibility,
token totals and cache counters still describe all trace invocations, including
saved usage on local replays; they must not be read as newly billed token totals.

`unpriced_calls` is exposed at run, model, and role levels. If any call in a group
is unpriced, that group's `cost_micro_usd` is null, rather than a misleading sum
of only the priced calls. Reported costs retain precedence over missing usage or
tariffs. Estimates retain their tariff source and the catalog version.

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
including that unrelated lockfile change in this patch. Optional adapter/server
feature qualification and deployment are not claimed.

The parent task owns delivery/review of this work branch. No main-branch merge or
production deployment is part of this lane.
