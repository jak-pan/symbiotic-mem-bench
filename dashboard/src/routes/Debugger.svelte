<script lang="ts">
  import { store } from "../lib/store.svelte";
  import { router } from "../lib/router.svelte";
  import type { DebugSubscreen } from "../lib/router.svelte";
  import { trialBadge, runKindLabel, runKindChipClass } from "../lib/run";
  import RegistryTree from "../components/RegistryTree.svelte";
  import Overview from "../sections/Overview.svelte";
  import Questions from "../sections/Questions.svelte";
  import Compare from "../sections/Compare.svelte";
  import Traces from "../sections/Traces.svelte";
  import Tuner from "../sections/Tuner.svelte";
  import Live from "../sections/Live.svelte";

  const selectedId = $derived(router.arg || store.runs[0]?.run_id || "");
  const selected = $derived(store.byId(selectedId));
  const isPending = $derived(store.isPending(selectedId));
  const hasNativeState = $derived(Boolean(selected?.native_state_available));

  const tabs = $derived.by(() => {
    const base = [
      ["overview", "OVERVIEW"],
      ["questions", "QUESTIONS"],
      ["compare", "COMPARE"],
      ["traces", "TRACES"],
    ] as const;
    const live = hasNativeState ? ([["live", "LIVE"]] as const) : [];
    return [...base, ...live, ["tuner", "TUNER"] as const];
  });

  const activeTab = $derived.by<DebugSubscreen>(() => {
    if (isPending) return "live";
    const requested = router.subscreen as DebugSubscreen;
    return tabs.some(([id]) => id === requested) ? requested : "overview";
  });
</script>

<div class="dbg">
  <RegistryTree {selectedId} activeTab={activeTab} />

  <div class="work">
    <div class="tabbar">
      <div class="run-id">
        {#if isPending}
          {@const p = store.pendingById(selectedId)}
          <span class="rk chip {p?.status === 'running' ? 'green' : p?.status === 'warning' ? 'amber' : ''}">{p?.status === "running" ? "live" : p?.status === "warning" ? "idle" : "stalled"}</span>
          <span class="rid">{selectedId}</span>
        {:else if selected}
          <span class="rk {runKindChipClass(selected.run_kind)} chip">{runKindLabel(selected.run_kind)}</span>
          {#if selected.is_trial_run}<span class="rk chip amber">{trialBadge(selected)}</span>{/if}
          <span class="rid">{selected.run_id}</span>
        {:else}
          <span class="rid faint">no run selected</span>
        {/if}
      </div>
      {#if isPending}
        <div class="tabs"><span class="tab on livetab">▶ LIVE MONITOR</span></div>
      {:else}
        <div class="tabs">
          {#each tabs as [id, lbl] (id)}
            <button class="tab" class:on={activeTab === id} onclick={() => router.openRunSubscreen(selectedId, id)}>{lbl}</button>
          {/each}
        </div>
      {/if}
    </div>

    <div class="tabview">
      {#if isPending}
        <Live id={selectedId} />
      {:else if !selected}
        <div class="empty">SELECT A RUN FROM THE REGISTRY</div>
      {:else if activeTab === "overview"}
        <Overview id={selectedId} />
      {:else if activeTab === "questions"}
        <Questions id={selectedId} />
      {:else if activeTab === "compare"}
        <Compare id={selectedId} {selected} />
      {:else if activeTab === "traces"}
        <Traces id={selectedId} />
      {:else if activeTab === "live" && hasNativeState}
        <Live id={selectedId} />
      {:else if activeTab === "tuner"}
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
    min-width: 0;
  }
  .work {
    background: var(--bg);
    display: flex;
    flex-direction: column;
    min-height: 0;
    min-width: 0;
    overflow: hidden;
  }
  .tabbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    border-bottom: 1px solid var(--border-bright);
    background: var(--bg-panel);
    flex: none;
    min-width: 0;
  }
  .run-id {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 12px;
    min-width: 0;
    flex: 1;
  }
  .rid {
    font-size: 11px;
    color: var(--text-dim);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tabs {
    display: flex;
    flex: none;
    min-width: 0;
    overflow-x: auto;
  }
  .tab {
    padding: 9px 13px;
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
  .livetab {
    color: var(--green) !important;
    box-shadow: inset 0 -2px 0 var(--green) !important;
    cursor: default;
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
