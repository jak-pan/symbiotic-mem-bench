<script lang="ts">
  import { api } from "../lib/api";
  import type { TracesResponse } from "../lib/types";
  import { ms, money, tokens } from "../lib/format";
  import Panel from "../components/Panel.svelte";

  let { id }: { id: string } = $props();
  let data = $state<TracesResponse | null>(null);
  let loading = $state(true);

  $effect(() => {
    const runId = id;
    loading = true;
    data = null;
    api.traces(runId).then((d) => { data = d; loading = false; });
  });

  // bucket the memory trace operations
  const opCounts = $derived.by(() => {
    const m = new Map<string, number>();
    for (const t of data?.memory_traces.rows ?? []) {
      const op = t.operation ?? "?";
      m.set(op, (m.get(op) ?? 0) + 1);
    }
    return [...m.entries()].sort((a, b) => b[1] - a[1]);
  });

  function evClass(ev: string): string {
    if (ev?.includes("failed")) return "down";
    if (ev?.includes("succeeded")) return "up";
    return "faint";
  }
</script>

{#if loading}
  <div class="load">LOADING TRACES…</div>
{:else if data}
  <div class="tr fade-in">
    {#if !data.model_rollup && !data.memory_traces.total && !data.queue_timing && !data.workflow_queue}
      <div class="none">NO NATIVE TRACES — this run is artifact-only (no model/memory/queue traces captured).</div>
    {/if}

    <div class="tr-top">
      {#if data.model_rollup}
        {@const m = data.model_rollup}
        <Panel title="Provider Calls" tag="{m.calls} calls">
          <div class="rollup">
            <div class="rt"><span class="label">CALLS</span><b>{m.calls}</b></div>
            <div class="rt"><span class="label">FAILED</span><b class:down={m.failed_calls > 0}>{m.failed_calls}</b></div>
            <div class="rt"><span class="label">IN TOK</span><b>{tokens(m.input_tokens)}</b></div>
            <div class="rt"><span class="label">OUT TOK</span><b>{tokens(m.output_tokens)}</b></div>
            <div class="rt"><span class="label">COST</span><b>{money(m.cost_micro_usd)}</b></div>
            <div class="rt"><span class="label">p50</span><b>{ms(m.latency_ms_p50)}</b></div>
            <div class="rt"><span class="label">p95</span><b>{ms(m.latency_ms_p95)}</b></div>
          </div>
          <div class="roles">
            {#each Object.entries(m.roles) as [role, model] (role)}
              <div class="role"><span class="rk">{role}</span><span class="rm">{model}</span></div>
            {/each}
          </div>
        </Panel>
      {/if}

      <Panel title="Memory Operations" tag="{data.memory_traces.total} events{data.memory_traces.truncated ? ' (capped)' : ''}">
        <div class="ops">
          {#each opCounts as [op, n] (op)}
            <div class="opb"><span class="opn">{op}</span><span class="opc mono-num">{n}</span></div>
          {/each}
          {#if !opCounts.length}<div class="faint">no memory traces</div>{/if}
        </div>
      </Panel>
    </div>

    {#if data.queue_timing && data.queue_timing.length}
      <Panel title="Provider Queue Timing" tag="{data.queue_timing.length} items" flush scroll>
        <table class="grid">
          <thead><tr><th>Queue</th><th>Op</th><th class="num">Attempts</th><th class="num">Wait</th><th class="num">Run</th><th class="num">Total</th><th>Status</th></tr></thead>
          <tbody>
            {#each data.queue_timing.slice(0, 300) as q (q.queue_id + q.item_id)}
              <tr>
                <td class="mono-num dim">{q.queue_id}</td>
                <td>{q.operation}</td>
                <td class="num mono-num">{q.attempts}</td>
                <td class="num mono-num dim">{ms(q.wait_ms)}</td>
                <td class="num mono-num dim">{ms(q.run_ms)}</td>
                <td class="num mono-num">{ms(q.total_ms)}</td>
                <td><span class:up={q.final_status === "succeeded"} class:down={q.final_status === "failed" || q.final_status === "dead"}>{q.final_status ?? "—"}</span></td>
              </tr>
            {/each}
          </tbody>
        </table>
      </Panel>
    {/if}

    {#if data.workflow_queue?.databases?.length}
      <Panel title="Workflow Queue" tag="{data.workflow_queue.databases.length} db" flush scroll>
        {#each data.workflow_queue.databases as db (db.path)}
          <div class="qdb">
            <div class="qdb-head">
              <span class="mono-num">{db.path}</span>
              <span class="faint">{db.total_items} items / {db.total_events} events</span>
            </div>
            <div class="qchips">
              {#each Object.entries(db.items_by_status) as [status, count] (status)}
                <span class="qchip" class:up={status === "succeeded"} class:down={status === "failed" || status === "dead"}>{status}: {count}</span>
              {/each}
              <span class="qchip">retried: {db.retried_items}</span>
              <span class="qchip">max attempt: {db.max_attempt}</span>
            </div>
            {#if db.recent_errors.length}
              <div class="subhead">Recent errors</div>
              <table class="grid compact">
                <thead><tr><th>Queue</th><th>Kind</th><th>Status</th><th class="num">Attempt</th><th>Error</th></tr></thead>
                <tbody>
                  {#each db.recent_errors.slice(0, 20) as e (e.item_id)}
                    <tr>
                      <td class="mono-num dim">{e.queue_id}</td>
                      <td>{e.kind}</td>
                      <td class:down={e.status === "failed" || e.status === "dead"}>{e.status}</td>
                      <td class="num mono-num">{e.attempt}</td>
                      <td class="dim">{e.error}</td>
                    </tr>
                  {/each}
                </tbody>
              </table>
            {/if}
            <div class="subhead">Recent events</div>
            <table class="grid compact">
              <thead><tr><th style="width:120px">Time</th><th>Queue</th><th>Kind</th><th>Status</th><th class="num">Attempt</th></tr></thead>
              <tbody>
                {#each db.recent_events.slice(0, 30) as e, i (e.item_id + e.status + i)}
                  <tr>
                    <td class="mono-num faint">{(e.timestamp ?? "").slice(11, 23)}</td>
                    <td class="mono-num dim">{e.queue_id}</td>
                    <td>{e.kind}</td>
                    <td><span class:up={e.status === "succeeded"} class:down={e.status === "failed" || e.status === "dead"}>{e.status}</span></td>
                    <td class="num mono-num">{e.attempt}</td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>
        {/each}
      </Panel>
    {/if}

    {#if data.memory_traces.rows.length}
      <Panel title="Memory Trace Stream" tag="first {data.memory_traces.rows.length}" flush scroll>
        <table class="grid">
          <thead><tr><th style="width:120px">Time</th><th>Op</th><th>Stage</th><th>Event</th><th class="num">Dur</th><th>Source</th></tr></thead>
          <tbody>
            {#each data.memory_traces.rows.slice(0, 400) as t, i (i)}
              <tr>
                <td class="mono-num faint">{(t.timestamp ?? "").slice(11, 23)}</td>
                <td class="amber">{t.operation ?? "—"}</td>
                <td class="dim">{t.stage ?? "—"}</td>
                <td class={evClass(t.event)}>{(t.event ?? "").replace("operation_", "")}</td>
                <td class="num mono-num dim">{t.duration_ms != null ? ms(t.duration_ms) : "—"}</td>
                <td class="mono-num faint">{t.question_id ?? t.source_id ?? "—"}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </Panel>
    {/if}
  </div>
{/if}

<style>
  .load {
    padding: 40px;
    color: var(--text-faint);
    letter-spacing: 0.2em;
  }
  .tr {
    padding: 10px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .none {
    padding: 10px;
    background: var(--bg-panel);
    border: 1px solid var(--border-bright);
    color: var(--text-dim);
    font-size: 11px;
  }
  .tr-top {
    display: grid;
    grid-template-columns: 1.4fr 1fr;
    gap: 10px;
  }
  .rollup {
    display: grid;
    grid-template-columns: repeat(7, 1fr);
    gap: 1px;
    background: var(--border);
    border: 1px solid var(--border);
  }
  .rt {
    background: var(--bg-panel);
    padding: 7px 6px;
    display: flex;
    flex-direction: column;
    gap: 3px;
    align-items: flex-start;
  }
  .rt b {
    font-size: 14px;
    font-weight: 600;
  }
  .roles {
    margin-top: 8px;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .role {
    display: flex;
    justify-content: space-between;
    font-size: 10.5px;
    padding: 2px 0;
    border-bottom: 1px dotted var(--border);
  }
  .rk {
    color: var(--text-faint);
  }
  .rm {
    color: var(--cyan);
  }
  .ops {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .opb {
    display: flex;
    justify-content: space-between;
    padding: 3px 6px;
    background: var(--bg-elev);
    border-left: 2px solid var(--amber-dim);
  }
  .opn {
    color: var(--text-dim);
  }
  .opc {
    color: var(--amber);
  }
  .qdb {
    padding: 8px;
    border-bottom: 1px solid var(--border);
  }
  .qdb:last-child {
    border-bottom: 0;
  }
  .qdb-head {
    display: flex;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 6px;
    font-size: 11px;
  }
  .qchips {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
    margin-bottom: 8px;
  }
  .qchip {
    border: 1px solid var(--border);
    background: var(--bg-elev);
    padding: 2px 6px;
    font-size: 10px;
    color: var(--text-dim);
  }
  .subhead {
    margin: 8px 0 4px;
    color: var(--text-faint);
    text-transform: uppercase;
    letter-spacing: 0.12em;
    font-size: 10px;
  }
  .compact th,
  .compact td {
    padding: 4px 6px;
  }
  .dim {
    color: var(--text-dim);
  }
  .faint {
    color: var(--text-faint);
  }
</style>
