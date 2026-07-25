# Contributing

Thanks for your interest in membench — the neutral benchmark harness for memory systems.

## Ground rules

The harness exists to publish **verifiable** benchmark results. Every contribution is bound by
the same integrity rules the maintainers follow:

- No benchmark-targeted hacks: no gold-string matching, no per-question special-casing, no
  tuning that reads the answer key. Levers land behind gated flags, off by default.
- No Python scoring scripts and no manual score entry. Scores come from the Rust pipeline and
  its judged artifacts.
- Never commit `runs/`, `.debug-session/`, `target/`, secrets, provider queues, or raw local
  datasets. Real `.env*` files are ignored; only the `*.example` templates are tracked.
- Tracked records must not contain raw prompts or secrets. Missing artifacts are declared
  missing in the manifest, never synthesized.
- Diagnostic experiment runs are `TRIAL`-flagged, not benchmark claims. Only records passing
  the review gate in `docs/longmemeval-methodology.md` may be ranked.

`AGENTS.md` is the full operating rulebook; on conflict it wins.

## Development setup

- Rust: pinned by `rust-toolchain.toml` (rustup picks it up automatically).
- Node 22 for the dashboard (`dashboard/`).
- Use an external Cargo target dir so `target/` never lands in the repo:
  `CARGO_TARGET_DIR=/tmp/symbiotic-mem-bench-target`.

No API keys are needed for development: tests, the explorer, the leaderboard exporter, the
dashboard, and explicit `--smoke` runs are all local and network-free.

## Quality gates

Run these before opening a PR (CI runs the same set — `.github/workflows/ci.yml`):

```bash
CARGO_TARGET_DIR=/tmp/symbiotic-mem-bench-target cargo fmt -- --check
CARGO_TARGET_DIR=/tmp/symbiotic-mem-bench-target cargo clippy --all-targets --features server -- -D warnings
CARGO_TARGET_DIR=/tmp/symbiotic-mem-bench-target cargo test
CARGO_TARGET_DIR=/tmp/symbiotic-mem-bench-target cargo test --features server
cargo deny check advisories licenses sources     # cargo install cargo-deny
cd dashboard && npm ci && npm run build
```

If you change the leaderboard export contract intentionally, regenerate the canary
(`canary/README.md`) and review the diff. If you change `records/`, regenerate the bundled
dashboard snapshot with `scripts/export-leaderboard-snapshot.sh`.

## Pull requests

- Keep commits coherent: one logical change per commit.
- Neutral run names in any committed record (see "Naming Runs" in `AGENTS.md`).
- Explain benchmark-relevant changes with evidence (which runs, what artifacts) — score deltas
  under the run-to-run variance floor are noise, not results.

## License

By contributing you agree that your contributions are licensed under the Apache License 2.0
(`LICENSE`).
