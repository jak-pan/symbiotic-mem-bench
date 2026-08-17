<script lang="ts">
  import { onMount } from "svelte";
  import { store } from "./lib/store.svelte";
  import { router } from "./lib/router.svelte";
  import Debugger from "./routes/Debugger.svelte";
  import CockpitLeaderboard from "./routes/CockpitLeaderboard.svelte";
  import CockpitRuns from "./routes/CockpitRuns.svelte";
  import CockpitLab from "./routes/CockpitLab.svelte";
  import CockpitCatalog from "./routes/CockpitCatalog.svelte";

  type Workspace = "leaderboard" | "runs" | "lab" | "catalog";

  const workspaces: Array<{ id: Workspace; fk: string; label: string; hint: string }> = [
    { id: "leaderboard", fk: "F1", label: "Leaderboard", hint: "Reviewed, cohort-locked results" },
    { id: "runs", fk: "F2", label: "Runs", hint: "Inspect ranked and held-back records" },
    { id: "lab", fk: "F3", label: "Lab", hint: "Configure and launch evidence-producing runs" },
    { id: "catalog", fk: "F4", label: "Catalog", hint: "Systems, benchmarks and artifact coverage" },
  ];

  let workspace = $state<Workspace>(router.view === "debug" ? "runs" : "leaderboard");
  let clock = $state("");
  let clockDate = $state("");

  const heldBack = $derived(
    store.isSnapshot
      ? (store.snapshot?.unranked.length ?? 0)
      : store.runs.filter((run) => !run.eligibility?.eligible).length,
  );
  const generated = $derived(!store.loaded ? "—" : store.snapshot?.generated_at.slice(0, 10) ?? "live");

  function go(next: Workspace) {
    workspace = next;
    if (next === "leaderboard") router.go("leaderboard");
    else if (next === "runs" && store.online) router.go("debug");
  }

  // Keep browser history and hand-edited hashes authoritative. Workspace-only
  // views (Lab/Catalog) intentionally remain local until they gain routes of
  // their own, while the two routed views must always follow hash navigation.
  $effect(() => {
    if (router.view === "leaderboard") workspace = "leaderboard";
    else if (router.view === "debug") workspace = "runs";
  });

  function tick() {
    const d = new Date();
    const p = (n: number) => String(n).padStart(2, "0");
    clock = `${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}`;
    clockDate = `${p(d.getDate())} ${d.toLocaleString("en", { month: "short" }).toUpperCase()} ${d.getFullYear()}`;
  }

  onMount(() => {
    store.boot();
    tick();
    const clockTimer = setInterval(tick, 1000);
    const refreshTimer = setInterval(() => {
      if (document.visibilityState === "visible") store.load();
    }, 15000);
    return () => {
      clearInterval(clockTimer);
      clearInterval(refreshTimer);
    };
  });

  function onKeydown(event: KeyboardEvent) {
    const target = event.target as HTMLElement | null;
    const typing = !!target &&
      (target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.tagName === "SELECT" || target.isContentEditable);
    if (typing || event.metaKey || event.ctrlKey || event.altKey) return;
    if (/^F[1-4]$/.test(event.key)) {
      event.preventDefault();
      go(workspaces[Number(event.key.slice(1)) - 1].id);
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

<div class="cockpit">
  <header class="topbar">
    <div class="brand">
      <span class="logo">▮▮ MEMBENCH</span>
      <span class="tag">v2 · memory-system cockpit</span>
    </div>
    <nav class="modes" aria-label="Product workspaces">
      {#each workspaces as item}
        <button
          class="mode"
          class:active={workspace === item.id}
          aria-current={workspace === item.id ? "page" : undefined}
          title={`${item.hint} · ${item.fk}`}
          onclick={() => go(item.id)}
        ><span class="fk">{item.fk}</span>{item.label}</button>
      {/each}
    </nav>
    <div class="spacer"></div>
    <span class="context" title="Every visible ranking comes from the published review gate">
      <span>DATA</span><b>{store.mode === "boot" ? "loading" : store.isSnapshot ? "reviewed snapshot" : store.online ? "live registry" : "unavailable"}</b>
    </span>
    <span class="chip" class:green={store.mode === "snapshot" || store.online} class:red={store.mode === "offline"}>
      {store.mode === "snapshot" ? "verified" : store.online ? "live" : store.mode}
    </span>
    <span class="clock mono-num" title={clockDate}>{clock}</span>
  </header>

  <div class="truth-tape" role="status">
    {#if store.mode === "boot"}
      <span class="pulse-dot"></span><b>LOADING</b> validating the published leaderboard snapshot before showing claims
    {:else if store.mode === "offline"}
      <span class="error-dot"></span><b>DATA UNAVAILABLE</b> {store.error ?? "leaderboard could not be loaded"}
    {:else}
      <span class="ok-dot"></span><b>TRUTH GATE</b> {store.verifiedCount} ranked · {heldBack} held back · projections never rank · source artifacts remain the authority
    {/if}
  </div>

  <main class="body">
    {#if workspace === "leaderboard"}
      <CockpitLeaderboard />
    {:else if workspace === "runs"}
      {#if store.online}<Debugger />{:else}<CockpitRuns />{/if}
    {:else if workspace === "lab"}
      <CockpitLab />
    {:else}
      <CockpitCatalog />
    {/if}
  </main>

  <footer class="statusbar">
    <span class="seg"><span class="dot" class:live={store.mode === "snapshot" || store.online}></span> {store.isSnapshot ? "STATIC SNAPSHOT" : store.online ? "LIVE" : store.mode.toUpperCase()}</span>
    <span class="seg">RECORDS <b>{store.loaded ? store.recordCount : "—"}</b></span>
    <span class="seg">VERIFIED <b>{store.loaded ? store.verifiedCount : "—"}</b></span>
    <span class="seg">SYSTEMS <b>{store.loaded ? store.systems.length : "—"}</b></span>
    <span class="seg">BENCHMARKS <b>{store.loaded ? store.benchmarks.length : "—"}</b></span>
    <span class="seg">PEAK ACC <b class="amber">{store.bestAccuracy == null ? "—" : `${(store.bestAccuracy * 100).toFixed(1)}%`}</b></span>
    <span class="spacer"></span>
    <span class="seg hints">F1–F4 workspaces · all public claims are artifact-backed</span>
    <span class="seg">SERVER <b>{store.serverSha || "—"}</b></span>
    <span class="seg">UI <b>{store.uiBundle || "—"}</b></span>
    <span class="seg" title={store.uiBuilt}>BUILT <b>{store.uiBuilt || "—"}</b></span>
    <span class="seg">DATA <b>{generated}</b></span>
  </footer>
</div>

<style>
  .cockpit { height: 100%; min-width: 980px; display: flex; flex-direction: column; overflow: hidden; }
  .topbar { height: 40px; flex: none; display: flex; align-items: center; overflow: hidden; border-bottom: 1px solid var(--border-bright); background: var(--bg-panel); padding: 0 12px; }
  .brand { display: flex; align-items: baseline; gap: 8px; padding-right: 16px; flex-shrink: 1; min-width: 0; overflow: hidden; white-space: nowrap; }
  .logo { flex-shrink: 0; font-family: var(--sans); font-size: 14px; font-weight: 800; letter-spacing: .16em; color: var(--amber); }
  .tag { min-width: 0; overflow: hidden; text-overflow: ellipsis; color: var(--text-faint); font-size: 9px; letter-spacing: .16em; text-transform: uppercase; }
  .modes { height: 100%; display: flex; align-items: stretch; }
  .mode { height: 100%; display: flex; align-items: center; gap: 7px; padding: 0 14px; cursor: pointer; background: none; border: none; border-bottom: 2px solid transparent; color: var(--text-dim); font-family: var(--sans); font-size: 11px; font-weight: 700; letter-spacing: .12em; text-transform: uppercase; }
  .mode:hover { color: var(--text); }
  .mode.active { color: var(--amber); border-bottom-color: var(--amber); background: rgba(255,165,36,.05); }
  .fk { padding: 0 3px; border: 1px solid var(--border-bright); color: var(--text-faint); font-size: 9px; }
  .mode.active .fk { color: var(--amber); border-color: var(--amber-dim); }
  .spacer { flex: 1; }
  .context { flex-shrink: 0; display: flex; align-items: center; gap: 6px; margin-right: 8px; padding: 3px 8px; border: 1px solid var(--border); white-space: nowrap; }
  .context span { color: var(--text-faint); font-family: var(--sans); font-size: 8.5px; font-weight: 700; letter-spacing: .1em; }
  .context b { color: var(--text-dim); font-size: 10px; font-weight: 500; }
  .clock { flex-shrink: 0; padding-left: 10px; color: var(--text-dim); font-size: 11px; letter-spacing: .06em; }
  .truth-tape { height: 24px; flex: none; display: flex; align-items: center; gap: 7px; padding: 0 12px; overflow: hidden; border-bottom: 1px solid var(--border); background: var(--bg-panel); color: var(--text-dim); font-size: 10px; white-space: nowrap; }
  .truth-tape b { color: var(--text); font-family: var(--sans); font-size: 8.5px; letter-spacing: .12em; }
  .ok-dot, .error-dot, .pulse-dot { width: 7px; height: 7px; flex: none; border-radius: 50%; }
  .ok-dot { background: var(--green); box-shadow: 0 0 6px rgba(47,207,122,.5); }
  .error-dot { background: var(--red); }
  .pulse-dot { background: var(--amber); animation: blink 1s steps(1) infinite; }
  @keyframes blink { 50% { opacity: .25; } }
  .body { flex: 1; min-height: 0; display: flex; overflow: hidden; }
  .statusbar { height: 26px; flex: none; display: flex; align-items: center; gap: 16px; padding: 0 12px; overflow: hidden; border-top: 1px solid var(--border-bright); background: var(--bg-panel); color: var(--text-dim); font-size: 10.5px; }
  .seg { display: flex; align-items: center; gap: 6px; flex-shrink: 0; white-space: nowrap; }
  .seg b { color: var(--text); font-weight: 600; }
  .seg b.amber { color: var(--amber); }
  .dot { width: 7px; height: 7px; border-radius: 50%; background: var(--text-faint); }
  .dot.live { background: var(--green); }
  .hints { min-width: 0; overflow: hidden; text-overflow: ellipsis; }
  .chip { flex-shrink: 0; }
</style>
