<script lang="ts">
  import { store } from "../lib/store.svelte";
  import type { GateFailure, RunSummary, UnrankedRecord } from "../lib/types";

  type Entry = {
    id: string;
    name: string;
    system: string;
    benchmark: string;
    limit: number | null;
    accuracy: number | null;
    state: "verified" | "held";
    reason: string;
    failures: GateFailure[];
    artifacts: string[];
    cohort: string | null;
  };

  function rankedEntry(run: RunSummary): Entry {
    return {
      id: run.run_id, name: run.display_name || run.run_name, system: run.system,
      benchmark: run.benchmark, limit: run.limit, accuracy: run.accuracy, state: "verified",
      reason: "passed every publication gate", failures: [], artifacts: run.artifacts_available,
      cohort: run.cohort_id,
    };
  }

  function unrankedEntry(run: UnrankedRecord): Entry {
    return {
      id: run.run_id, name: run.run_name, system: run.system, benchmark: run.benchmark,
      limit: run.limit ?? null, accuracy: run.accuracy ?? null, state: "held", reason: run.reason,
      failures: run.failed_gates, artifacts: [], cohort: null,
    };
  }

  const entries = $derived.by((): Entry[] => {
    if (store.snapshot) return [
      ...store.snapshot.cohorts.flatMap((cohort) => cohort.rows.map(rankedEntry)),
      ...store.snapshot.unranked.map(unrankedEntry),
    ];
    return store.runs.map((run) => run.eligibility?.eligible ? rankedEntry(run) : ({
      ...rankedEntry(run), state: "held" as const, reason: "gate-failed",
      failures: run.eligibility?.failures ?? [],
    }));
  });

  let selectedId = $state("");
  let filter = $state<"all" | "verified" | "held">("all");
  const visible = $derived(entries.filter((entry) => filter === "all" || entry.state === filter));
  const selected = $derived(entries.find((entry) => entry.id === selectedId) ?? visible[0]);
  const verified = $derived(entries.filter((entry) => entry.state === "verified").length);
  const held = $derived(entries.length - verified);

  function pct(value: number | null) { return value == null ? "—" : `${(value * 100).toFixed(1)}%`; }
</script>

<aside class="rail">
  <div class="rail-head"><span>RUN REGISTRY</span><b>{entries.length}</b></div>
  <div class="filters">
    <button class:active={filter === "all"} onclick={() => filter = "all"}>ALL {entries.length}</button>
    <button class:active={filter === "verified"} onclick={() => filter = "verified"}>RANKED {verified}</button>
    <button class:active={filter === "held"} onclick={() => filter = "held"}>HELD {held}</button>
  </div>
  <div class="run-list">
    {#each visible as entry}
      <button class="run" class:active={selected?.id === entry.id} onclick={() => selectedId = entry.id}>
        <span class:green={entry.state === "verified"} class:amber={entry.state === "held"}>{entry.state === "verified" ? "✓" : "◆"}</span>
        <div><b>{entry.name}</b><small>{entry.system} · {entry.benchmark} · {entry.limit ?? "all"}q</small></div>
        <em>{pct(entry.accuracy)}</em>
      </button>
    {:else}<p class="empty">No records in this filter.</p>{/each}
  </div>
</aside>

<section class="work">
  <div class="subtabs"><span class="active">OVERVIEW</span><span>QUESTIONS</span><span>EVIDENCE</span><span>TRACES</span><span>MEMORY</span><span>TELEMETRY</span><span>AUDIT</span></div>
  {#if selected}
    <div class="scroll">
      <div class="run-hero">
        <div><span class="eyebrow">RUN INSPECTION</span><h1>{selected.name}</h1><p>{selected.id}</p></div>
        <span class="state" class:verified={selected.state === "verified"}>{selected.state === "verified" ? "VERIFIED · RANKABLE" : "HELD BACK"}</span>
      </div>
      <div class="tiles">
        <div><span>ACCURACY</span><b class="amber">{pct(selected.accuracy)}</b></div>
        <div><span>SYSTEM</span><b>{selected.system}</b></div>
        <div><span>BENCHMARK</span><b>{selected.benchmark}</b></div>
        <div><span>SCALE</span><b>{selected.limit ?? "—"} questions</b></div>
      </div>
      <div class="grid">
        <section class="panel">
          <header><span>EVIDENCE COVERAGE</span><b>{selected.artifacts.length} public artifact classes</b></header>
          {#if selected.artifacts.length}
            <div class="artifacts">
              {#each selected.artifacts as artifact}<span>✓ {artifact.replaceAll("_", " ")}</span>{/each}
            </div>
          {:else}
            <div class="notice amber"><b>NO RANKING EVIDENCE EXPOSED</b><p>This record is visible for audit, but its claim remains withheld.</p></div>
          {/if}
        </section>
        <section class="panel">
          <header><span>PUBLICATION DECISION</span><b>{selected.state}</b></header>
          <div class="decision">
            <b class:green={selected.state === "verified"} class:amber={selected.state === "held"}>{selected.reason}</b>
            {#if selected.cohort}<p>Locked cohort: <span title={selected.cohort}>{selected.cohort.slice(0, 72)}…</span></p>{/if}
            {#each selected.failures as failure}
              <div><strong>{failure.gate}</strong><span>{failure.detail}</span></div>
            {/each}
          </div>
        </section>
      </div>
      <section class="panel inspector">
        <header><span>FULL INSPECTION CONTRACT</span><b>fail-closed static canary</b></header>
        <div class="contract">
          <p>The public canary carries only the portable leaderboard export. Question bodies, traces, memory state and provider telemetry require the live <code>membench-server</code> artifact endpoints.</p>
          <p>No synthetic question, trace, memory operation or telemetry sample is substituted when those endpoints are absent.</p>
        </div>
      </section>
    </div>
  {/if}
</section>

<style>
  .rail { width: 300px; flex: none; display: flex; flex-direction: column; overflow: hidden; border-right: 1px solid var(--border); background: var(--bg-panel); }
  .rail-head { display: flex; justify-content: space-between; padding: 9px 12px; border-bottom: 1px solid var(--border); color: var(--text-faint); font-family: var(--sans); font-size: 9.5px; font-weight: 700; letter-spacing: .13em; }
  .rail-head b { color: var(--amber); }
  .filters { display: grid; grid-template-columns: repeat(3, 1fr); border-bottom: 1px solid var(--border); }
  .filters button { padding: 6px 3px; cursor: pointer; background: none; border: 0; border-right: 1px solid var(--border); color: var(--text-faint); font-size: 8px; font-weight: 700; }
  .filters button.active { color: var(--amber); background: rgba(255,165,36,.07); }
  .run-list { flex: 1; overflow-y: auto; }
  .run { width: 100%; display: grid; grid-template-columns: 14px 1fr auto; gap: 7px; align-items: start; padding: 8px 10px; cursor: pointer; text-align: left; background: none; border: 0; border-bottom: 1px solid var(--border); color: var(--text-dim); }
  .run:hover, .run.active { background: var(--bg-hover); }
  .run.active { box-shadow: inset 2px 0 0 var(--amber); }
  .run div { min-width: 0; }
  .run b, .run small { display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .run b { color: var(--text); font-size: 10px; }
  .run small { color: var(--text-faint); font-size: 8.5px; }
  .run em { color: var(--text-dim); font-size: 10px; font-style: normal; }
  .green { color: var(--green) !important; }.amber { color: var(--amber) !important; }
  .empty { padding: 14px; color: var(--text-faint); font-size: 10px; }
  .work { min-width: 0; flex: 1; display: flex; flex-direction: column; overflow: hidden; }
  .subtabs { height: 34px; flex: none; display: flex; align-items: stretch; gap: 3px; padding: 0 7px; border-bottom: 1px solid var(--border); background: var(--bg-panel); }
  .subtabs span { display: flex; align-items: center; padding: 0 10px; border-bottom: 2px solid transparent; color: var(--text-faint); font-family: var(--sans); font-size: 9px; font-weight: 700; letter-spacing: .1em; }
  .subtabs .active { color: var(--amber); border-bottom-color: var(--amber); }
  .scroll { flex: 1; overflow-y: auto; padding: 12px; }
  .run-hero { display: flex; justify-content: space-between; align-items: center; padding: 12px 14px; border: 1px solid var(--border-bright); background: linear-gradient(100deg, rgba(70,169,255,.06), transparent 45%), var(--bg-panel); }
  .eyebrow { color: var(--cyan); font-family: var(--sans); font-size: 8px; font-weight: 800; letter-spacing: .17em; }
  h1 { margin-top: 2px; font-size: 20px; } .run-hero p { color: var(--text-faint); font-size: 9px; }
  .state { padding: 4px 8px; border: 1px solid var(--amber-dim); color: var(--amber); font-size: 9px; font-weight: 700; letter-spacing: .1em; }
  .state.verified { border-color: var(--green-dim); color: var(--green); }
  .tiles { display: grid; grid-template-columns: repeat(4, 1fr); gap: 1px; margin-top: 10px; background: var(--border); }
  .tiles div { padding: 10px 12px; background: var(--bg-panel); }
  .tiles span { display: block; color: var(--text-faint); font-family: var(--sans); font-size: 8px; font-weight: 700; letter-spacing: .1em; }
  .tiles b { display: block; margin-top: 3px; overflow: hidden; font-size: 16px; text-overflow: ellipsis; white-space: nowrap; }
  .grid { display: grid; grid-template-columns: 1fr 1fr; gap: 10px; margin-top: 10px; }
  .panel { border: 1px solid var(--border); background: var(--bg-panel); }
  .panel header { display: flex; justify-content: space-between; padding: 8px 10px; border-bottom: 1px solid var(--border); }
  .panel header span { font-family: var(--sans); font-size: 9px; font-weight: 700; letter-spacing: .11em; }
  .panel header b { color: var(--amber); font-size: 8px; font-weight: 500; text-transform: uppercase; }
  .artifacts { display: grid; grid-template-columns: repeat(3, 1fr); gap: 1px; padding: 8px; background: var(--border); }
  .artifacts span { padding: 8px; background: var(--bg-elev); color: var(--green); font-size: 9px; text-transform: uppercase; }
  .notice, .decision, .contract { padding: 12px; }
  .notice p, .contract p, .decision p { margin-top: 6px; color: var(--text-dim); font-size: 10px; }
  .decision > b { font-family: var(--sans); font-size: 11px; text-transform: uppercase; }
  .decision div { display: grid; grid-template-columns: 120px 1fr; gap: 8px; padding: 6px 0; border-top: 1px solid var(--border); }
  .decision div strong { color: var(--amber); font-size: 9px; text-transform: uppercase; }
  .decision div span { color: var(--text-dim); font-size: 9px; }
  .inspector { margin-top: 10px; }
  code { color: var(--cyan); }
</style>
