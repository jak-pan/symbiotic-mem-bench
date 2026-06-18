import type {
  Cohort,
  CompareResponse,
  LiveResponse,
  PendingRun,
  QuestionRow,
  RunDetail,
  RunSummary,
  RunnerPreview,
  RunnerSchema,
  TracesResponse,
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
  health: () => get<{ ok: boolean }>("/health"),
  runs: () => get<{ runs: RunSummary[] }>("/runs").then((r) => r.runs),
  pending: () => get<{ pending: PendingRun[] }>("/pending").then((r) => r.pending),
  live: (id: string) => get<LiveResponse>(`/run/live?id=${enc(id)}`),
  leaderboard: (benchmark?: string, limit?: number) => {
    const q = new URLSearchParams();
    if (benchmark) q.set("benchmark", benchmark);
    if (limit != null) q.set("limit", String(limit));
    const qs = q.toString();
    return get<{ cohorts: Cohort[] }>(`/leaderboard${qs ? "?" + qs : ""}`).then(
      (r) => r.cohorts,
    );
  },
  run: (id: string) => get<RunDetail>(`/run?id=${enc(id)}`),
  questions: (id: string) =>
    get<{ total: number; questions: QuestionRow[] }>(
      `/run/questions?id=${enc(id)}`,
    ).then((r) => r.questions),
  artifact: (id: string, kind: string, offset = 0, limit = 200) =>
    get<any>(`/run/artifact?id=${enc(id)}&kind=${kind}&offset=${offset}&limit=${limit}`),
  traces: (id: string) => get<TracesResponse>(`/run/traces?id=${enc(id)}`),
  compare: (base: string, cand: string) =>
    get<CompareResponse>(`/compare?base=${enc(base)}&cand=${enc(cand)}`),
  runnerSchema: () => get<RunnerSchema>("/runner/schema"),
  runnerPlan: (params: Record<string, unknown>) =>
    post<RunnerPreview>("/runner/plan", params),
};
