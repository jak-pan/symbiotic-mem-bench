<script lang="ts">
  import { store } from "../lib/store.svelte";
  import { router } from "../lib/router.svelte";
  import type { DebugSubscreen } from "../lib/router.svelte";
  import { ago, pct } from "../lib/format";
  import type { RunSummary } from "../lib/types";
  import Overview from "../sections/Overview.svelte";
  import Questions from "../sections/Questions.svelte";
  import Compare from "../sections/Compare.svelte";
  import Traces from "../sections/Traces.svelte";
  import Tuner from "../sections/Tuner.svelte";
  import Live from "../sections/Live.svelte";

  let showStale = $state(false);
  let registrySort = $state<"score" | "newest" | "oldest">("score");
  let showBenchmarks = $state(true);
  let showTuning = $state(true);
  let showTrials = $state(true);
  let showRuns = $state(true);
  let showRecords = $state(true);

  const selectedId = $derived(router.arg || store.runs[0]?.run_id || "");
  const selected = $derived(store.byId(selectedId));
  const isPending = $derived(store.isPending(selectedId));
  const hasNativeState = $derived(Boolean(selected?.native_state_available));

  // In-flight runs (not yet finalized). Running + idle-warning shown by default;
  // stalled behind a toggle.
  const pendingShown = $derived(
    store.pending.filter((p) => showStale || p.status !== "stalled"),
  );
  const staleCount = $derived(store.pending.filter((p) => p.status === "stalled").length);

  // Group runs into explicit registry sections so diagnostic transport tuning
  // records don't visually merge with benchmark-score runs.
  type Group = { key: string; label: string; sublabel: string; runs: RunSummary[] };
  const filteredRuns = $derived.by(() =>
    store.runs.filter((run) => sectionAllowed(run) && originAllowed(run)),
  );
  const tree = $derived.by(() => {
    const map = new Map<string, Group>();
    for (const r of filteredRuns) {
      const meta = groupMeta(r);
      if (!map.has(meta.key)) map.set(meta.key, { ...meta, runs: [] });
      map.get(meta.key)!.runs.push(r);
    }
    return [...map.values()]
      .map((group) => ({
        ...group,
        runs: [...group.runs].sort(sortRuns),
      }))
      .sort((a, b) => a.key.localeCompare(b.key));
  });
  const hiddenRegistryCount = $derived(store.runs.length - filteredRuns.length);

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

  function trialBadge(run: RunSummary): string {
    return run.trial_markers.some((marker) => marker.focused) ? "FOCUSED" : "TRIAL";
  }

  function sectionAllowed(run: RunSummary): boolean {
    if (run.registry_section === "tuning") return showTuning;
    if (run.registry_section === "trials") return showTrials;
    return showBenchmarks;
  }

  function originAllowed(run: RunSummary): boolean {
    if (run.origin === "records") return showRecords;
    return showRuns;
  }

  function groupMeta(run: RunSummary): Omit<Group, "runs"> {
    const source = run.origin === "records" ? "records" : "runs";
    if (run.registry_section === "tuning") {
      const cohort = run.tuning_cohort ?? "embedding transport";
      return {
        key: `1:tuning:${source}:${cohort}:${run.limit ?? "?"}`,
        label: `tuning · ${source}`,
        sublabel: `${cohort} / ${run.limit ?? "?"}Q`,
      };
    }
    if (run.registry_section === "trials") {
      return {
        key: `2:trials:${source}:${run.system}:${run.benchmark}:${run.limit ?? "?"}`,
        label: `trials · ${source}`,
        sublabel: `${run.system} / ${run.benchmark} / ${run.limit ?? "?"}Q`,
      };
    }
    return {
      key: `3:bench:${source}:${run.system}:${run.benchmark}:${run.limit ?? "?"}`,
      label: `benchmark · ${source}`,
      sublabel: `${run.system} / ${run.benchmark} / ${run.limit ?? "?"}Q`,
    };
  }

  function sourceBadge(run: RunSummary): string {
    if (run.is_meta_record) return "META";
    return run.origin === "records" ? "REC" : "RUN";
  }

  function runTime(run: RunSummary): number {
    if (run.modified_ms != null) return run.modified_ms;
    const created = run.created_at ? Date.parse(run.created_at) : NaN;
    return Number.isFinite(created) ? created : 0;
  }

  function sortRuns(a: RunSummary, b: RunSummary): number {
    if (registrySort === "newest") return runTime(b) - runTime(a);
    if (registrySort === "oldest") return runTime(a) - runTime(b);
    return (b.accuracy ?? -1) - (a.accuracy ?? -1) || runTime(b) - runTime(a);
  }

  function registryMeta(run: RunSummary): string {
    return registrySort === "score" ? pct(run.accuracy) : ago(runTime(run));
  }

  function toggleKind(kind: "benchmarks" | "tuning" | "trials") {
    const nextBench = kind === "benchmarks" ? !showBenchmarks : showBenchmarks;
    const nextTuning = kind === "tuning" ? !showTuning : showTuning;
    const nextTrials = kind === "trials" ? !showTrials : showTrials;
    if (!nextBench && !nextTuning && !nextTrials) return;
    showBenchmarks = nextBench;
    showTuning = nextTuning;
    showTrials = nextTrials;
  }

  function toggleSource(source: "runs" | "records") {
    const nextRuns = source === "runs" ? !showRuns : showRuns;
    const nextRecords = source === "records" ? !showRecords : showRecords;
    if (!nextRuns && !nextRecords) return;
    showRuns = nextRuns;
    showRecords = nextRecords;
  }

  function openRegistryRun(run: RunSummary) {
    const target =
      activeTab === "live" && !run.native_state_available ? "overview" : activeTab;
    router.openRun(run.run_id, target);
  }
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
          <button class="node pnode" class:on={p.run_id === selectedId} onclick={() => router.openRun(p.run_id, "live")}>
            <span class="pdot {p.status}"></span>
            <span class="nname" title={p.run_name}>{p.run_name}</span>
            <span class="pprog mono-num">{p.ingested}/{p.limit ?? "?"}</span>
          </button>
        {/each}
        {#if !pendingShown.length}<div class="tree-empty">no running runs{#if staleCount} ({staleCount} stale hidden){/if}</div>{/if}
      </div>
    {/if}
    <div class="tree-h registry-h">
      <span class="label">REGISTRY</span>
      <select class="sortpick" bind:value={registrySort} aria-label="Sort registry runs">
        <option value="score">score</option>
        <option value="newest">newest</option>
        <option value="oldest">oldest</option>
      </select>
    </div>
    <div class="filterbar" aria-label="Registry run kind filters">
      <button class:on={showBenchmarks} onclick={() => toggleKind("benchmarks")}>bench</button>
      <button class:on={showTuning} onclick={() => toggleKind("tuning")}>tuning</button>
      <button class:on={showTrials} onclick={() => toggleKind("trials")}>trials</button>
    </div>
    <div class="filterbar source" aria-label="Registry source filters">
      <button class:on={showRuns} onclick={() => toggleSource("runs")}>runs</button>
      <button class:on={showRecords} onclick={() => toggleSource("records")}>records</button>
      {#if hiddenRegistryCount > 0}<span class="hidden-count">{hiddenRegistryCount} hidden</span>{/if}
    </div>
    <div class="tree-body">
      {#each tree as g (g.key)}
        <div class="grp">
          <div class="grp-h"><span>{g.label}</span><small>{g.sublabel}</small></div>
          {#each g.runs as r (r.run_id)}
            <button class="node" class:on={r.run_id === selectedId} onclick={() => openRegistryRun(r)}>
              <span class="dot" class:native={r.run_kind === "native"} class:trial={r.is_trial_run} class:tuning={r.registry_section === "tuning"}></span>
              <span class="nname" title={`${r.run_name} · ${r.run_id}`}>
                {r.display_name || r.run_name}
                {#if r.is_trial_run}<span class="trial-pill">{trialBadge(r)}</span>{/if}
                <span class="origin-pill" class:meta={r.is_meta_record}>{sourceBadge(r)}</span>
              </span>
              <span class="nacc mono-num" title={`${pct(r.accuracy)} score · ${ago(runTime(r))}`}>
                {registryMeta(r)}
              </span>
            </button>
          {/each}
        </div>
      {/each}
      {#if !store.runs.length}
        <div class="tree-empty">No runs found.</div>
      {:else if !filteredRuns.length}
        <div class="tree-empty">No runs match the active toggles.</div>
      {/if}
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
  .registry-h {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .sortpick {
    margin-left: auto;
    max-width: 90px;
    background: var(--bg-elev);
    border: 1px solid var(--border-bright);
    color: var(--text-dim);
    font-size: 9.5px;
    padding: 2px 5px;
    cursor: pointer;
  }
  .sortpick:hover,
  .sortpick:focus {
    color: var(--amber);
    border-color: var(--amber-dim);
    outline: none;
  }
  .filterbar {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 5px 10px 0;
    background: var(--bg-panel);
  }
  .filterbar.source {
    padding-top: 4px;
    padding-bottom: 5px;
    border-bottom: 1px solid var(--border-bright);
  }
  .filterbar button {
    flex: 0 0 auto;
    background: var(--bg-row);
    border: 1px solid var(--border);
    color: var(--text-faint);
    cursor: pointer;
    font-size: 8.5px;
    letter-spacing: 0.05em;
    padding: 1px 5px;
    text-transform: uppercase;
  }
  .filterbar button:hover {
    color: var(--amber);
    border-color: var(--amber-dim);
  }
  .filterbar button.on {
    background: rgba(255, 165, 36, 0.08);
    border-color: var(--amber-dim);
    color: var(--amber);
  }
  .hidden-count {
    margin-left: auto;
    color: var(--text-faint);
    font-size: 8.5px;
    white-space: nowrap;
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
    display: grid;
    gap: 1px;
    padding: 5px 10px 4px;
    font-size: 9.5px;
    color: var(--text-faint);
    letter-spacing: 0.03em;
    border-bottom: 1px dotted var(--border);
    text-transform: uppercase;
  }
  .grp-h span {
    color: var(--amber);
    font-family: var(--sans);
    font-weight: 700;
    letter-spacing: 0.12em;
  }
  .grp-h small {
    color: var(--text-faint);
    font-size: 9px;
    font-weight: 400;
    letter-spacing: 0.02em;
    overflow: hidden;
    text-overflow: ellipsis;
    text-transform: none;
    white-space: nowrap;
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
  .dot.trial {
    background: var(--amber);
    box-shadow: 0 0 5px var(--amber-dim);
  }
  .dot.tuning {
    background: var(--violet);
    box-shadow: 0 0 5px rgba(178, 133, 255, 0.35);
  }
  .nname {
    color: var(--text-dim);
    font-size: 11px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .trial-pill {
    margin-left: 5px;
    padding: 0 4px;
    border: 1px solid var(--amber-dim);
    color: var(--amber);
    font-size: 7.5px;
    font-weight: 700;
    letter-spacing: 0.06em;
  }
  .origin-pill {
    margin-left: 5px;
    color: var(--text-faint);
    font-size: 7.5px;
    font-weight: 700;
    letter-spacing: 0.06em;
  }
  .origin-pill.meta {
    color: var(--cyan);
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
