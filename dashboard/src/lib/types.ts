// DTOs mirroring the Rust server's JSON shapes (src/registry.rs, leaderboard.rs,
// compare.rs, cost.rs, artifacts.rs, runner.rs).

export interface RunSummary {
  run_id: string;
  origin: string;
  system: string;
  benchmark: string;
  limit: number | null;
  run_name: string;
  display_name: string;
  run_kind: string;
  registry_section: string;
  is_meta_record: boolean;
  tuning_cohort: string | null;
  tuning_shape: string | null;
  config_label: string;
  settings_label: string;
  accuracy: number | null;
  accuracy_correct: number | null;
  accuracy_total: number | null;
  task_averaged_accuracy: number | null;
  abstention_accuracy: number | null;
  cost_micro_usd: number | null;
  latency_ms_p50: number | null;
  latency_ms_p95: number | null;
  config_signature: string | null;
  cohort_id: string;
  dataset_fingerprint: string | null;
  judge_model: string | null;
  judge_prompt_mode: string | null;
  /** Oracle-gold run: gold evidence fed straight to the answerer (reader-ceiling method). */
  oracle_gold: boolean;
  created_at: string | null;
  modified_ms: number | null;
  per_question_type: Record<string, QTypeScore> | null;
  artifacts_available: string[];
  artifacts_missing: string[];
  native_state_available: boolean | null;
  is_trial_run: boolean;
  trial_markers: TrialMarker[];
  /** Synthetic contract fixture — never a real benchmark result. */
  fixture?: boolean;
  eligibility?: Eligibility;
}

export interface TrialMarker {
  stack_id: string;
  change_id: string;
  change_title: string;
  decision: string;
  analysis_path: string;
  compared_to_run_id: string | null;
  original_baseline_run_id: string | null;
  improvements: number;
  regressions: number;
  unchanged_wrong: number;
  unchanged_correct: number;
  question_count: number;
  sample_classification: string;
  focused: boolean;
  aggregate_accuracy: number | null;
  aggregate_correct: number | null;
  aggregate_total: number | null;
}

export interface QTypeScore {
  accuracy: number;
  correct: number;
  total: number;
}

export type RankedRow = RunSummary & { rank: number };

export interface Cohort {
  /** `{benchmark}::{limit}::ds:{fingerprint}::judge:{model}::mode:{prompt_mode}` —
   *  the full comparability identity, equal to each row's `cohort_id`. */
  cohort_id: string;
  benchmark: string;
  limit: number | null;
  run_count: number;
  /** The single fingerprint/judge/prompt mode shared by every row. */
  dataset_fingerprint: string | null;
  judge_model: string | null;
  judge_prompt_mode: string | null;
  dataset_fingerprints: string[];
  judge_models: string[];
  judge_prompt_modes: string[];
  strictly_comparable: boolean;
  best_accuracy: number | null;
  rows: RankedRow[];
}

// `membench.leaderboard.v1` — the static, publishable leaderboard document
// exported by `membench-leaderboard` (see docs/schemas.md). The SPA falls back
// to a bundled copy at /data/leaderboard.json when no /api backend is present.

export interface ReviewAttestation {
  reviewer: string;
  reviewed_at: string;
  reviewed_commit?: string | null;
  verdict: string;
}

/** One failed condition of the published review gate. */
export interface GateFailure {
  gate: string;
  detail: string;
}

/** Whether a record may be ranked, decided from bytes on disk (src/eligibility.rs). */
export interface Eligibility {
  eligible: boolean;
  level: "verified" | "unverified";
  missing_artifacts: string[];
  failures: GateFailure[];
  review?: ReviewAttestation | null;
}

export interface RowVerification {
  /** `verified` = every review gate passed; only verified rows are ranked. */
  level: "verified" | "unverified";
  missing_artifacts: string[];
  review?: ReviewAttestation | null;
}

/** Ranked rows in the export carry an extra per-row verification object. */
export type SnapshotRankedRow = RankedRow & { verification?: RowVerification };

export interface UnrankedRecord {
  run_id: string;
  run_name: string;
  /** `meta-record`, `unscored`, or `gate-failed`. */
  reason: string;
  failed_gates: GateFailure[];
  system: string;
  benchmark: string;
  limit?: number | null;
  accuracy?: number | null;
  accuracy_correct?: number | null;
  accuracy_total?: number | null;
  fixture?: boolean;
}

export interface LeaderboardSnapshot {
  schema: string;
  generated_at: string;
  source: {
    records_root: string;
    /** Content hash of the records tree — recomputable, unlike the commit sha. */
    records_digest: string | null;
    git_sha: string;
    run_count: number;
    ranked_count: number;
    unranked_count: number;
    contains_fixtures: boolean;
  };
  methodology: string;
  cohorts: Cohort[];
  unranked: UnrankedRecord[];
}

export interface QuestionRow {
  question_id: string;
  question_type: string | null;
  question: string | null;
  gold_answer: string | null;
  hypothesis: string | null;
  label: boolean | null;
  is_abstention: boolean | null;
  judge_raw: string | null;
  judge_system_prompt: string | null;
  judge_user_prompt: string | null;
  judge_model: string | null;
  router_pick: string | null;
  initial_pick: string | null;
  final_pick: string | null;
  debug_artifact: string | null;
  error: string | null;
}

export interface QueryPlannerCallDebug {
  mode?: string | null;
  system_prompt?: string | null;
  user_prompt?: string | null;
  response_text?: string | null;
  parsed_plan?: {
    canonical_query?: string | null;
    dense_queries?: string[] | null;
    sparse_terms?: string[] | null;
    expected_answer_type?: string | null;
    needs_raw_turns?: boolean | null;
  } | null;
  usage?: Record<string, unknown> | null;
  finish_reason?: string | null;
  error?: string | null;
}

export interface QuestionDebug {
  recall?: {
    query_planner_call?: QueryPlannerCallDebug | null;
    retrieval_queries?: string[] | null;
    query_plan?: Record<string, unknown> | null;
    initial_profile?: RetrievalProfileDebug | null;
    fallback_profile?: RetrievalProfileDebug | null;
    /** Reranker scoring trace (present when the run used a reranker). One entry
     *  per search profile (initial, then fallback). */
    rerank_trace?: RerankProfile[] | null;
    [key: string]: unknown;
  } | null;
  [key: string]: unknown;
}

export interface RerankCandidate {
  candidate_id: string;
  /** Original dense-retrieval rank/score before reranking.
   *  Fields are optional — older debug bundles may omit them. */
  embedding_rank?: number | null;
  embedding_score?: number | null;
  final_rank?: number | null;
  rerank_score?: number | null;
  text?: string | null;
}

export interface RerankProfile {
  candidate_type?: string | null;
  candidates: RerankCandidate[];
}

export interface AnswererCallDebug {
  phase?: string | null;
  context?: string[] | null;
  system_prompt?: string | null;
  prompt?: string | null;
  response_text?: string | null;
  processed_text?: string | null;
  selection_reason?: string | null;
  usage?: Record<string, unknown> | null;
  finish_reason?: string | null;
  error?: string | null;
}

export interface RetrievalProfileDebug {
  route?: string | null;
  facts?: FactEvidenceDebug[] | null;
  raw_turns?: RawTurnEvidenceDebug[] | null;
  [key: string]: unknown;
}

export interface FactEvidenceDebug {
  score?: number | null;
  fact?: {
    memory_id?: string | null;
    content?: string | null;
    event_time?: string | null;
    valid_from?: string | null;
    status?: string | null;
    tags?: string[] | null;
    source_refs?: Array<Record<string, unknown>> | null;
    [key: string]: unknown;
  } | null;
  [key: string]: unknown;
}

export interface RawTurnEvidenceDebug {
  score?: number | null;
  speaker?: string | null;
  text?: string | null;
  event_time?: string | null;
  ordinal?: number | null;
  source_ref?: Record<string, unknown> | null;
  [key: string]: unknown;
}

export interface ModelStat {
  model: string;
  operator: string;
  operation: string;
  calls: number;
  input_tokens: number;
  output_tokens: number;
  cost_micro_usd: number | null;
  cost_estimated: boolean;
  pricing_source: string | null;
  latency_ms_p50: number | null;
}

export interface ModelRollup {
  calls: number;
  failed_calls: number;
  input_tokens: number;
  output_tokens: number;
  cost_micro_usd: number | null;
  cost_estimated: boolean;
  pricing_table_version: string | null;
  pricing_sources: string[];
  latency_ms_p50: number | null;
  latency_ms_p95: number | null;
  models: ModelStat[];
  roles: Record<string, string>;
}

export interface RunDetail {
  summary: RunSummary;
  // Loose JSON passthrough from the server's `benchmark-report.json` blob. The
  // UI renders summary/cohort/cost directly; `report` is carried for raw
  // inspection only, so it stays untyped at the boundary.
  report: Record<string, unknown> | null;
  params: Record<string, unknown> | null;
  cohort: {
    dataset_fingerprint: string | null;
    judge_model: string | null;
    judge_prompt_mode: string | null;
    models: { answer?: string; distill?: string; embed?: string; rerank?: string; judge?: string };
    role_stats: ModelStat[];
    config_signature: string;
    cost_micro_usd: number | null;
    latency_ms_p50: number | null;
    latency_ms_p95: number | null;
    cached_input_tokens: number | null;
    uncached_input_tokens: number | null;
    response_cache_hits: number | null;
    prompt_cache_hits: number | null;
    prompt_cache_partial_hits: number | null;
    prompt_cache_misses: number | null;
  };
  cost: ModelRollup | null;
}

export interface CompareCounts {
  common: number;
  newly_correct: number;
  newly_wrong: number;
  unchanged_correct: number;
  unchanged_wrong: number;
  abstention_changes: number;
  only_in_base: number;
  only_in_candidate: number;
}

export interface TypeDelta {
  question_type: string;
  n: number;
  base_accuracy: number;
  candidate_accuracy: number;
  delta: number;
}

export interface ChangedRow {
  question_id: string;
  question_type: string | null;
  question: string | null;
  gold_answer: string | null;
  base_hypothesis: string | null;
  base_label: boolean | null;
  candidate_hypothesis: string | null;
  candidate_label: boolean | null;
  transition: string;
}

export interface CompareResult {
  base_accuracy: number | null;
  candidate_accuracy: number | null;
  accuracy_delta: number | null;
  counts: CompareCounts;
  per_type: TypeDelta[];
  changed: ChangedRow[];
}

export interface CompareResponse {
  base: RunSummary;
  candidate: RunSummary;
  result: CompareResult;
}

export interface ParamField {
  name: string;
  label: string;
  kind: "path" | "int" | "bool" | "enum" | "string";
  default: unknown;
  options: string[];
  observed?: string[];
  group: string;
  help: string;
  required: boolean;
}

export interface RunnerSchema {
  system: string;
  benchmark: string;
  fields: ParamField[];
}

export interface PlannedCommand {
  program: string;
  args: string[];
}

export interface RunnerPreview {
  run_root: string;
  run_command: PlannedCommand;
  run_shell: string;
  score_command: PlannedCommand | null;
  score_shell: string | null;
  env_defaults: [string, string][];
  warnings: string[];
}

export interface QueueTiming {
  queue_id: string;
  item_id: string;
  operation: string;
  attempts: number;
  wait_ms?: number;
  run_ms?: number;
  total_ms?: number;
  final_status?: string;
}

/** Aggregated per-queue timing row shown in the Provider Queue Summary panel. */
export interface QueueSummaryRow {
  name: string;
  count: number;
  failed: number;
  wait_p50: number | null;
  wait_p80: number | null;
  wait_p95: number | null;
  wait_p98: number | null;
  run_p50: number | null;
  run_p80: number | null;
  run_p95: number | null;
  run_p98: number | null;
  total_p50: number | null;
  total_p80: number | null;
  total_p95: number | null;
  total_p98: number | null;
}

export interface WorkflowQueueEventRow {
  item_id: string;
  queue_id: string;
  kind: string;
  status: string;
  attempt: number;
  timestamp?: string | null;
  error?: string | null;
}

export interface WorkflowQueueDatabase {
  path: string;
  total_items: number;
  total_events: number;
  items_by_status: Record<string, number>;
  events_by_status: Record<string, number>;
  queues: Record<string, number>;
  retried_items: number;
  max_attempt: number;
  recent_errors: WorkflowQueueEventRow[];
  recent_events: WorkflowQueueEventRow[];
}

export interface WorkflowQueueSummary {
  databases: WorkflowQueueDatabase[];
}

export interface TracesResponse {
  memory_traces: { total: number; truncated: boolean; rows: unknown[] };
  memory_stage_timing: Array<{
    operation: string;
    events: number;
    batch_events: number;
    intermediate_failed: number;
    failed: number;
    item_count: number;
    item_unit: string;
    work_ms_p50: number | null;
    work_ms_p80: number | null;
    work_ms_p95: number | null;
    work_ms_p98: number | null;
    numeric_metrics?: Record<string, {
      count: number;
      p50: number | null;
      p80: number | null;
      p95: number | null;
      p98: number | null;
      max: number | null;
    }>;
  }>;
  model_rollup: ModelRollup | null;
  queue_timing: QueueTiming[] | null;
  trace_waterfall: TraceWaterfall | null;
  dependency_waterfall: DependencyWaterfall | null;
  trace_events: TraceEventStream | null;
  workflow_queue: WorkflowQueueSummary | null;
}

export interface TraceEventRow {
  timestamp: string;
  kind: "memory" | "provider" | string;
  operation: string;
  lane: string;
  event: string;
  status: string;
  attempt: number;
  duration_ms: number | null;
  wait_ms: number | null;
  run_ms: number | null;
  total_ms: number | null;
  item_count: number;
  item_unit: string;
  source: string;
  error: string | null;
}

export interface TraceEventStream {
  total: number;
  truncated: boolean;
  rows: TraceEventRow[];
}

export interface TraceWaterfallBlock {
  kind: "memory_work" | "memory_failed" | "provider_wait" | "provider_run" | "provider_failed" | string;
  start_ms: number;
  end_ms: number;
  duration_ms: number;
  label: string;
  status: string;
  source: string;
  item_count: number;
  item_unit: string;
}

export interface TraceWaterfallLane {
  name: string;
  kind: "memory" | "provider" | string;
  blocks: TraceWaterfallBlock[];
}

export interface TraceWaterfall {
  timeline_start: string | null;
  timeline_end: string | null;
  duration_ms: number;
  block_count: number;
  truncated: boolean;
  lanes: TraceWaterfallLane[];
}

export interface DependencyWaterfallBlock {
  kind: string;
  label: string;
  start_ms: number;
  end_ms: number;
  duration_ms: number;
  item_count: number;
  item_unit: string;
}

export interface DependencyWaterfallLane {
  source: string;
  wait_ms: number;
  setup_ms?: number;
  blocks: DependencyWaterfallBlock[];
}

export interface DependencyWaterfall {
  timeline_start: string | null;
  timeline_end: string | null;
  duration_ms: number;
  lanes: DependencyWaterfallLane[];
}

export interface PendingRun {
  run_id: string;
  origin: string;
  system: string;
  benchmark: string;
  limit: number | null;
  run_name: string;
  config_label: string;
  settings_label: string;
  status: "running" | "warning" | "stalled" | "complete";
  started_ms: number | null;
  updated_ms: number | null;
  age_secs: number | null;
  hypotheses: number;
  ingested: number;
  /** Oracle-gold run: gold evidence fed straight to the answerer (reader-ceiling method). */
  oracle_gold: boolean;
}

export interface QueuePressure {
  queued: number;
  running: number;
  succeeded: number;
  failed: number;
  dead: number;
  in_flight: number;
  window: number;
}

export interface QueueBreakdown {
  queue_id: string;
  operation: string;
  queued: number;
  running: number;
  succeeded: number;
  failed: number;
  dead: number;
  in_flight: number;
  window: number;
  queued_units: number;
  running_units: number;
  succeeded_units: number;
  failed_units: number;
  dead_units: number;
  in_flight_units: number;
  observed_peak_running: number;
  observed_peak_running_units: number;
  starts_last_minute: number;
  starts_last_minute_units: number;
  peak_starts_per_minute: number;
  peak_starts_per_minute_units: number;
  avg_running: number;
  avg_running_units: number;
  avg_queued: number;
  avg_queued_units: number;
  avg_starts_per_minute: number;
  avg_starts_per_minute_units: number;
  observed_duration_secs: number;
  last_event_at: string | null;
}

export interface ModelLive {
  window_calls: number;
  window_failed: number;
  input_tokens: number;
  output_tokens: number;
  total_bytes: number;
}

export interface LiveErrorRow {
  timestamp: string | null;
  source: string;
  kind: string | null;
  message: string;
}

export interface ErrorCategory {
  category: string;
  source: string;
  kind: string | null;
  count: number;
}

export interface StageSegment {
  id: string;
  started: number;
  succeeded: number;
  failed: number;
  in_flight: number;
  item_succeeded: number;
  item_failed: number;
  progress: number;
  status: string;
}

export interface StageProgress {
  operation: string;
  started: number;
  succeeded: number;
  failed: number;
  item_succeeded: number;
  item_failed: number;
  item_unit: string;
  in_flight: number;
  intermediate_failed: number;
  segments: StageSegment[];
  last_event: string | null;
  last_event_at: string | null;
}

export interface LiveActivityRow {
  timestamp: string | null;
  source: string;
  operation: string;
  status: string;
  queue_id: string | null;
  message: string;
  severity: string;
}

export interface LiveDetail {
  queue: QueuePressure;
  queues: QueueBreakdown[];
  model: ModelLive;
  memory_stages: StageProgress[];
  memory_failures: number;
  error_categories: ErrorCategory[];
  errors: LiveErrorRow[];
  activity: LiveActivityRow[];
}

export interface LiveResponse {
  pending: PendingRun;
  detail: LiveDetail;
}

export const QTYPES = [
  "single-session-user",
  "single-session-assistant",
  "single-session-preference",
  "multi-session",
  "temporal-reasoning",
  "knowledge-update",
] as const;

// `gold-eval.json` — gold-evidence coverage per run (membench gold-eval).
export type GoldClass = "correct" | "reader_fail" | "retrieval_gap";

export interface GoldEvalQuestion {
  qid: string;
  type: string | null;
  answer: unknown;
  n_gold_pieces: number;
  covered_pieces: number;
  missing_pieces: string[];
  covered_by_fact: number;
  covered_by_raw: number;
  gold_top_rank: number | null;
  gold_deepest_rank: number | null;
  // Deepest (worst) gold turn's rank among the RAW-TURN candidates, re-ranked
  // among themselves by embedding score (embed) and by rerank score (rerank).
  // Null when no gold turn appears in the candidate set. Comparable across runs.
  gold_embed_rank: number | null;
  gold_rerank_rank: number | null;
  gold_turns_in_set: number;
  gold_turns_total: number;
  correct: boolean;
  abstained: boolean;
  class: GoldClass;
}

// Top-N recall of the deepest gold turn, after embedding vs after rerank.
export interface GoldRankDistribution {
  n: number;
  within_10: number;
  within_20: number;
  within_50: number;
  within_100: number;
  mean: number;
}

export interface GoldRankSummary {
  embed: GoldRankDistribution;
  rerank: GoldRankDistribution;
  gold_turns_in_set: number;
  gold_turns_total: number;
  gold_turn_in_set_pct: number;
}

export interface GoldEvalSummary {
  total: number;
  correct: number;
  wrong: number;
  abstained: number;
  single_piece: number;
  multi_piece: number;
  gold_pieces_needed: number;
  gold_pieces_covered: number;
  piece_coverage: number;
  class_counts: Record<GoldClass, number>;
  coverage_by_source: { fact: number; raw: number; both: number; none: number };
  // Present on artifacts regenerated after the embed-vs-rerank feature landed;
  // optional so older gold-eval.json still type-checks.
  gold_rank?: GoldRankSummary;
}

export interface GoldEvalResponse {
  schema_version: number;
  run_name: string;
  dataset_path: string;
  summary: GoldEvalSummary;
  questions: GoldEvalQuestion[];
}
