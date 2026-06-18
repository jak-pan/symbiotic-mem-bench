<script lang="ts">
  import { onMount } from "svelte";
  import { router } from "./lib/router.svelte";
  import { store } from "./lib/store.svelte";
  import { pct } from "./lib/format";
  import Leaderboard from "./routes/Leaderboard.svelte";
  import Debugger from "./routes/Debugger.svelte";

  let clock = $state("");
  let cmd = $state("");
  let cmdEl: HTMLInputElement;

  function tick() {
    const d = new Date();
    clock =
      d.toLocaleTimeString("en-GB", { hour12: false }) +
      " " +
      d.toLocaleDateString("en-GB", { day: "2-digit", month: "short" }).toUpperCase();
  }

  onMount(() => {
    store.load();
    tick();
    const t = setInterval(tick, 1000);
    const r = setInterval(() => store.load(), 15000);
    return () => {
      clearInterval(t);
      clearInterval(r);
    };
  });

  function onKey(e: KeyboardEvent) {
    if (e.key === "/" && document.activeElement !== cmdEl) {
      e.preventDefault();
      cmdEl?.focus();
    } else if (e.key === "F1") {
      e.preventDefault();
      router.go("leaderboard");
    } else if (e.key === "F2") {
      e.preventDefault();
      router.go("debug");
    } else if (e.key === "Escape") {
      cmdEl?.blur();
    }
  }

  function runCommand() {
    const q = cmd.trim().toLowerCase();
    if (!q) return;
    if (q === "lb" || q === "leaderboard") return router.go("leaderboard");
    if (q === "dbg" || q === "debug") return router.go("debug");
    const hit = store.runs.find(
      (r) =>
        r.run_name.toLowerCase().includes(q) || r.run_id.toLowerCase().includes(q),
    );
    if (hit) {
      router.openRun(hit.run_id);
      cmd = "";
    }
  }
</script>

<svelte:window on:keydown={onKey} />

<header class="topbar">
  <div class="brand">
    <span class="mark">▮▮</span>
    <span class="word">MEMBENCH</span>
    <span class="sub">MEMORY&nbsp;SYSTEM&nbsp;TERMINAL</span>
  </div>

  <nav class="nav">
    <button class="navbtn" class:active={router.view === "leaderboard"} onclick={() => router.go("leaderboard")}>
      <span class="fk">F1</span> LEADERBOARD
    </button>
    <button class="navbtn" class:active={router.view === "debug"} onclick={() => router.go("debug")}>
      <span class="fk">F2</span> DEBUGGER
    </button>
  </nav>

  <div class="cmd">
    <span class="prompt">&gt;</span>
    <input
      bind:this={cmdEl}
      bind:value={cmd}
      name="command"
      onkeydown={(e) => e.key === "Enter" && runCommand()}
      placeholder="run / cmd  ( press / )"
      spellcheck="false"
      autocomplete="off"
    />
    <span class="cursor blink">_</span>
  </div>

  <div class="clock mono-num">{clock}</div>
</header>

<main class="stage">
  {#if router.view === "debug"}
    <Debugger />
  {:else}
    <Leaderboard />
  {/if}
</main>

<footer class="statusbar">
  <span class="st" class:on={store.online} class:off={!store.online}>
    <span class="dot"></span>{store.online ? "LIVE" : "OFFLINE"}
  </span>
  <span class="sep">│</span>
  <span class="stk">RUNS <b>{store.runs.length}</b></span>
  <span class="sep">│</span>
  <span class="stk">SYSTEMS <b>{store.systems.length}</b></span>
  <span class="sep">│</span>
  <span class="stk">BENCHMARKS <b>{store.benchmarks.length}</b></span>
  <span class="sep">│</span>
  <span class="stk">PEAK&nbsp;ACC <b class="amber">{pct(store.bestAccuracy)}%</b></span>
  {#if store.active.length}
    <span class="sep">│</span>
    <button class="inflight" onclick={() => router.openRun(store.active[0].run_id)}>
      <span class="ifdot"></span>{store.active.length} IN&nbsp;FLIGHT
    </button>
  {/if}
  {#if store.error}<span class="sep">│</span><span class="err">ERR {store.error}</span>{/if}
  <span class="spacer"></span>
  <span class="hint"><kbd>/</kbd> cmd</span>
  <span class="hint"><kbd>F1</kbd>/<kbd>F2</kbd> view</span>
  <span class="hint"><kbd>j</kbd><kbd>k</kbd> rows</span>
  <span class="sep">│</span>
  <span class="stk">MEMBENCH v0.1</span>
</footer>

<style>
  .topbar {
    height: var(--h-topbar);
    flex: none;
    display: flex;
    align-items: stretch;
    background: var(--bg-panel);
    border-bottom: 1px solid var(--border-bright);
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 0 16px;
    border-right: 1px solid var(--border);
  }
  .mark {
    color: var(--amber);
    letter-spacing: -2px;
    font-size: 14px;
    text-shadow: 0 0 8px rgba(255, 165, 36, 0.6);
  }
  .word {
    font-family: var(--sans);
    font-weight: 800;
    letter-spacing: 0.16em;
    font-size: 14px;
    color: var(--text);
  }
  .sub {
    font-family: var(--sans);
    font-size: 8px;
    font-weight: 700;
    letter-spacing: 0.18em;
    color: var(--text-faint);
    align-self: center;
    padding-top: 2px;
  }
  .nav {
    display: flex;
  }
  .navbtn {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 0 16px;
    background: transparent;
    border: none;
    border-right: 1px solid var(--border);
    color: var(--text-dim);
    font-family: var(--sans);
    font-weight: 700;
    font-size: 11px;
    letter-spacing: 0.1em;
    cursor: pointer;
    transition: all 0.12s;
  }
  .navbtn:hover {
    color: var(--text);
    background: var(--bg-elev);
  }
  .navbtn.active {
    color: var(--amber);
    background: rgba(255, 165, 36, 0.07);
    box-shadow: inset 0 -2px 0 var(--amber);
  }
  .fk {
    font-size: 8.5px;
    color: var(--text-faint);
    border: 1px solid var(--border-bright);
    padding: 1px 3px;
  }
  .navbtn.active .fk {
    color: var(--amber);
    border-color: var(--amber-dim);
  }
  .cmd {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0 14px;
    border-right: 1px solid var(--border);
  }
  .prompt {
    color: var(--amber);
    font-weight: 700;
  }
  .cmd input {
    flex: 1;
    background: transparent;
    border: none;
    outline: none;
    color: var(--text);
    font-size: 12.5px;
    letter-spacing: 0.02em;
  }
  .cmd input::placeholder {
    color: var(--text-faint);
  }
  .cursor {
    color: var(--amber);
  }
  .clock {
    display: flex;
    align-items: center;
    padding: 0 16px;
    color: var(--text-dim);
    font-size: 11.5px;
    letter-spacing: 0.04em;
  }

  .stage {
    flex: 1;
    min-height: 0;
    display: flex;
    overflow: hidden;
  }

  .statusbar {
    height: var(--h-statusbar);
    flex: none;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 12px;
    background: var(--bg-panel);
    border-top: 1px solid var(--border-bright);
    font-size: 10.5px;
    color: var(--text-dim);
  }
  .st {
    display: flex;
    align-items: center;
    gap: 5px;
    font-weight: 700;
    letter-spacing: 0.08em;
  }
  .st .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
  }
  .st.on {
    color: var(--green);
  }
  .st.on .dot {
    background: var(--green);
    box-shadow: 0 0 6px var(--green);
    animation: pulse 2s ease infinite;
  }
  .st.off {
    color: var(--red);
  }
  .st.off .dot {
    background: var(--red);
  }
  @keyframes pulse {
    50% {
      opacity: 0.4;
    }
  }
  .stk b {
    color: var(--text);
    font-weight: 700;
  }
  .sep {
    color: var(--border-bright);
  }
  .err {
    color: var(--red);
  }
  .inflight {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    background: rgba(47, 207, 122, 0.08);
    border: 1px solid var(--green-dim);
    color: var(--green);
    font-family: var(--mono);
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.06em;
    padding: 1px 7px;
    cursor: pointer;
  }
  .inflight:hover {
    background: rgba(47, 207, 122, 0.16);
  }
  .ifdot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--green);
    box-shadow: 0 0 6px var(--green);
    animation: pulse 1.4s ease infinite;
  }
  @keyframes pulse {
    50% {
      opacity: 0.35;
    }
  }
  .spacer {
    flex: 1;
  }
  .hint {
    color: var(--text-faint);
    letter-spacing: 0.04em;
  }
  kbd {
    font-family: var(--mono);
    border: 1px solid var(--border-bright);
    padding: 0 4px;
    color: var(--text-dim);
    font-size: 9.5px;
  }
</style>
