# Bake in now — the irreversibles (2026-07-02)

Test for inclusion: **cheap now, crippling to retrofit.** These are the fields, envelopes, and
disciplines that must exist BEFORE the organism/verticals/merge are built, so nothing built later
requires rework. The convergence audit's seam catalog is the evidence-grounded companion.

## The ten irreversibles

1. **IDs everywhere, paths nowhere.** Stable, sortable, globally-unique ids (ULID/UUIDv7) on every
   noun — memory, turn, fact, run, step, action, approval, flag. Id-addressed APIs only.
   *Already paid for this lesson once: membench's path-addressed question-debug.*
2. **One event envelope for everything that happens.** Generalize StepEnvelope into THE ecosystem
   envelope: header (id · ts · actor · schema_version · provenance · sensitivity) + typed payload
   + links. Captures, memory ops, trace steps, action proposals, approvals, audit entries — all
   ONE stream shape. Unknown-kind renders neutrally (proven in the trace renderer). The tape, the
   audit chain, the referee, and Evolution all become readers of one stream.
3. **schema_version in every artifact** — frontmatter, envelopes, manifests — plus the additive-only
   evolution rule (reserved fields, never repurpose). Version negotiation is a feature; version
   archaeology is a disaster.
4. **Derivation metadata on every derived thing.** Not just source refs+digests (exists) — also
   which model, which prompt version, which config hash produced it. This is what makes
   reprocessability real ("models improved → re-distill the archive") and receipts possible.
   membench's source-hash staleness gate is the pattern; make it universal.
5. **Actor on every write**: `human | agent(id) | system`. Human-edit-as-supersede, audit, and
   trust-building are all impossible to backfill without it.
6. **Write-time temporal normalization.** Absolute timestamps + semantic time resolved AT CAPTURE
   (event-time vs ingest-time distinguished). The temporal-reasoning wall is unfixable at read
   time; it is trivial at write time.
7. **Sensitivity label mandatory-with-default at the envelope level.** The Recall Gateway can only
   enforce what exists. Default T0; capture connectors set higher.
8. **The engine seam as a trait NOW** — chat/embed/rerank/reason with capability flags (context
   window, tool-use, structured output, cost class) behind foundation's provider queue. Claude =
   reference impl; every seam swappable. Also: standardized queue telemetry (throttle-wait vs
   http-time — the measured lesson) on every provider.
9. **One manifest schema family** for everything pluggable: adapter, benchmark, overlay, opinion
   pack, connector. Capabilities declared, knobs declared, trace streams declared, requires_caps.
   Reserve the fields now even where empty.
10. **Action-as-data before action-as-UI.** The approval-card contract as a message type (what ·
    why · evidence refs · cost · reversibility · action-CLASS) + Gatekeeper capability classes
    named now (incl. the ask-always-forever floor list). The autonomy dial needs stable classes
    to graduate; UIs come and go, the proposal message is forever.

## Upgrades to existing code (small, now)

- **symbiotic-memory**: actor field on writes; archive-watcher → human edits become supersede
  events; schema_version in OKF frontmatter if missing; id-addressed versioned API surface.
- **symbiotic-foundation**: engine-seam capability flags; queue telemetry standardized.
- **capture unification**: runtime Intake and smartoffice IngestPayload are one seam wearing two
  names — unify the contract (the envelope, #2) before a third variant appears.
- **the pb story**: the stringly-typed view-mirror proto is superseded by the envelope (#2) —
  don't invest in it further.

## The law that binds them

Every one of these exists to serve the invariant: **everything goes back to memory.** The envelope
writes to the archive's satellites; derivations trace to raw; actions and approvals are captures;
the referee reads the same stream it always did.
