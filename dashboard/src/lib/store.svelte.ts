import { api } from "./api";
import type { PendingRun, RunSummary } from "./types";

// Shared, rune-backed app state: the scanned run index + connection status.
class Store {
  runs = $state<RunSummary[]>([]);
  pending = $state<PendingRun[]>([]);
  loaded = $state(false);
  online = $state(false);
  error = $state<string | null>(null);

  async load() {
    try {
      const [runs, pending] = await Promise.all([api.runs(), api.pending()]);
      this.runs = runs;
      this.pending = pending;
      this.loaded = true;
      this.online = true;
      this.error = null;
    } catch (e) {
      this.error = (e as Error).message;
      this.online = false;
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

  get systems(): string[] {
    return [...new Set(this.runs.map((r) => r.system))];
  }
  get benchmarks(): string[] {
    return [...new Set(this.runs.map((r) => r.benchmark))];
  }
  get bestAccuracy(): number | null {
    const vals = this.runs.map((r) => r.accuracy).filter((v): v is number => v != null);
    return vals.length ? Math.max(...vals) : null;
  }
  byId(id: string): RunSummary | undefined {
    return this.runs.find((r) => r.run_id === id);
  }
}

export const store = new Store();
