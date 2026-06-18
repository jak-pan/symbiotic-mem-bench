<script lang="ts">
  import { store } from "../lib/store.svelte";
  import { router } from "../lib/router.svelte";
  import { pct } from "../lib/format";
  import type { RunSummary } from "../lib/types";
  import Overview from "../sections/Overview.svelte";
  import Questions from "../sections/Questions.svelte";
  import Compare from "../sections/Compare.svelte";
  import Traces from "../sections/Traces.svelte";
  import Tuner from "../sections/Tuner.svelte";
  import Live from "../sections/Live.svelte";

  let tab = $state<"overview" | "questions" | "compare" | "traces" | "tuner">("overview");
  let showStale = $state(false);

  const selectedId = $derived(router.arg || store.runs[0]?.run_id || "");
  const selected = $derived(store.byId(selectedId));
  const isPending = $derived(store.isPending(selectedId));

  // In-flight runs (not yet finalized). Running + idle-warning shown by default;
  // stalled behind a toggle.
  const pendingShown = $derived(
    store.pending.filter((p) => showStale || p.status !== "stalled"),
  );
  const staleCount = $derived(store.pending.filter((p) => p.status === "stalled").length);

  // Group runs into a system → benchmark → limit tree.
  type Group = { key: string; runs: RunSummary[] };
  const tree = $derived.by(() => {
    const map = new Map<string, RunSummary[]>();
    for (const r of store.runs) {
      const k = `${r.system} / ${r.benchmark} / ${r.limit ?? "?"}Q`;
      if (!map.has(k)) map.set(k, []);
      map.get(k)!.push(r);
    }
    return [...map.entries()]
      .map(([key, runs]) => ({
        key,
        runs: runs.sort((a, b) => (b.accuracy ?? 0) - (a.accuracy ?? 0)),
      }))
      .sort((a, b) => a.key.localeCompare(b.key));
  });

  const TABS = [
    ["overview", "OVERVIEW"],
    ["questions", "QUESTIONS"],
    ["compare", "COMPARE"],
    ["traces", "TRACES"],
    ["tuner", "TUNER"],
  ] as const;
</script>

<div class="dbg">
  <aside class="tree">
    {#if store.pending.length}
      <div class="tree-h flight">
        <span class="label">IN FLIGHT</span>
        {#if store.running.length}<span class="liven">● {store.running.length} live</span>{/if}
        {#if store.warning.length}<span class="warnn">◐ {store.warning.length} idle</span>{/if}
        {#if staleCount}
          <button class="staletog" class:on={showStale} onclick={() => (showStale = !showStale)}>
            {showStale ? "hide" : "show"} stale ({staleCount})
          </button>
        {/if}
      </div>
      <div class="flight-body">
        {#each pendingShown as p (p.run_id)}
          <button class="node pnode" class:on={p.run_id === selectedId} onclick={() => router.openRun(p.run_id)}>
            <span class="pdot {p.status}"></span>
            <span class="nname" title={p.run_name}>{p.run_name}</span>
            <span class="pprog mono-num">{p.ingested}/{p.limit ?? "?"}</span>
          </button>
        {/each}
        {#if !pendingShown.length}<div class="tree-empty">no running runs{#if staleCount} ({staleCount} stale hidden){/if}</div>{/if}
      </div>
    {/if}
    <div class="tree-h label">REGISTRY</div>
    <div class="tree-body">
      {#each tree as g (g.key)}
        <div class="grp">
          <div class="grp-h">{g.key}</div>
          {#each g.runs as r (r.run_id)}
            <button class="node" class:on={r.run_id === selectedId} onclick={() => router.openRun(r.run_id)}>
              <span class="dot" class:native={r.run_kind === "native"}></span>
              <span class="nname" title={r.run_name}>{r.run_name}</span>
              <span class="nacc mono-num">{pct(r.accuracy)}</span>
            </button>
          {/each}
        </div>
      {/each}
      {#if !store.runs.length}<div class="tree-empty">No runs found.</div>{/if}
    </div>
  </aside>

  <div class="work">
    <div class="tabbar">
      <div class="run-id">
        {#if isPending}
          {@const p = store.pendingById(selectedId)}
          <span class="rk chip {p?.status === 'running' ? 'green' : p?.status === 'warning' ? 'amber' : ''}">{p?.status === "running" ? "live" : p?.status === "warning" ? "idle" : "stalled"}</span>
          <span class="rid">{selectedId}</span>
        {:else if selected}
          <span class="rk {selected.run_kind === 'native' ? 'green' : 'cyan'} chip">{selected.run_kind === "imported-artifact" ? "import" : selected.run_kind}</span>
          <span class="rid">{selected.run_id}</span>
        {:else}
          <span class="rid faint">no run selected</span>
        {/if}
      </div>
      {#if isPending}
        <div class="tabs"><span class="tab on livetab">▶ LIVE MONITOR</span></div>
      {:else}
        <div class="tabs">
          {#each TABS as [id, lbl] (id)}
            <button class="tab" class:on={tab === id} onclick={() => (tab = id)}>{lbl}</button>
          {/each}
        </div>
      {/if}
    </div>

    <div class="tabview">
      {#if isPending}
        <Live id={selectedId} />
      {:else if !selected}
        <div class="empty">SELECT A RUN FROM THE REGISTRY</div>
      {:else if tab === "overview"}
        <Overview id={selectedId} />
      {:else if tab === "questions"}
        <Questions id={selectedId} />
      {:else if tab === "compare"}
        <Compare id={selectedId} {selected} />
      {:else if tab === "traces"}
        <Traces id={selectedId} />
      {:else if tab === "tuner"}
        <Tuner {selected} />
      {/if}
    </div>
  </div>
</div>

<style>
  .dbg {
    flex: 1;
    display: grid;
    grid-template-columns: 248px 1fr;
    gap: 1px;
    background: var(--border);
    min-height: 0;
  }
  .tree {
    background: var(--bg-panel);
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  .tree-h {
    padding: 7px 10px;
    border-bottom: 1px solid var(--border-bright);
  }
  .tree-h.flight {
    display: flex;
    align-items: center;
    gap: 8px;
    background: rgba(47, 207, 122, 0.05);
  }
  .liven {
    color: var(--green);
    font-size: 9.5px;
    font-weight: 700;
    letter-spacing: 0.04em;
  }
  .staletog {
    margin-left: auto;
    background: var(--bg-elev);
    border: 1px solid var(--border-bright);
    color: var(--text-faint);
    font-size: 8.5px;
    letter-spacing: 0.04em;
    padding: 1px 6px;
    cursor: pointer;
  }
  .staletog:hover,
  .staletog.on {
    color: var(--amber);
    border-color: var(--amber-dim);
  }
  .flight-body {
    border-bottom: 1px solid var(--border-bright);
    padding: 4px 0 6px;
    max-height: 200px;
    overflow: auto;
  }
  .pnode {
    grid-template-columns: 10px 1fr auto;
  }
  .pdot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--text-faint);
  }
  .pdot.running {
    background: var(--green);
    box-shadow: 0 0 6px var(--green);
    animation: pulse 1.4s ease infinite;
  }
  .pdot.warning {
    background: var(--amber);
    box-shadow: 0 0 5px var(--amber-dim);
  }
  .warnn {
    color: var(--amber);
    font-size: 9.5px;
    font-weight: 700;
    letter-spacing: 0.04em;
  }
  @keyframes pulse {
    50% {
      opacity: 0.35;
    }
  }
  .pprog {
    font-size: 10px;
    color: var(--cyan);
  }
  .livetab {
    color: var(--green) !important;
    box-shadow: inset 0 -2px 0 var(--green) !important;
    cursor: default;
  }
  .tree-body {
    overflow: auto;
    flex: 1;
    padding: 4px 0;
  }
  .grp {
    margin-bottom: 6px;
  }
  .grp-h {
    padding: 4px 10px;
    font-size: 9.5px;
    color: var(--text-faint);
    letter-spacing: 0.03em;
    border-bottom: 1px dotted var(--border);
  }
  .node {
    width: 100%;
    display: grid;
    grid-template-columns: 10px 1fr auto;
    align-items: center;
    gap: 7px;
    padding: 4px 10px 4px 14px;
    background: transparent;
    border: none;
    border-left: 2px solid transparent;
    cursor: pointer;
    text-align: left;
  }
  .node:hover {
    background: var(--bg-elev);
  }
  .node.on {
    background: var(--bg-sel);
    border-left-color: var(--amber);
  }
  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--cyan);
  }
  .dot.native {
    background: var(--green);
  }
  .nname {
    color: var(--text-dim);
    font-size: 11px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .node.on .nname {
    color: var(--text);
  }
  .nacc {
    font-size: 10.5px;
    color: var(--text-faint);
  }
  .tree-empty {
    padding: 14px;
    color: var(--text-faint);
    font-size: 11px;
  }

  .work {
    background: var(--bg);
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  .tabbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    border-bottom: 1px solid var(--border-bright);
    background: var(--bg-panel);
    flex: none;
  }
  .run-id {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 12px;
  }
  .rid {
    font-size: 11px;
    color: var(--text-dim);
  }
  .tabs {
    display: flex;
  }
  .tab {
    padding: 9px 16px;
    background: transparent;
    border: none;
    border-left: 1px solid var(--border);
    color: var(--text-faint);
    font-family: var(--sans);
    font-weight: 700;
    font-size: 10px;
    letter-spacing: 0.1em;
    cursor: pointer;
  }
  .tab:hover {
    color: var(--text);
    background: var(--bg-elev);
  }
  .tab.on {
    color: var(--amber);
    box-shadow: inset 0 -2px 0 var(--amber);
  }
  .tabview {
    flex: 1;
    min-height: 0;
    overflow: auto;
  }
  .empty {
    padding: 50px;
    text-align: center;
    color: var(--text-faint);
    letter-spacing: 0.2em;
  }
  .faint {
    color: var(--text-faint);
  }
</style>
