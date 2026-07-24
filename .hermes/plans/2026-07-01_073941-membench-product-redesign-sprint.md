# MEMBENCH Product Redesign Sprint — Merged Persona / Workflow / Data Model Synthesis

> **Status:** Evidence-backed product redesign synthesis merged from local browser walkthrough, source-backed component/API audit, and three parallel sweeps: persona strategy, full app IA/screen audit, and persona-driven data/API architecture. This document is intentionally above the trace-schema level.
>
> **User correction incorporated:** Do not redesign only the tracing endpoint. MEMBENCH is an agnostic benchmark/debug tool for memory / hybrid / multi-prong / agentic systems. The redesign must start from personas and jobs-to-be-done, then derive screens, data products, materialized artifacts, APIs, and trace structures.
>
> **Related design doc:** `.hermes/plans/2026-07-01_062051-tracing-data-model-design-sprint.md` covers the trace/data-artifact model. This document is the product/IA layer that should drive that lower-level design.

## Goal

Redesign MEMBENCH as a coherent product, not just a terminal dashboard with accumulated tabs.

The target product should support four high-level jobs:

1. **Choose / evaluate a memory system** — leaderboard, comparisons, evidence-backed ranking, cost/quality tradeoffs.
2. **Understand and debug a run** — questions, gold answers, retrieval/evidence coverage, traces, costs, prompts, artifacts.
3. **Run and iterate experiments** — configure, launch, monitor, stop, retry, tune, compare, and promote runs.
4. **Maintain benchmark/data quality** — question/gold/evidence audit, judge audit, annotation review, cohort comparability.

The central shift:

```text
old mental model: screens/tabs backed by ad hoc endpoint bundles
new mental model: personas → workflows → data products → materialized artifacts/APIs → screens
```

## Merged product thesis from async sweeps

The strategy sweep sharpened the product thesis:

```text
MEMBENCH is the decision cockpit for memory systems:
Can I trust this memory system, understand why it wins or fails,
tune it safely, and publish/reproduce the result?
```

The product is therefore not merely:

```text
leaderboard + debugger + traces
```

It is a benchmark operating system with four missions:

```text
Scout / Discover
  Compare memory systems, verify comparability, shortlist or choose.

Inspect / Analyze
  Explain scores, failures, evidence, costs, traces, prompts, and regressions.

Run Lab / Operate
  Configure, launch, monitor, stop, resume, retry, budget, and compare experiments.

Publish / Govern / Maintain
  Validate artifacts, dataset/gold quality, provenance, officialness, and reproducibility.
```

The existing `F1 Leaderboard` / `F2 Debugger` shell can remain as the expert terminal skin, but the redesigned product model should be mission-based underneath.

### Non-negotiable design implications

From the merged sweeps:

1. **Trust/comparability is a first-class product layer.**
   - Every cohort/run/record needs explicit comparable/mixed status.
   - Official / trial / oracle-gold / native / artifact-only / imported / missing-artifact states must be product-level trust states, not tiny labels.

2. **Question debugging must become an id-addressed product workflow.**
   - Current UI is question-centric, but debug is path-addressed via `question-debug?path=...`.
   - Target APIs and artifacts should pivot on `run_id + question_id`.

3. **Trace is not the umbrella product.**
   - Raw event streams, spans, waterfalls, heatmaps, cost rollups, question response traces, gold coverage, and comparison reports are distinct data products.

4. **Tuner + Live need to become a Run Lab/control plane.**
   - Current Tuner is command-preview only.
   - Current Live is strong monitoring but weak control.
   - The FAFO workflow needs launch/stop/pause/resume/retry/budget/concurrency/lineage.

5. **Publishability/reproducibility is a product workflow, not just a manifest.**
   - A trusted leaderboard needs artifact completeness, hashes, native/provenance state, redaction/publication checks, and submission/promotion status.

6. **Screens should compose product data objects; screens should not define backend schemas.**
   - Target objects include `System`, `SystemConfig`, `Benchmark`, `QuestionSet`, `Run`, `RemoteJob`, `QuestionRunRecord`, `GoldEvidencePiece`, `TraceStream`, `MaterializedArtifact`, and `Experiment`.

## Current local evidence

### Evidence collection performed

This document is grounded in:

- Browser walkthrough of the running app at `http://127.0.0.1:8180`.
- Screens inspected visually: Leaderboard, Debugger Overview, Questions, Question Drilldown, Compare, Traces, Gold Coverage, Live, Tuner.
- Source/component audit in `dashboard/src/**`.
- API evidence from browser resource timings and `dashboard/src/lib/api.ts` references seen through component imports.
- Prior trace/data audit in `.hermes/plans/2026-07-01_062051-tracing-data-model-design-sprint.md`.

Representative screenshot captures produced during walkthrough:

```text
Leaderboard screenshot supplied by user:
/private/var/folders/5_/95m_t6ts2s94sgs916mc4dg80000gn/T/TemporaryItems/NSIRD_screencaptureui_jGJkvZ/Screenshot 2026-07-01 at 06.33.36.png

Browser captures:
/Users/k/.hermes/cache/screenshots/browser_screenshot_857d4f469a1a4f11a26d074d6aa2387f.png  overview
/Users/k/.hermes/cache/screenshots/browser_screenshot_48829ddbb3ee46b2aaf34adc496c6fd3.png  questions
/Users/k/.hermes/cache/screenshots/browser_screenshot_eddf54a8e3964e259b1c26848dd3c5fd.png  question detail
/Users/k/.hermes/cache/screenshots/browser_screenshot_08031b2c204f439197c16e2dcd052b8a.png  traces
/Users/k/.hermes/cache/screenshots/browser_screenshot_80455021253f48c8a672ccdb8511f4a6.png  compare
/Users/k/.hermes/cache/screenshots/browser_screenshot_03e1fd837fc44d3b8fb15848c4a38490.png  gold coverage
/Users/k/.hermes/cache/screenshots/browser_screenshot_dc371e82f17f4e64a16f559414f74831.png  live
/Users/k/.hermes/cache/screenshots/browser_screenshot_858e0315aaf54e41920e8d60c1e12eb5.png  tuner
```

### Component / screen structure evidence

Source files found in `dashboard/src`:

```text
routes/Leaderboard.svelte
routes/Debugger.svelte
sections/Overview.svelte
sections/Questions.svelte
sections/Compare.svelte
sections/Traces.svelte
sections/GoldCoverage.svelte
sections/Live.svelte
sections/Tuner.svelte
components/TraceWaterfall.svelte
components/TraceLog.svelte
components/QueueSummary.svelte
components/CategoryHeat.svelte
components/Radar.svelte
components/RingGauge.svelte
components/DeltaBars.svelte
components/Panel.svelte
```

Debugger tab list is source-backed:

```text
dashboard/src/routes/Debugger.svelte:22 overview
dashboard/src/routes/Debugger.svelte:23 questions
dashboard/src/routes/Debugger.svelte:24 compare
dashboard/src/routes/Debugger.svelte:25 traces
```

More tabs are visible in browser and component search:

```text
OVERVIEW
QUESTIONS
COMPARE
TRACES
GOLD COVERAGE
LIVE
TUNER
```

Key screen API consumers from source search:

```text
Leaderboard.svelte     → api.leaderboard()
Overview.svelte        → api.run(id)
Questions.svelte       → api.questions(id), api.questionDebug(id, path)
Compare.svelte         → api.compare(base, cand)
Traces.svelte          → api.traces(id)
GoldCoverage.svelte    → api.goldEval(id) plus comparison runs
Live.svelte            → api.live(id), polls every ~2s
Tuner.svelte           → api.runnerSchema(), api.run(selected.run_id), api.runnerPlan(values)
```

### Current screen observations

#### Leaderboard

Visible product surface:

- Global mode: `F1 LEADERBOARD`.
- Cohort selector with many `long-mem-eval · NQ` cohorts.
- Warning banner: cohort not strictly comparable.
- Leaderboard hero with peak accuracy and cohort metadata.
- Field ranking matrix by question type.
- Multi-system comparison panel with radar chart and table.
- Ranked system table with accuracy, task average, abstention, category blocks, cost, updated time.

Current data products mixed:

```text
cohort registry
system leaderboard rows
comparability metadata
per-question-type score summaries
cost summaries
selected-system comparison projection
ranked table projection
```

Product strength: excellent for expert comparison.

Product gap: insufficient persona separation between public/executive leaderboard, research comparison, and system-selection decision support.

#### Debugger Overview

Visible panels:

```text
Score
Cohort & Models
Run Parameters
Artifacts
Model Calls
```

Current data concepts mixed at equal priority:

```text
quality metrics
cost/usage
model stack
dataset/cohort provenance
raw run parameters
artifact availability
system/build/live state
```

Product gap: Overview tells what happened, but not what to do next. Missing direct actions like:

```text
view failures
view abstentions
compare to baseline
open high-cost calls
open missing-artifact warnings
clone/rerun this config
```

#### Questions

Visible workflow:

- Filter by status: all/correct/wrong/abstain/error.
- Filter by question type.
- Search by question/answer/id.
- Table columns: status, id, type, question, answer/gold, hypothesis.
- Counts: correct/wrong/matches/rendered.
- Row click opens a debug modal.

Question drilldown evidence:

- Opens a modal for `001be529`.
- Shows correctness badge, question type, judge model/decision, debug bundle path.
- Shows gold answer vs hypothesis.
- Shows exact judge prompt.
- Browser resource confirms path-addressed API:

```text
/api/run/question-debug.pb?id=...&path=vaults/001be529/debug/hypotheses/hypotheses/question-debug.json
```

Product gap: question is the natural product unit, but debug access is path-addressed. It should be question-id addressed with artifact refs behind the scenes.

#### Compare

Visible workflow:

- Baseline dropdown: e.g. `collapse-nemo (85.4%)`.
- Candidate pill: `collapse-pplx`.
- Headline delta: `+1.4`.
- Verdict transition counts:

```text
fixed
regressed
still wrong
still right
abstention delta
common question count
```

- Per-category delta bars.
- Changed verdicts table with question id, question, baseline answer, candidate answer.

API evidence:

```text
/api/compare.pb?base=...collapse-nemo&cand=...collapse-pplx
```

Product strength: very strong release/regression decision surface.

Product gap: lacks explicit decision layer: ship/no-ship, risk severity, blocker regressions, row review state, annotation/ownership.

#### Traces

Visible panels:

```text
Dependency Waterfall
Memory Work Timing
Unified Trace Log
Bottleneck Overview
Trace Waterfall
Provider Queue Summary
Workflow Queue
```

Source evidence:

```text
Traces.svelte:177 Dependency Waterfall
Traces.svelte:197 Memory Work Timing
Traces.svelte:220 Unified Trace Log
Traces.svelte:226 Bottleneck Overview
Traces.svelte:256 Trace Waterfall
Traces.svelte:274 Provider Queue Summary
```

Product observation:

- Most visible panels are derived analytics/views, not raw traces.
- Current tab name `Traces` hides multiple product jobs:
  - performance diagnosis
  - raw event inspection
  - provider queue analysis
  - workflow/runner state inspection
  - waterfall visualization

Product gap: should split into Observability / Performance / Raw Events / Provider Calls / Runner Pipeline, with persona-specific summaries above expert views.

#### Gold Coverage

Visible workflow:

- Classification summary:

```text
434 correct
0 reader fail
66 retrieval gap
```

- Gold piece coverage: `0.0%` visible in capture.
- Coverage by source:

```text
both: 0
fact: 0
raw: 941
none: 7
```

- Question shape: single vs multi.
- Embedding vs rerank rank table.
- Run comparison for retrieval/gold coverage.
- Per-question table with ranks/classification.

API evidence:

```text
/api/run/gold-eval.pb?id=...
/api/run/gold-eval.pb?id=...c500-coh-1
/api/run/gold-eval.pb?id=...nemo-rpmfix-500
/api/run/gold-eval.pb?id=...pplx-rpmfix-500
```

Product strength: this is not just a trace UI; it is an evidence/retrieval benchmark diagnostic.

Product gap: metric definitions are hard to infer. `0.0% top-k` beside low mean ranks may be correct but confusing without explicit definition such as “all required gold pieces within top-k.”

#### Live

Visible workflow:

- Selected run `collapse-pplx`, status `DONE`.
- Progress panel with queue events, ingested/answered counts, calls, failures, tokens, trace size, RPM.
- Per-provider/model rows.
- Pipeline drilldown with stages:

```text
setup
recall setup
prompt plan
answer embed
fact search
raw search
support
answer ctx
answer
rerank
```

- Error log with provider unavailable errors.
- Recent activity stream.
- Auto-refreshing every 2s.

API evidence:

```text
/api/run/live.pb?id=...collapse-pplx
```

Product gap: strong monitoring, weak control. The user’s FAFO persona needs visible run lifecycle actions:

```text
start
stop
pause
resume
cancel/kill
drain queue
retry failed
clone/rerun
change concurrency/provider/budget
recover stale runs
```

#### Tuner

Visible workflow:

- Run configuration seeded from selected run.
- Fields grouped into Inputs, Lifecycle, Memory, Scoring.
- Command preview generates exact `cargo run --release ... membench ...` command.
- Buttons:

```text
COPY
COPY FULL SCRIPT
SPAWN RUN (preview mode) [disabled]
```

- Explicit text:

```text
Preview mode — the dashboard builds the exact command; run it in your terminal. Live spawn + log streaming is the next milestone.
```

API evidence:

```text
/api/runner/schema.pb
/api/run.pb?id=...
/api/runner/plan.pb
```

Product gap: currently a command builder, not a remote experiment workbench. Missing cost estimate, budget cap, provider credentials, remote worker selection, live logs, control plane, job state, and run lineage.

## Personas and jobs-to-be-done

### Persona A — Memory system evaluator / buyer / model selector

This is the person “looking for a memory system.” They want to know:

- Which system is best for my benchmark/cohort?
- Is the ranking comparable and trustworthy?
- What is the quality/cost/latency tradeoff?
- Which system is robust across categories, not just high average?
- Can I compare top systems side-by-side?
- Can I export/share a report?

Primary screens today:

```text
Leaderboard
Compare
Overview summary
```

Needs:

```text
cohort-aware leaderboard
system profile pages
comparison report
cost/quality frontier
category/risk breakdown
plain-language metric definitions
share/export
```

Data products:

```text
CohortSummary
LeaderboardRow
SystemProfile
SystemConfigSummary
ComparisonReport
ScoreBreakdown
CostSummary
ComparabilityWarning
```

### Persona B — Memory system builder / tuning engineer

This person wants to improve a system. They ask:

- Why did this run fail on these questions?
- Were failures due to retrieval, memory distillation, ranking, prompt, reader, judge, or data issue?
- Which knobs should I tune next?
- What changed from baseline?
- Which prompts/model components caused failures or costs?

Primary screens today:

```text
Overview
Questions
Question drilldown
Compare
Gold Coverage
Traces
Tuner
```

Needs:

```text
question-centric debug workspace
failure taxonomy
retrieval/evidence drilldown
provider/prompt drilldown
trace/performance drilldown
parameter diff vs baseline
recommended next trials
```

Data products:

```text
QuestionRunRecord
QuestionDebugBundle
FailureClassification
GoldEvidenceRecord
RetrievalAttempt
ProviderCallSummary
PromptArtifact
TraceSpan/Event
ParameterDiff
TuningRecommendation
```

### Persona C — Operator / FAFO experiment runner

This is the “run, stop runs, tweak prompts/models, tune from traces, pay for remote high-score attempt” user.

They ask:

- Can I clone this run, tweak it, and launch a new one?
- What will it cost?
- Which credentials/providers/workers will be used?
- Is it running, stuck, stale, or burning money?
- Can I pause/kill/drain/retry/switch provider?
- When done, can I compare/promote the result?

Primary screens today:

```text
Tuner
Live
left registry / in-flight rail
command palette
```

Needs:

```text
run workbench
preflight checklist
cost estimate + budget cap
remote job control plane
live logs + queue state
stale run recovery
run lineage
post-run actions
```

Data products:

```text
ExperimentSpec
RunPlan
CommandPreview
PreflightReport
CostEstimate
BudgetPolicy
RemoteJob
JobEvent
LiveRunState
QueueState
WorkerState
ControlAction
RunLineage
```

### Persona D — Benchmark/data maintainer

This person owns benchmark validity. They ask:

- Are question/gold annotations correct?
- Are gold pieces mapped to raw turns/facts/sessions?
- Are failures due to data ambiguity or judge issues?
- Are cohorts comparable?
- Are metrics defined correctly?

Primary screens today:

```text
Questions
Question drilldown
Gold Coverage
Compare
artifact browser
```

Needs:

```text
gold annotation audit
question set browser
raw/fact/gold mapping
judge prompt/rationale audit
review queues
accepted alias/equivalence handling
cohort fingerprint/comparability report
```

Data products:

```text
QuestionDefinition
GoldAnswer
GoldEvidencePiece
GoldEvidenceMapping
JudgeInput/Output
AnnotationReview
CohortFingerprint
DatasetVersion
MetricDefinition
```

### Persona E — Release owner / product stakeholder

This person asks:

- Is this candidate better enough to promote?
- What are the regressions?
- Are risks acceptable?
- What does the change mean in plain language?
- Can I share a concise report?

Primary screens today:

```text
Leaderboard comparison
Compare
Overview
Gold Coverage summaries
```

Needs:

```text
ship/no-ship summary
risk callouts
fix/regression severity
acceptance gates
shareable comparison report
trend over previous releases
```

Data products:

```text
ReleaseGateReport
ComparisonDecisionSummary
RegressionSet
FixSet
RiskSummary
TrendSummary
ExportableReport
```

### Persona F — Cost/provider manager

Not explicit in the user’s list, but warranted by the product evidence.

They ask:

- Which provider/model consumes most tokens/cost?
- Are retries or failures wasting money?
- What will the next run cost?
- Which model stack gives best cost-adjusted quality?
- Are rate limits/cache settings working?

Primary screens today:

```text
Overview Model Calls
Live provider rows
Tuner config
Leaderboard cost column
```

Needs:

```text
cost ledger
provider health
cache hit/miss
rate limit pressure
cost forecast
cost/accuracy frontier
budget gates
```

Data products:

```text
CostLedger
ProviderUsageRollup
ProviderHealth
CacheSummary
RateLimitSummary
BudgetEstimate
CostQualityFrontier
```

## Proposed product information architecture

Current top-level modes:

```text
F1 Leaderboard
F2 Debugger
```

This is visually strong but conceptually too coarse. The product should still preserve the terminal aesthetic, but the underlying IA should be organized around workflows.

### Proposed top-level workspaces

```text
1. Discover
   Leaderboards, system profiles, cohort comparisons, reports.

2. Analyze
   Run overview, questions, compare, evidence/gold, performance, costs, artifacts.

3. Observe
   Live runs, queues, workers, provider health, logs, alerts.

4. Experiment
   Tuner, run planning, remote launch, preflight, budget, run lineage.

5. Maintain
   Dataset/gold/judge audit, annotation review, cohort validity, metric definitions.
```

Mapping current screens into this IA:

| Current screen | Proposed home | Notes |
|---|---|---|
| Leaderboard | Discover | Keep as primary public/system-selection surface. |
| Leaderboard comparison panel | Discover + Analyze | Turn selected systems into shareable comparison report. |
| Overview | Analyze / Run Summary | Split decision summary from raw config/provenance. |
| Questions | Analyze / Question Workbench | Make question id first-class. |
| Question modal | Analyze / Question Detail | Add tabs: summary, evidence, judge, provider, traces, raw. |
| Compare | Analyze / Comparison | Add release-decision and review workflows. |
| Traces | Analyze / Observability | Split derived performance views from raw events. |
| Gold Coverage | Analyze + Maintain | Rename/position as Evidence Coverage; add maintainer/audit mode. |
| Live | Observe | Add run controls and incident/remediation actions. |
| Tuner | Experiment | Evolve from command builder to run workbench. |
| Registry sidebar | Global object browser | Needs filtering/search/lineage, not endless lists. |

### Proposed product object model

The UI should revolve around these first-class objects:

```text
System
  The evaluated memory/hybrid/agentic system identity.

SystemConfig
  Model stack, memory backend, prompts, retriever/reranker, runtime knobs.

Benchmark
  Benchmark family, e.g. long-mem-eval.

QuestionSet / Cohort
  Comparable set of questions with fingerprint, judge config, metric definitions.

Run
  A completed or in-progress execution of a SystemConfig over a QuestionSet.

RunAttempt / RemoteJob
  Operational execution attempt that may be live/stale/failed/done.

Question
  Stable benchmark unit.

QuestionRunRecord
  Per-run result for one question: answer, judgment, evidence, trace refs, costs.

GoldEvidencePiece
  Annotated evidence expected to support an answer.

TraceStream
  Raw append-only observability stream: memory/provider/runner/judge/tool/etc.

MaterializedArtifact
  Derived/cached product artifact: summaries, heatmaps, waterfalls, comparisons.

Experiment
  A planned or launched set of parameter changes and associated runs.
```

## Data product taxonomy by workflow

### Discover / choose a system

Data products:

```text
LeaderboardCohort
LeaderboardRow
SystemProfile
SystemComparison
QualityCostFrontier
ComparabilityReport
```

Materialization:

- Post-run/cohort-level.
- Recompute when new runs are added or scoring/judge definitions change.

API direction:

```text
GET /api/discover/cohorts
GET /api/discover/leaderboard?cohort_id=...
GET /api/systems/{system_id}
GET /api/systems/{system_id}/runs
POST /api/comparisons/systems
```

### Analyze / debug a run

Data products:

```text
RunSummary
RunHealthSummary
QuestionRunIndex
QuestionRunRecord
QuestionDebugWorkspace
ComparisonReport
EvidenceCoverageReport
PerformanceSummary
ProviderUsageReport
ArtifactManifest
```

Materialization:

- Run summary and question index should be materialized after run.
- Question detail can lazy-load raw artifacts/traces by refs.
- Comparison reports can be materialized/cacheable by `(base_run_id, candidate_run_id, metric_version)`.

API direction:

```text
GET /api/runs/{run_id}
GET /api/runs/{run_id}/summary
GET /api/runs/{run_id}/questions?filters...
GET /api/runs/{run_id}/questions/{question_id}
GET /api/runs/{run_id}/questions/{question_id}/debug
GET /api/runs/{run_id}/questions/{question_id}/evidence
GET /api/runs/{run_id}/questions/{question_id}/provider-calls
GET /api/runs/{run_id}/questions/{question_id}/events
POST /api/comparisons/runs
```

### Observe / operate live runs

Data products:

```text
LiveRunState
QueueState
PipelineProgress
ProviderHealth
ErrorLog
ActivityLog
WorkerState
BudgetBurnState
ControlActionAudit
```

Materialization:

- During run: cheap incremental counters and state snapshots.
- Stream raw/job events through cursor/SSE/WebSocket.
- Final live snapshot becomes part of run report after completion.

API direction:

```text
GET /api/jobs
POST /api/jobs
GET /api/jobs/{job_id}
GET /api/jobs/{job_id}/stream
POST /api/jobs/{job_id}/actions/pause
POST /api/jobs/{job_id}/actions/resume
POST /api/jobs/{job_id}/actions/cancel
POST /api/jobs/{job_id}/actions/kill
POST /api/jobs/{job_id}/actions/retry-failed
POST /api/jobs/{job_id}/actions/set-concurrency
```

### Experiment / FAFO / tuning

Data products:

```text
ExperimentSpec
RunPlan
ParameterSchema
ParameterDiff
PreflightReport
CostEstimate
BudgetPolicy
CommandPreview
RemoteLaunchRequest
RunLineage
TuningRecord
Recommendation
```

Materialization:

- Preflight computed before launch.
- Cost estimate computed from question count, model stack, historical usage, provider pricing.
- Run lineage written at launch and finalized after completion.

API direction:

```text
GET /api/experiments/schema
POST /api/experiments/plan
POST /api/experiments/preflight
POST /api/experiments/launch
GET /api/experiments/{experiment_id}
GET /api/experiments/{experiment_id}/lineage
POST /api/experiments/{experiment_id}/clone
```

### Maintain / benchmark + gold quality

Data products:

```text
DatasetProfile
QuestionSetProfile
QuestionDefinition
GoldAnswer
GoldEvidencePiece
GoldEvidenceMapping
JudgeAuditRecord
AnnotationReviewItem
MetricDefinition
CohortComparabilityReport
```

Materialization:

- Dataset/gold validation should run after dataset import and after scoring.
- Review annotations should be durable user-editable records, not derived only from run artifacts.

API direction:

```text
GET /api/datasets
GET /api/datasets/{dataset_id}
GET /api/question-sets/{qset_id}
GET /api/question-sets/{qset_id}/questions
GET /api/question-sets/{qset_id}/gold-coverage
POST /api/reviews/question
POST /api/reviews/judgment
POST /api/reviews/gold-evidence
```

## Screen redesign proposals

### Leaderboard → Discover workspace

Keep:

- Cohort picker.
- Ranked table.
- Category heat.
- Multi-system comparison.
- Terminal aesthetic.

Add:

```text
system profile drawer
cost/accuracy frontier
comparability filter/split by qset/judge/config
plain-language cohort warnings
shareable comparison link/report
rank by metric selector: accuracy, cost-adjusted, abstention-adjusted, category-specific
```

Primary actions:

```text
Compare selected
Open system profile
Open best run
View evidence behind rank
Export leaderboard
Promote as baseline
```

### Overview → Run Summary / Decision page

Split into two layers:

1. **Decision summary**
   - Score and delta vs baseline.
   - Correct/wrong/abstain.
   - Top failure slices.
   - Main cost driver.
   - Main latency/performance issue.
   - “What to inspect next” actions.

2. **Provenance/details**
   - Model stack.
   - Cohort/config fingerprints.
   - Parameter groups.
   - Artifact manifest with open/download/validate/compare actions.

Suggested top actions:

```text
View failures
View abstentions
Compare to baseline
Open evidence gaps
Open provider cost
Clone into Tuner
Export report
```

### Questions → Question Workbench

Make `question_id` the main drilldown key.

Main table should add:

```text
failure class
severity
retrieval status
judge status
baseline/candidate status if comparing
cost/latency per question
review state
```

Question detail should become a tabbed workspace:

```text
Summary
Gold vs Hypothesis
Evidence / Retrieval
Prompt / Provider Calls
Judge
Trace / Timeline
Artifacts / Raw JSON
Review / Annotation
```

Actions:

```text
Open in Compare
Open traces for question
Mark judge issue
Flag bad gold
Add accepted alias
Create tuning item
Export failure case
```

### Compare → Release / Regression Workbench

Keep:

- Baseline selector.
- Candidate score delta.
- Fixed/regressed/still wrong/still right.
- Category deltas.
- Changed verdicts.

Add:

```text
ship/no-ship summary
release gates
regression severity
transition matrix
filter changed rows by fixed/regressed/abstention/category/severity
side-by-side question detail
review/approve regressions
export comparison report
```

### Traces → Observability / Performance Workbench

Rename or split. `Traces` currently mixes raw events and derived analytics.

Suggested subviews:

```text
Summary         high-level bottlenecks, warnings, deltas
Timeline        trace/waterfall derived views
Operations      memory stage timing, provider queue timing
Provider Calls  cost/tokens/latency/errors/cache
Runner          workflow queue, job/worker lifecycle
Raw Events      paged/searchable memory/provider/runner/judge/tool events
Errors          provider/system/memory/judge errors with remediation links
```

Critical data principle:

```text
Raw trace streams are fetched separately by type/query/cursor.
Derived trace views are materialized analytics with source refs.
```

### Gold Coverage → Evidence Coverage / Benchmark Audit

Rename for product clarity:

```text
Evidence Coverage
```

Modes:

```text
Retrieval Debug
Distillation Debug
Benchmark Audit
Run Comparison
```

Add metric definitions inline:

```text
gold piece
gold turn
gold session
deepest gold rank
in candidate set
top-k all pieces vs any piece
raw-only vs fact-only
```

Actions:

```text
Open 66 retrieval gaps
Open raw-only gold pieces
Open none-covered gold pieces
Show rank movement examples
Show raw → fact mapping
Create dataset review item
```

### Live → Run Operations Center

Keep telemetry, but add controls.

For active jobs:

```text
Pause
Resume
Cancel
Kill
Drain queue
Retry failed
Change concurrency
Switch provider/fallback
Set budget cap
Open live logs
```

For stale jobs:

```text
Show heartbeat
Reconnect
Mark dead
Clear stale
Recover checkpoint
Confirm not burning money
```

For completed jobs:

```text
Open results
Compare
Clone/rerun
Export logs
Archive/delete
Promote to leaderboard/baseline
```

### Tuner → Experiment Lab

Keep command preview, but demote it to “CLI equivalent.”

Add primary workflow:

```text
Select baseline
Clone config
Edit meaningful knobs
View parameter diff
Preflight
Estimate cost/runtime
Set budget
Launch local/remote
Monitor job
Compare result
Promote or iterate
```

Required new panels:

```text
Preflight checklist
Cost estimate
Provider credentials/rate limits
Remote target / worker selection
Budget cap
Run lineage
Experiment variants
Result summary after completion
```

## Data model implications

### Do not let tabs define data models

Current APIs reflect screens:

```text
/api/run/traces
/api/run/live
/api/run/questions
/api/run/gold-eval
/api/compare
```

Target data products should be independent of screens:

```text
RunSummary
QuestionRunRecord
EvidenceCoverageReport
TraceEventStream
PerformanceAnalytics
ProviderUsageReport
ComparisonReport
ExperimentPlan
RemoteJobState
```

Screens compose these data products.

### Question-centric model is mandatory

The product repeatedly pivots on `question_id`:

```text
Questions table
Question debug modal
Compare changed verdicts
Gold coverage rows
Trace/event correlation
Provider calls
Failure triage
Dataset/gold review
```

Target model:

```text
QuestionRunRecord {
  run_id
  question_id
  question_set_id
  question_type
  question_text
  gold_answer
  hypothesis
  judgment
  failure_class
  review_state
  debug_artifact_refs
  evidence_refs
  trace_refs
  provider_call_refs
  cost_summary
  timing_summary
}
```

This record should be materialized after a run and used by Questions, Compare, Gold Coverage, and report/export flows.

### Trace model should serve product data, not vice versa

The trace model still needs common envelope + typed payloads, but product screens should not consume giant raw trace bundles.

Raw streams:

```text
memory events
provider events
runner/job events
judge/scoring events
tool/agent events
artifact lifecycle events
```

Derived products:

```text
performance summary
waterfall view
provider usage/cost
queue timing
pipeline progress
gold/evidence coverage
question debug timeline
```

### Materialized artifacts should be explicit and source-backed

Every derived product should carry:

```text
schema/version
generated_at
builder_version
source artifact refs
source hashes
run_id/question_set_id
invalidated_by
```

Examples:

```text
run-summary.v1.pb/json
question-run-index.v1.pb/jsonl
evidence-coverage.v1.pb/json
comparison-report.v1.pb/json
performance-summary.v1.pb/json
provider-usage.v1.pb/json
search-index.v1.sqlite/json
```

## Materialization lifecycle

### During run

Compute cheap, operationally necessary projections:

```text
job state
heartbeat
queue counts
per-stage progress
open spans
recent errors/activity
provider usage counters
tokens/cost so far
budget burn
artifact presence
per-question status: pending/running/done/failed/scored
```

Avoid heavy post-hoc analytics during run unless they are needed for control decisions.

### On question completion

Materialize incremental question-level records:

```text
QuestionRunRecord partial/final
provider calls linked to question
retrieval/evidence refs
judge input/output refs
trace span refs
cost/timing summary
```

This improves debugger latency and avoids re-reading giant raw artifacts for every question drawer.

### After run completion

Compute reproducible analytics:

```text
run summary
question index
comparison-ready verdict table
evidence coverage
gold coverage
provider usage/cost ledger
performance/waterfall artifacts
search indexes
artifact manifest with hashes/sizes/schema
cohort/comparability report
```

### On comparison request

Materialize/cache:

```text
ComparisonReport(base, candidate, metric_version)
changed verdicts
transition matrix
category deltas
risk/regression labels
```

### On dataset/gold review

Persist durable human review records:

```text
accepted aliases
bad gold flags
judge issue flags
annotation comments
severity labels
review decisions
```

These should not be overwritten by benchmark reruns.

## Roadmap

### Phase 0 — freeze product/design target

No runtime code changes except docs/fixtures.

Deliverables:

```text
this product redesign sprint doc
trace/data model ADR linked to product workflows
screen/data product matrix
object model diagrams or text spec
API inventory with old→new mapping
```

### Phase 1 — central data product inventory

Add read-only inventory endpoints/utilities:

```text
run data product inventory
artifact manifest with schemas/sizes/hashes
question debug refs by question_id
trace source refs by type
materialized artifact presence/status
```

Purpose: make current mess observable before changing it.

### Phase 2 — question-centric foundation

Build first-class `QuestionRunRecord` materialization and API.

Replace path-first debug fetch with question-first API while preserving old path endpoint as compatibility.

```text
/api/runs/{run}/questions
/api/runs/{run}/questions/{qid}
/api/runs/{run}/questions/{qid}/debug
```

### Phase 3 — product data product APIs

Create screen-independent APIs:

```text
RunSummary
ComparisonReport
EvidenceCoverageReport
PerformanceSummary
ProviderUsageReport
TraceEventStream
ExperimentPlan
LiveJobState
```

Current screen endpoints can wrap these initially.

### Phase 4 — redesigned Question Workbench + Evidence Coverage

Start where debugging value is highest:

```text
question detail tabs
evidence/retrieval panel
judge panel
provider calls panel
trace timeline panel
review/failure-classification fields
```

Also rename/reshape Gold Coverage into Evidence Coverage with metric definitions and drilldowns.

### Phase 5 — Experiment Lab / FAFO control plane

Turn Tuner + Live into a real run workbench:

```text
preflight
cost estimate
budget cap
remote/local launch
job status
live logs
pause/cancel/retry controls
stale recovery
run lineage
```

### Phase 6 — Discover / Leaderboard polish

Convert current strong leaderboard into a system-selection product:

```text
system profile
comparability controls
shareable reports
cost-quality frontier
plain-language summaries
promotion/baseline flows
```

### Phase 7 — Maintain / Benchmark audit

Add dataset/gold/judge review workflows:

```text
gold evidence editor/review queue
judge audit
accepted aliases
bad-question flags
cohort comparability management
metric definition docs in UI
```

## Merged async sweep addendum

### Additional personas confirmed by strategy sweep

The local brief already includes evaluator, builder/debugger, FAFO operator, benchmark maintainer, release owner, and cost/provider manager. The async strategy sweep adds or sharpens four more personas that should stay explicit in the product model:

#### Adapter / integration engineer

Adds a new memory system or imported result format.

Jobs:

```text
understand adapter contracts
emit required artifacts/traces/manifests
validate artifact completeness
label unsupported capabilities honestly
submit comparable record
```

Required data products:

```text
AdapterCapabilities
TraceStreamInventory
ArtifactManifest
ValidationReport
CapabilityGapReport
```

#### Researcher / methodologist

Studies mechanisms, ablations, oracle ceilings, and retrieval-vs-reader decomposition.

Jobs:

```text
compare controlled ablations
export charts/tables
inspect oracle-gold ceilings
study rank distributions and tail behavior
produce paper/blog-ready evidence
```

Required data products:

```text
AblationMatrix
OracleCeilingReport
RetrievalReaderDecomposition
RankDistributionArtifact
ExportableAnalysisBundle
```

#### Public leaderboard participant

Wants to prove a system is state-of-the-art.

Jobs:

```text
run official shape
submit/import artifacts
validate provenance and hashes
protect private/secrets while sharing enough evidence
track review/publishing state
```

Required data products:

```text
SubmissionBundle
PublicationCheck
RedactionPolicy
OfficialRecordStatus
ReviewDecision
```

#### Executive / technical lead

Needs compressed, trustworthy progress/status.

Jobs:

```text
understand quality/cost/reliability trend
see blockers and regressions
approve budget or roadmap
consume one-page report
```

Required data products:

```text
ExecutiveSummary
TrendReport
RiskSummary
BudgetSummary
RoadmapEvidenceReport
```

### Key user decision moments to design around

The strategy sweep identifies seven decision moments that should drive the product flows:

```text
Can I compare these runs?
Can I trust this score?
Why did this answer fail?
What should I tune next?
Is this run worth paying for / continuing?
Did my change help or hurt?
Should this become an official/public/curated record?
```

Every major screen should help answer at least one of these questions and provide the next action.

### Product success metrics

The redesign should be measurable. Suggested global metrics:

```text
time to first trusted comparison
time from wrong answer to classified root cause
time from config idea to launched run
time from run completion to baseline comparison
% runs with complete artifact manifests
% leaderboard cohorts strictly comparable
% completed runs with usable question debug
% failed runs with actionable error classification
cost per valid scored run
public/curated record promotion rate
third-party adapter/submission count
```

Suggested workflow metrics:

```text
Leaderboard:
  cohort selection rate
  compare selection rate
  run-detail open rate
  trust-warning comprehension rate

Debugger:
  wrong-question filter usage
  drawer open rate
  gold/evidence coverage usage
  failure classification completion
  compare usage after question inspection

Run Lab / Operator:
  tuner-to-run conversion
  spawn success rate
  stop/resume usage
  provider failure rate
  budget overrun rate
  run abandonment rate

Publishing:
  validation pass rate
  artifact completeness rate
  submission rejection reasons
  time to public/curated record
```

### Full screen audit: repeated pain points

The IA/screen audit independently confirmed recurring product problems across screens:

```text
high density / low discoverability
critical identifiers truncated
many cards/rows imply drilldown but lack visible action affordances
insufficient legends/tooltips for abbreviations and badges
weak local path from aggregate summary → exact cause/evidence/artifact
copy/export/permalink controls missing in many places
empty/missing states often informative but not actionable enough
```

High-impact cross-screen fixes:

```text
sidebar search + saved filters
copy/permalink controls for run/question/trace/artifact
clickable summary cards with filtered drilldowns
row/detail drawers for Compare, Gold Coverage, Traces, Live stages
sticky local section nav for long tabs
legends/tooltips for G/NATIVE/REC/META/SS·*/ABST/CAD/A/etc.
actionable artifact management: open/generate/copy/validate/refresh
export/report affordances
validation/environment readiness in Tuner/Run Lab
```

### Full screen audit: notable screen-specific deltas

#### Leaderboard

Needs:

```text
filter to comparable subgroup
explicit comparison tray or checkboxes
search/filter by run/model/config
metric glossary
sort indicators
cost-quality frontier
share/export report
```

#### Overview

Needs:

```text
run health diagnosis
clickable artifact rows
parameter grouping
copy fingerprints/config/run path
cost/cache drilldown
next-step actions from score/failure/cost panels
```

#### Questions

Needs:

```text
explicit details chevron
legend for verdict/type/abstention
next wrong / previous wrong
section nav inside drawer
links to traces/gold/compare for same question
copy/export question case
failure classification and review state
```

#### Compare

Needs:

```text
baseline/candidate swap
clickable fixed/regressed/still-wrong buckets
changed-question detail drawer
transition matrix
release-gate summary
export/permalink
```

#### Traces / Observability

Needs:

```text
split raw events from derived views
span inspector drawer
filters by source/question/operation/status
sticky headers/local nav
explain dashes: missing vs not applicable vs zero
trace export/permalink
```

#### Evidence / Gold Coverage

Needs:

```text
artifact generate/copy/refresh/open actions
metric definitions for gold piece/turn/session/deepest rank/top-k
coverage funnel: gold exists → candidate set → reranked top-k → context → correct
clickable cards and per-question drawer
selectable comparison runs instead of hardcoded refs
```

#### Live / Run Operations

Needs:

```text
state clarity: active live vs completed snapshot vs stale
queue event browser
stage detail drawer
activity row expansion
cost/throughput panel
manual refresh/pause polling
run controls when backend supports them
```

#### Tuner / Experiment Lab

Needs:

```text
avoid disabled false affordance for spawn preview
field help/tooltips
validation checklist: paths/env/provider/writable/credentials
save preset / duplicate / reset / diff from seed
manual-run attach instructions
remote/local execution when backend supports it
```

### Product data architecture: merged non-negotiables

The data/API sweep independently confirms the core architecture:

```text
Do not make trace the umbrella product.
Make QuestionRunRecord the central product object.
Replace path-addressed question debug with question-id addressed APIs.
Treat gold/source/evidence as id-addressable dataset objects.
Split raw event streams from materialized views.
Rename/clarify provider/model traces as provider lifecycle events.
Promote workflow SQLite projections into typed runner/job events.
Give every constructed artifact a header with schema/source hashes/builder/redaction.
Materialize heatmaps, waterfalls, question response traces, and distributions as artifacts/views.
Keep /api/run/traces only as a compatibility façade during migration.
Centralize source resolution and validation.
```

### Recommended constructed artifact families from data/API sweep

Run-level artifacts:

```text
run_summary.v1
artifact_manifest.v2
score_summary.v1
cost_rollup.v1
trace_summary.v1
stage_timing.v1
provider_queue_timing.v1
trace_waterfall.v1
dependency_waterfall.v1
gold_coverage.v1
rank_distribution_heatmap.v1
search_index.v1
```

Question-level artifacts:

```text
question_run_record.v1
question_response_trace.v1
answer_context.v1
retrieval_candidates.v1
rerank_trace_view.v1
judge_record.v1
question_event_index.v1
```

Compare/publishing artifacts:

```text
compare_summary.v1
compare_question_deltas.v1
cohort_matrix.v1
leaderboard_snapshot.v1
publication_check.v1
submission_bundle.v1
```

### API partition refined by data/API sweep

The target API families should be screen-independent:

```text
Registry / leaderboard / compare:
  GET /api/runs.pb
  GET /api/run.pb?id=...
  GET /api/cohorts.pb
  GET /api/leaderboard.pb?benchmark=&limit=&cohort=
  GET /api/compare.pb?base=...&cand=...
  GET /api/compare/questions.pb?base=...&cand=...&transition=&type=&offset=&limit=

Question-centric:
  GET /api/run/questions.pb?id=...&q=&label=&type=&offset=&limit=
  GET /api/run/question.pb?id=...&question_id=...
  GET /api/run/question/debug.pb?id=...&question_id=...&mode=redacted|raw
  GET /api/run/question/response-trace.pb?id=...&question_id=...
  GET /api/run/question/evidence.pb?id=...&question_id=...
  GET /api/run/question/events.pb?id=...&question_id=...&type=memory|provider|runner|recall|judge

Dataset / gold / raw data:
  GET /api/datasets.pb
  GET /api/dataset/questions.pb?dataset=...&q=&type=&offset=&limit=
  GET /api/dataset/question.pb?dataset=...&question_id=...
  GET /api/dataset/question/gold.pb?dataset=...&question_id=...
  GET /api/dataset/question/raw.pb?dataset=...&question_id=...&mode=redacted|raw

Raw observability:
  GET /api/run/trace-streams.pb?id=...
  GET /api/run/events.pb?id=...&type=&question_id=&q=&cursor=&limit=
  GET /api/run/spans.pb?id=...&type=&question_id=&cursor=&limit=

Materialized analytics/views:
  GET /api/run/trace-views/summary.pb?id=...
  GET /api/run/trace-views/waterfall.pb?id=...&scope=run|question&question_id=
  GET /api/run/analytics/stages.pb?id=...
  GET /api/run/analytics/cost.pb?id=...
  GET /api/run/analytics/provider-queues.pb?id=...
  GET /api/run/analytics/retrieval.pb?id=...
  GET /api/run/analytics/gold-coverage.pb?id=...
  GET /api/run/analytics/rank-distribution.pb?id=...&compare=...

Artifacts / validation / publication:
  GET /api/run/artifacts.pb?id=...
  GET /api/run/artifact.pb?id=...&artifact_id=...&cursor=&limit=&mode=summary|redacted|raw
  GET /api/run/capabilities.pb?id=...
  GET /api/run/validation.pb?id=...
  GET /api/run/publication-check.pb?id=...
```

## Immediate decision points

Before implementation, decide:

1. **Product IA names**
   - Keep `Leaderboard/Debugger` top-level and add workspace modes underneath?
   - Or move to `Discover / Analyze / Observe / Experiment / Maintain` as first-class nav?

2. **Primary initial persona**
   - Optimize first for system evaluator/buyer?
   - Or memory-system builder/debugger?
   - Or FAFO experiment runner?

3. **Question record ownership**
   - Should `QuestionRunRecord` become a required post-run artifact for every completed run?
   - Or generated lazily on first debugger access?

4. **Review/annotation scope**
   - Should MEMBENCH support durable human review states now?
   - Or stay read-only for benchmark artifacts until later?

5. **Remote execution scope**
   - Should Tuner remain CLI preview for now?
   - Or build local spawn first?
   - Or build remote job execution with cost/budget as the target?

6. **Billing/cost model**
   - Is “pay for run” a real product goal now?
   - If yes, cost estimation and budget caps must be first-class before remote launch.

7. **Trace redesign dependency**
   - Should trace/data model cleanup happen before product UI work?
   - Or should product work first establish question-centric records and data products while legacy traces remain behind adapters?

8. **Terminology**
   - Rename `Gold Coverage` to `Evidence Coverage`?
   - Rename `Traces` to `Observability` or split into subviews?
   - Rename `Tuner` to `Experiment Lab`?

9. **Comparability policy**
   - Should cohorts with different judge/qset/config be separated by default instead of warning-only?
   - Should leaderboard ranking force strict comparable groups?

10. **Terminal aesthetic**
   - Preserve current dense terminal aesthetic as expert mode?
   - Add guided/report modes for evaluator/stakeholder personas?

## Product thesis

MEMBENCH should not be just a leaderboard and not just a trace debugger.

The differentiated product is:

```text
A benchmark operating system for memory / hybrid / agentic systems:
  discover which systems work,
  understand why they work or fail,
  run controlled experiments,
  and maintain benchmark/data validity.
```

The product architecture should therefore be built around:

```text
System / Config / Benchmark / QuestionSet / Run / Question / Evidence / Trace / Experiment / Job / Artifact
```

not around current tabs or overloaded endpoints.

## Appendix — current endpoint-to-workflow map

| Current endpoint | Current screen(s) | Product data class |
|---|---|---|
| `/api/leaderboard.pb` | Leaderboard | Discover / cohort leaderboard |
| `/api/run.pb` | Overview, Tuner seed | Run summary + params/provenance |
| `/api/run/questions.pb` | Questions | Question run index |
| `/api/run/question-debug.pb?id&path` | Question modal | Question debug bundle, currently path-addressed |
| `/api/compare.pb` | Compare | Comparison report |
| `/api/run/traces.pb` | Traces | Mixed observability bundle: raw-ish + derived + workflow |
| `/api/run/gold-eval.pb` | Gold Coverage | Evidence/gold coverage artifact |
| `/api/run/live.pb` | Live | Live/final run telemetry snapshot |
| `/api/runner/schema.pb` | Tuner | Experiment parameter schema |
| `/api/runner/plan.pb` | Tuner | Command preview / run plan |
| `/api/run/artifact.pb` | Generic/export | Raw/materialized artifact access |

## Appendix — screen/data smell list

Current structural smells observed:

```text
Leaderboard combines public ranking, expert comparison, cohort validity warnings, and cost hints.
Overview mixes decision summary, raw params, artifact status, and cost/model calls.
Questions is question-centric but debug API is path-centric.
Compare is strong but lacks review/release decision workflow.
Traces includes derived charts, raw logs, provider queue summaries, and workflow state under one name.
Gold Coverage is actually evidence/retrieval/distillation coverage and benchmark audit.
Live is strong read-only observability but weak run control.
Tuner is a command builder, not yet an experiment control plane.
Registry sidebar is powerful but visually overwhelms every persona.
```
