# Memory strategy — the substrate, the overlays, the referee (2026-07-02)

The question: what's the best move in the memory space — distillation? multi-level thinking?
What should we build as the best open-source memory usable by AI and people together?

## The convergence (four independent systems, one architecture)

| System | Truth | Derived | Human surface |
|---|---|---|---|
| symbiotic-memory | Archive Markdown+YAML (lossless, provenance-digested) | facts / briefs / indexes — all rebuildable | one .md per memory, editable |
| smartoffice | Vault Markdown, pristine-current-state, git = history | ephemeral per-query index, audit chain | Obsidian/CLI, RBAC at recall |
| membench findings | raw turns carried **100% of gold** (fact lane 0%) | overlays must *earn* recall lift | evidence-by-source scoreboard |
| Claude's own memory | atomic .md files + index | MEMORY.md brief tier | same files |

Nobody planned this convergence. That makes it the strongest possible signal.

## Answers to the direct questions

**Is it distillation?** Not as identity. Measured: distillation-as-replacement LOSES (fact lane 0%
of gold; briefs walled out of recall for cause). Distillation-as-navigation WINS where it has
structure: slot_key current-state, temporal normalization at write, subject links. **Facts are the
map; raw is the territory; answers come from the territory, found via the map.**

**Multi-level thinking?** Yes, but as *view maintenance*, never as new truth: sleep-time passes
that link/supersede/contradict/normalize — all re-derivable. The killer property this buys:
**reprocessability** — when models improve, re-distill the whole archive and memory improves
retroactively. That is the compounding asset.

**The moat.** Three different moats, don't conflate them:
- **Raw is the USER'S moat** — their archive compounds in value forever.
- **The FORMAT is the PROJECT'S moat** — the open standard becomes the schelling point. Nobody
  commits their company's payroll memory to a closed blob; openness is the adoption precondition,
  not a concession. Lock-in and memory don't mix.
- **Overlays are the COMPETITION SURFACE, on purpose** — swappable, pluggable, commodity by design.
- **membench is the REFEREE** — the unique asset nobody else has: *memory with receipts*. A public
  scoreboard proving which overlay earns its tokens, per benchmark, per dollar.

## What to build (the stack, named)

- **L0 · The Vault format** — publish the spec: one memory = md+YAML (id, provenance refs+digests,
  subjects, slot_key, lifecycle Active/Superseded/Contradicted/Uncertain/Unavailable, sensitivity,
  semantic times); append-only raw archive + artifacts; git = time machine; pristine-current-state
  rule. The "POSIX of memory" play. It already exists in code (OKF) — the move is *publishing it*.
- **L1 · The engine** (symbiotic-memory, OSS core) — ingest→distill→consolidate→recall, lifecycle
  machinery, Recall Gateway with in-flight redaction (smartoffice's no-principal-refusal model).
- **L2 · Overlays as plugins** — distillers/graphs/briefs/task-brains each ship a **manifest**
  (the membench AdapterCapabilityManifest IS the plugin API — the bench and the product share one
  contract), write only provenance-stamped views, get a scorecard.
- **L3 · Verticals** — smartoffice is the first proof: an office brain as an overlay + RBAC,
  humans and agents on the same vault.
- **L4 · The scoreboard** (membench v2) — public leaderboard per overlay/config = the trust engine
  AND the distribution engine for the whole ecosystem.

## Human+AI together — 3 solved, 1 missing

Solved in the stack already: (1) same file, both readers (md+YAML); (2) trust at recall, not at
storage (gateway redaction, memory stays whole); (3) history via git + clean current state.
**Missing: human edits as first-class memory ops** — watch the vault; a human correction becomes
a supersede event with `author: human`, re-embeds, re-consolidates, shows in the audit lane.
Close that and "usable by AI and people together" is real, not aspirational.

## Where the leverage is right now (the easy way)

1. **Rank/denoise overlays beat new distillers** — measured: denoise is the whole path 89→95.
2. **Publish the L0 format spec** — cheap, huge (the standard play starts).
3. **Make evidence-by-source the overlay scoreboard** — membench already splits raw/fact lanes;
   generalize to per-overlay recall-lift/$ and it's the referee.
4. **Human-edit-as-supersede** — the one missing primitive.
