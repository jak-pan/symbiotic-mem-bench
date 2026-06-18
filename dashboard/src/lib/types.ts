// DTOs mirroring the Rust server's JSON shapes (src/registry.rs, leaderboard.rs,
// compare.rs, cost.rs, artifacts.rs, runner.rs).

export interface RunSummary {
  run_id: string;
  origin: string;
  system: string;
  benchmark: string;
  limit: number | null;
  run_name: string;
  run_kind: string;
  config_label: string;
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
  created_at: string | null;
  modified_ms: number | null;
  per_question_type: Record<string, QTypeScore> | null;
  artifacts_available: string[];
  artifacts_missing: string[];
  native_state_available: boolean | null;
}

export interface QTypeScore {
  accuracy: number;
  n: number;
  correct: number;
}

export type RankedRow = RunSummary & { rank: number };

export interface Cohort {
  cohort_id: string;
  benchmark: string;
  limit: number | null;
  run_count: number;
  dataset_fingerprints: string[];
  judge_models: string[];
  strictly_comparable: boolean;
  best_accuracy: number | null;
  rows: RankedRow[];
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
  judge_model: string | null;
  router_pick: string | null;
  initial_pick: string | null;
  final_pick: string | null;
  error: string | null;
}

export interface ModelStat {
  model: string;
  operator: string;
  operation: string;
  calls: number;
  input_tokens: number;
  output_tokens: number;
  cost_micro_usd: number | null;
  latency_ms_p50: number | null;
}

export interface ModelRollup {
  calls: number;
  failed_calls: number;
  input_tokens: number;
  output_tokens: number;
  cost_micro_usd: number | null;
  latency_ms_p50: number | null;
  latency_ms_p95: number | null;
  models: ModelStat[];
  roles: Record<string, string>;
}

export interface RunDetail {
  summary: RunSummary;
  report: any;
  params: any;
  cohort: {
    dataset_fingerprint: string | null;
    judge_model: string | null;
    judge_prompt_mode: string | null;
    models: { answer?: string; distill?: string; embed?: string; judge?: string };
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
  default: any;
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

export interface WorkflowQueueDatabase {
  path: string;
  total_items: number;
  total_events: number;
  items_by_status: Record<string, number>;
  events_by_status: Record<string, number>;
  queues: Record<string, number>;
  retried_items: number;
  max_attempt: number;
  recent_errors: any[];
  recent_events: any[];
}

export interface WorkflowQueueSummary {
  databases: WorkflowQueueDatabase[];
}

export interface TracesResponse {
  memory_traces: { total: number; truncated: boolean; rows: any[] };
  model_rollup: ModelRollup | null;
  queue_timing: QueueTiming[] | null;
  workflow_queue: WorkflowQueueSummary | null;
}

export interface PendingRun {
  run_id: string;
  origin: string;
  system: string;
  benchmark: string;
  limit: number | null;
  run_name: string;
  config_label: string;
  status: "running" | "warning" | "stalled";
  started_ms: number | null;
  updated_ms: number | null;
  age_secs: number | null;
  hypotheses: number;
  ingested: number;
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

export interface StageProgress {
  operation: string;
  started: number;
  succeeded: number;
  failed: number;
  in_flight: number;
}

export interface LiveDetail {
  queue: QueuePressure;
  model: ModelLive;
  memory_stages: StageProgress[];
  memory_failures: number;
  errors: LiveErrorRow[];
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
