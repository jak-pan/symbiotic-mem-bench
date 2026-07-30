import type {
  Cohort,
  CompareResponse,
  GoldEvalResponse,
  LeaderboardSnapshot,
  LiveResponse,
  PendingRun,
  QuestionDebug,
  QuestionRow,
  RunDetail,
  RunSummary,
  RunnerPreview,
  RunnerSchema,
  TracesResponse,
  UnrankedRecord,
} from "./types";

async function get<T>(path: string): Promise<T> {
  const res = await fetch(`/api${path}`);
  if (!res.ok) {
    const body = await res.json().catch(() => ({}));
    throw new Error(body.error ?? `${res.status} ${res.statusText}`);
  }
  return res.json();
}

async function post<T>(path: string, body: unknown): Promise<T> {
  const res = await fetch(`/api${path}`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(body),
  });
  if (!res.ok) throw new Error(`${res.status} ${res.statusText}`);
  return res.json();
}

const enc = encodeURIComponent;

export const api = {
  health: () => get<{ ok: boolean; version?: string; git_sha?: string; binary_sha?: string }>("/health"),
  uiVersion: async () => {
    // Static sidecar written by scripts/write-version.mjs after each build.
    // Fetch with no-store so a rebuilt bundle's new hash is always visible.
    const res = await fetch("/version.json", { cache: "no-store" });
    if (!res.ok) throw new Error(`${res.status} ${res.statusText}`);
    return res.json() as Promise<{
      schema: string;
      version: string;
      tag: string;
      commit: string;
      records_digest: string;
      snapshot_sha256: string;
      dist_tree_sha256: string;
      bundle: string;
    }>;
  },
  runs: () => get<{ runs: RunSummary[] }>("/runs").then((r) => r.runs),
  pending: () => get<{ pending: PendingRun[] }>("/pending").then((r) => r.pending),
  live: (id: string) => get<LiveResponse>(`/run/live?id=${enc(id)}`),
  // Live counterpart of the static export: the same ranked cohorts *and* the
  // same exclusion list, so the two surfaces cannot tell different stories.
  leaderboard: (benchmark?: string, limit?: number) => {
    const q = new URLSearchParams();
    if (benchmark) q.set("benchmark", benchmark);
    if (limit != null) q.set("limit", String(limit));
    const qs = q.toString();
    return get<{ cohorts: Cohort[]; unranked: UnrankedRecord[] }>(
      `/leaderboard${qs ? "?" + qs : ""}`,
    );
  },
  leaderboardSnapshot: async () => {
    // Static `membench.leaderboard.v1` export bundled into the SPA at build
    // time (dashboard/public/data/leaderboard.json). Only used as a fallback
    // when the live /api backend is unreachable — e.g. a static deploy.
    const res = await fetch("/data/leaderboard.json", { cache: "no-store" });
    if (!res.ok) throw new Error(`${res.status} ${res.statusText}`);
    const doc = (await res.json()) as LeaderboardSnapshot;
    if (doc.schema !== "membench.leaderboard.v1") {
      throw new Error(`unexpected leaderboard schema: ${doc.schema}`);
    }
    return doc;
  },
  run: (id: string) => get<RunDetail>(`/run?id=${enc(id)}`),
  questions: (id: string) =>
    get<{ total: number; questions: QuestionRow[] }>(
      `/run/questions?id=${enc(id)}`,
    ).then((r) => r.questions),
  questionDebug: (id: string, path: string) =>
    get<{ path: string; json: QuestionDebug }>(
      `/run/question-debug?id=${enc(id)}&path=${enc(path)}`,
    ).then((r) => r.json),
  artifact: (id: string, kind: string, offset = 0, limit = 200) =>
    get<any>(`/run/artifact?id=${enc(id)}&kind=${kind}&offset=${offset}&limit=${limit}`),
  traces: (id: string) => get<TracesResponse>(`/run/traces?id=${enc(id)}`),
  goldEval: (id: string) =>
    // Non-jsonl artifacts are served as `{ kind, json }` — unwrap to the payload.
    get<{ kind: string; json: GoldEvalResponse }>(
      `/run/artifact?id=${enc(id)}&kind=gold_eval`,
    ).then((r) => r.json),
  compare: (base: string, cand: string) =>
    get<CompareResponse>(`/compare?base=${enc(base)}&cand=${enc(cand)}`),
  runnerSchema: () => get<RunnerSchema>("/runner/schema"),
  runnerPlan: (params: Record<string, unknown>) =>
    post<RunnerPreview>("/runner/plan", params),
};
