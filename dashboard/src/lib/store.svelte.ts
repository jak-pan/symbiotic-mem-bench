import { api } from "./api";
import type { PendingRun, RunSummary } from "./types";

// Shared, rune-backed app state: the scanned run index + connection status.
// Derived views are `$derived` so the per-second clock re-render of the status
// bar doesn't re-filter the whole registry on every tick.
class Store {
  runs = $state<RunSummary[]>([]);
  pending = $state<PendingRun[]>([]);
  loaded = $state(false);
  online = $state(false);
  error = $state<string | null>(null);
  /** Server version/git, fetched once on mount for the status bar. */
  serverVersion = $state<string>("");
  serverSha = $state<string>("");
  /** UI bundle content hash (the Vite de-cache hash) + build time. */
  uiBundle = $state<string>("");
  uiBuilt = $state<string>("");

  async loadVersion() {
    try {
      const [h, ui] = await Promise.all([
        api.health().catch(() => null),
        api.uiVersion().catch(() => null),
      ]);
      this.serverVersion = h?.version ?? "";
      this.serverSha = h?.binary_sha ?? "";
      this.uiBundle = ui?.bundle ?? "";
      this.uiBuilt = ui?.built ?? "";
      this.online = h != null;
    } catch {
      this.online = false;
    }
  }

  async load() {
    try {
      const [runs, pending] = await Promise.all([api.runs(), api.pending()]);
      // Skip the reassignment when nothing changed so derived views keep their
      // referential identity and downstream components don't re-render.
      if (this.runs !== runs) this.runs = runs;
      if (this.pending !== pending) this.pending = pending;
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

  systems = $derived([...new Set(this.runs.map((r) => r.system))]);
  benchmarks = $derived([...new Set(this.runs.map((r) => r.benchmark))]);
  bestAccuracy = $derived.by(() => {
    const vals = this.runs.map((r) => r.accuracy).filter((v): v is number => v != null);
    return vals.length ? Math.max(...vals) : null;
  });
  byId(id: string): RunSummary | undefined {
    return this.runs.find((r) => r.run_id === id);
  }
}

export const store = new Store();
