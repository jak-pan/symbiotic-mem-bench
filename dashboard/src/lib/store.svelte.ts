import { api } from "./api";
import type { LeaderboardSnapshot, PendingRun, RunSummary } from "./types";

/**
 * How this page is being served, decided once at boot:
 *
 * - `live` — an `/api` backend answered; the registry, the debugger and the
 *   in-flight panels all work, and the store polls for changes.
 * - `snapshot` — no backend, but the bundled `membench.leaderboard.v1` export
 *   is there. This is a static deploy. The API-only endpoints are *not* polled:
 *   asking for `/api/runs` on a static host is guaranteed to 404, and reporting
 *   that 404 as a connection error told the reader something false — the page
 *   is working exactly as designed, it just has no live backend.
 * - `offline` — neither. Something really is wrong, and the UI says so.
 */
export type AppMode = "boot" | "live" | "snapshot" | "offline";

// Shared, rune-backed app state: the scanned run index + connection status.
// Derived views are `$derived` so the per-second clock re-render of the status
// bar doesn't re-filter the whole registry on every tick.
class Store {
  runs = $state<RunSummary[]>([]);
  pending = $state<PendingRun[]>([]);
  loaded = $state(false);
  mode = $state<AppMode>("boot");
  error = $state<string | null>(null);
  /** The bundled static export, when running in snapshot mode. */
  snapshot = $state<LeaderboardSnapshot | null>(null);
  /** Server version/git, fetched once on mount for the status bar. */
  serverVersion = $state<string>("");
  serverSha = $state<string>("");
  /** UI bundle content hash and the full source commit from landing evidence. */
  uiBundle = $state<string>("");
  uiCommit = $state<string>("");

  get online(): boolean {
    return this.mode === "live";
  }
  get isSnapshot(): boolean {
    return this.mode === "snapshot";
  }

  /**
   * Decide the mode, then load what that mode actually has. Called once on
   * mount; nothing else may set `mode`.
   */
  async boot() {
    const [health, ui] = await Promise.all([
      api.health().catch(() => null),
      api.uiVersion().catch(() => null),
    ]);
    this.uiBundle = ui?.bundle ?? "";
    this.uiCommit = ui?.commit ?? "";

    if (health) {
      this.serverVersion = health.version ?? "";
      this.serverSha = health.binary_sha ?? "";
      this.mode = "live";
      await this.load();
      return;
    }

    try {
      this.snapshot = await api.leaderboardSnapshot();
      this.mode = "snapshot";
      this.loaded = true;
      this.error = null;
    } catch (e) {
      this.mode = "offline";
      this.error = (e as Error).message;
    }
  }

  /** Poll the live registry. A no-op unless a backend answered at boot. */
  async load() {
    if (this.mode !== "live") return;
    try {
      const [runs, pending] = await Promise.all([api.runs(), api.pending()]);
      // Skip the reassignment when nothing changed so derived views keep their
      // referential identity and downstream components don't re-render.
      if (this.runs !== runs) this.runs = runs;
      if (this.pending !== pending) this.pending = pending;
      this.loaded = true;
      this.error = null;
    } catch (e) {
      // A backend that answered at boot and then stopped is a real error.
      this.error = (e as Error).message;
      this.mode = "offline";
    }
  }

  get running(): PendingRun[] {
    return this.pending.filter((p) => p.status === "running");
  }
  get warning(): PendingRun[] {
    return this.pending.filter((p) => p.status === "warning");
  }
  // In-flight = actively running or idle-warning (not yet stalled).
  get active(): PendingRun[] {
    return this.pending.filter((p) => p.status !== "stalled");
  }
  isPending(id: string): boolean {
    return this.pending.some((p) => p.run_id === id);
  }
  pendingById(id: string): PendingRun | undefined {
    return this.pending.find((p) => p.run_id === id);
  }

  /** Ranked (therefore verified) rows of the bundled snapshot. */
  snapshotRanked = $derived(this.snapshot?.cohorts.flatMap((c) => c.rows) ?? []);

  /** Records the current mode actually knows about. */
  recordCount = $derived.by(() => {
    if (this.isSnapshot) {
      return this.snapshotRanked.length + (this.snapshot?.unranked.length ?? 0);
    }
    return this.runs.length;
  });
  /** Records that passed every review gate. Zero is a legitimate answer. */
  verifiedCount = $derived.by(() => {
    if (this.isSnapshot) return this.snapshotRanked.length;
    return this.runs.filter((r) => r.eligibility?.eligible).length;
  });

  systems = $derived.by(() => {
    if (this.isSnapshot) {
      return [
        ...new Set([
          ...this.snapshotRanked.map((r) => r.system),
          ...(this.snapshot?.unranked.map((r) => r.system) ?? []),
        ]),
      ];
    }
    return [...new Set(this.runs.map((r) => r.system))];
  });
  benchmarks = $derived.by(() => {
    if (this.isSnapshot) {
      return [
        ...new Set([
          ...this.snapshotRanked.map((r) => r.benchmark),
          ...(this.snapshot?.unranked.map((r) => r.benchmark) ?? []),
        ]),
      ];
    }
    return [...new Set(this.runs.map((r) => r.benchmark))];
  });
  /**
   * Best *verified* accuracy. Unranked records keep their measured numbers, but
   * a headline score must never be drawn from a record that failed the gate —
   * that is precisely the claim the gate exists to withhold.
   */
  bestAccuracy = $derived.by(() => {
    const rows = this.isSnapshot
      ? this.snapshotRanked
      : this.runs.filter((r) => r.eligibility?.eligible);
    const vals = rows.map((r) => r.accuracy).filter((v): v is number => v != null);
    return vals.length ? Math.max(...vals) : null;
  });
  byId(id: string): RunSummary | undefined {
    return this.runs.find((r) => r.run_id === id);
  }
}

export const store = new Store();
