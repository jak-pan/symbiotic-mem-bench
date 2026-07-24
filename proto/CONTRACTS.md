# Bench-owned contract schemas (`proto/membench/`)

These are the schemas **symbiotic-mem-bench owns** as the ecosystem's external referee. Three
new files were authored here per the convergence audit's **Stage 2 gate** ("Lock the contract
proto-family"):

| File | Package | Seam | What it is |
|---|---|---|---|
| `membench/trace/v1/trace.proto` | `membench.trace.v1` | **S10** | `StepEnvelope` + `Link` DAG — one typed, append-only, linkable step record for every pipeline (ingest, recall, bench run, agent run), static and streaming. |
| `membench/manifest/v1/manifest.proto` | `membench.manifest.v1` | **S16** | `AdapterCapabilityManifest` + `BenchmarkManifest` + `resolve()` decision table + the `caps_hash` comparability rule — the plugin API. |
| `membench/scorecard/v1/scorecard.proto` | `membench.scorecard.v1` | **S17** | `Scorecard` + `EvolutionReceipt` + `ArtifactProvenance` — the referee's receipts, one type family. |

The pre-existing `membench/dashboard/v1/debugger.proto` is the **stringly view-mirror wire**
(the current pb-migration). The bake-in note flags it as superseded by the envelope (#2); these
three files are the source-of-truth contracts it will be regenerated from, not a competitor to it.

## The Stage-2 gate they serve

> **Stage 2 · Lock the contract proto-family.** Gate: *schema files exist and compile in each
> owner tree; caps_hash computation specified; docs/redesign has ONE canonical home.*

These three are the bench-owned members of that coordinated commit wave. The other members land
in their own owner trees (memory-op.v1 + capture.v1 in the kit; action-envelope.v1 in the
runtime) and are intentionally **not** authored here.

Validation performed: `protoc` (homebrew, on PATH) —
`protoc --proto_path=proto --proto_path=$(brew --prefix)/include --descriptor_set_out=/dev/null <file>`
run against each of the three files. All three compile clean (the extra include path resolves
`google/protobuf/timestamp.proto`).

## Ownership (post-merge map)

Per the convergence audit §2 contract registry, symbiotic-mem-bench is the rightful owner of:
`StepEnvelope + Link DAG` (S10), `AdapterCapabilityManifest + BenchmarkManifest + resolve()`
(S16), and `ArtifactProvenance / scorecard` (S17). The engine and foundation reference these
(foundation's `ProviderDescriptor` is documented as the *contained* engine-level shape, not a
second capability dialect). membench is declared THE referee in both trees.

## The rules baked into every file

- **Additive-only evolution.** New payload prongs / fields = new field numbers; old readers skip
  unknown prongs and render them neutrally. Never repurpose or renumber. Reservations for
  intended-future fields are **by NUMBER ONLY**, with the intended names in comments
  (`tenant_id`, `space`, `acl`, `caps_hash_algo`; prongs `action_envelope`, `human_edit`,
  `evolution`) — a `reserved "name"` would permanently *forbid* re-adding that field, defeating
  the reservation. `reserved "name"` is used only for a field that was truly retired (none yet).
- **Envelope header where applicable** — id · schema_version · actor · sensitivity · provenance,
  then typed payload, then links (bake-in irreversibles #1–#7).
- **Explicit Unknown passthrough** — `trace.v1` carries an `UnknownPayload` prong so a fifth trace
  vocabulary degrades to neutral rendering instead of data loss; manifests keep an `ext` map
  (`x-<id>.*`) that the host stores, never rejects.
- **Reserved namespaces** — `membench.*` is host-owned; `x-<id>.*` is adapter/benchmark-owned.
- **Timing is schema, not inferred** — `trace.v1` splits `queued_ms / budget_wait_ms /
  cooldown_wait_ms / exec_ms` (the measure-don't-reason lesson; the conflated `queued_ms` was the
  opacity behind the 16-hour rpm incident).
- **caps_hash, not adapter id** — the comparability partition key is the resolved capability set
  (fixes the cohort-key contradiction, §3 #10); it lives on `manifest.v1 Resolution` and keys the
  `scorecard.v1 ScorecardSubject`.
- **Provenance by source-hash, never by clock** — `ArtifactProvenance` validity is
  `sha256(current sources) == sources[*].sha256`; digest vocabulary unified with OKF
  `sources[].sha256` (one sha256 language ecosystem-wide). The digest convention, named
  identically across the whole Stage-2 family, is **`sha256, lowercase hex`**.
- **Goodhart hedge as a field** — `EvolutionReceipt.frozen_hard_set_hash` proves the same frozen
  census was scored on both sides of a change without exposing its items; the frozen hard set is
  never a tuning input.

## Build wiring (done)

`build.rs` compiles the three contract files into `membench::proto::{trace,manifest,scorecard}::v1`
via `prost-build`, with `protoc` supplied by `protoc-bin-vendored` so a clean clone needs no system
protobuf install. Generated code is NOT committed; the Rust build regenerates it on every build, so
the `.proto` files are the single source of truth. Round-trip tests live in `src/proto.rs`.

The pre-existing `membench/dashboard/v1/debugger.proto` was deleted in the OSS triage: it was the
stringly view-mirror for the abandoned pb-migration, nothing in the repo consumed it (the server
serves plain JSON), and the bake-in note marks it superseded by the envelope. It remains in git
history (`git log -- proto/membench/dashboard`) if the migration is ever revived.

## Judgment calls made while authoring

1. **One `manifest.v1` file carries both manifests.** The seam catalog names them `adapter.v1`
   and `benchmark.v1`; the file layout groups the plugin-API family into a single versioned file.
   Package + message names keep the S16 identities (`AdapterCapabilityManifest`,
   `BenchmarkManifest`) so a later split into two packages is a move, not a wire change.
2. **`google.protobuf.Timestamp` for time** (matching the redesign/05 sketch) rather than the
   `string`/`double`-ms style of `debugger.proto`. This is the modern, absolute-time-at-write
   choice (bake-in #6); it adds one well-known-types include path to the build, noted above.
   This is the **family-wide** convention — the runtime's `action-envelope.v1` uses `Timestamp`
   too. The one documented divergence is the kit's `memory-op.v1` / `capture.v1`, which use
   RFC 3339 strings because their `op_id` is content-addressed and rebuild must replay the
   timestamp byte-for-byte (a `Timestamp` would not round-trip byte-identically). That is
   load-bearing, not drift; see the kit `contracts/README.md`.
3. **Richer `Timing` than redesign/05.** The redesign sketch used `wait_ms/run_ms`; S10 and the
   as-built consumer (data-api §F/§H) both demand the four split wait fields, so those win and
   subsume the two-field sketch.
4. **Local enums for `Actor` / `Sensitivity` / `Status`** in each file that needs them, rather
   than a shared `common.proto`. The task scoped authoring to exactly these three files (no shared
   import between them), so each stays self-contained and imports only google well-known types.
   The enums are commented as mirrors of the canonical S12/S14 vocab and must stay value-compatible
   (additive-only). `trace.v1 ActorKind` is value- **and number-**identical to the kit's
   `memory-op.v1 Actor.Kind` and the runtime's `action-envelope.v1 Actor.Kind`
   (`UNSPECIFIED=0, HUMAN=1, SYSTEM=2, AGENT=3, DISTILLER=4, DREAMER=5, REFEREE=6, EVOLUTION=7`),
   so the three unify without a renumber; all three map onto the kit Actor serde
   (`symbiotic-memory/src/types.rs ~:36`) `"human"`/`"system"`/`"agent:<id>"`.
5. **`ArtifactProvenance` lives in `scorecard.v1`.** S17 owns the provenance stamp; the redesign
   sketched it near the trace/data model. Materialized trace views reference the same shape.
