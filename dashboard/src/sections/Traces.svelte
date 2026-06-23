<script lang="ts">
  import { api } from "../lib/api";
  import type { QueueTiming, TracesResponse } from "../lib/types";
  import { ms, tokens } from "../lib/format";
  import Panel from "../components/Panel.svelte";

  let { id }: { id: string } = $props();
  let data = $state<TracesResponse | null>(null);
  let loading = $state(true);
  let traceKind = $state("all");
  let traceFilter = $state("");

  $effect(() => {
    const runId = id;
    loading = true;
    data = null;
    api.traces(runId).then((d) => { data = d; loading = false; });
  });

  const queueSummaries = $derived.by(() => {
    const groups = new Map<string, QueueTiming[]>();
    for (const row of data?.queue_timing ?? []) {
      const key = `${row.operation}:${shortQueue(row.queue_id)}`;
      const rows = groups.get(key) ?? [];
      rows.push(row);
      groups.set(key, rows);
    }
    return [...groups.entries()]
      .map(([name, rows]) => {
        const wait = rows.map((row) => row.wait_ms).filter(isFiniteNumber);
        const run = rows.map((row) => row.run_ms).filter(isFiniteNumber);
        const total = rows.map((row) => row.total_ms).filter(isFiniteNumber);
        const failed = rows.filter((row) => row.final_status === "failed" || row.final_status === "dead").length;
        return {
          name,
          count: rows.length,
          failed,
          wait_p50: percentile(wait, 50),
          wait_p80: percentile(wait, 80),
          wait_p95: percentile(wait, 95),
          wait_p98: percentile(wait, 98),
          run_p50: percentile(run, 50),
          run_p80: percentile(run, 80),
          run_p95: percentile(run, 95),
          run_p98: percentile(run, 98),
          total_p50: percentile(total, 50),
          total_p80: percentile(total, 80),
          total_p95: percentile(total, 95),
          total_p98: percentile(total, 98),
        };
      })
      .sort((a, b) => num(b.total_p98) - num(a.total_p98));
  });

  const bottlenecks = $derived.by(() => {
    const memory = (data?.memory_stage_timing ?? [])
      .filter((row) => row.work_ms_p98 != null)
      .map((row) => ({
        kind: "memory",
        name: row.operation,
        primary: num(row.work_ms_p98),
        left: 0,
        right: num(row.work_ms_p98),
        label: `${ms(row.work_ms_p98)} p98 cadence`,
        meta: `${row.batch_events || 0} windows · ${tokens(row.item_count)} ${row.item_unit}`,
      }));
    const queues = queueSummaries.map((row) => ({
      kind: "provider",
      name: row.name,
      primary: num(row.total_p98),
      left: num(row.wait_p98),
      right: num(row.run_p98),
      label: `${ms(row.total_p98)} p98 total`,
      meta: `${ms(row.wait_p98)} wait · ${ms(row.run_p98)} run · ${row.count} items`,
    }));
    return [...memory, ...queues].sort((a, b) => b.primary - a.primary).slice(0, 10);
  });

  const bottleneckMax = $derived.by(() => Math.max(1, ...bottlenecks.map((row) => row.primary)));
  const waterfall = $derived(data?.trace_waterfall);
  const dependencyWaterfall = $derived(data?.dependency_waterfall);
  const filteredTraceEvents = $derived.by(() => {
    const rows = data?.trace_events?.rows ?? [];
    const needle = traceFilter.trim().toLowerCase();
    return rows.filter((row) => {
      if (traceKind === "error" && row.status !== "failed" && !row.error) return false;
      if (traceKind !== "all" && traceKind !== "error" && row.kind !== traceKind) return false;
      if (!needle) return true;
      return [
        row.timestamp,
        row.kind,
        row.operation,
        row.lane,
        row.event,
        row.status,
        row.source,
        row.error ?? "",
      ].join(" ").toLowerCase().includes(needle);
    });
  });
  const waterfallTicks = $derived.by(() => {
    const duration = Math.max(1, waterfall?.duration_ms ?? 1);
    return [0, 0.25, 0.5, 0.75, 1].map((ratio) => ({
      ratio,
      label: ms(duration * ratio),
    }));
  });
  const dependencyTicks = $derived.by(() => {
    const duration = Math.max(1, dependencyWaterfall?.duration_ms ?? 1);
    return [0, 0.25, 0.5, 0.75, 1].map((ratio) => ({
      ratio,
      label: ms(duration * ratio),
    }));
  });

  function shortQueue(id: string): string {
    return id.replace(/^chat:/, "").replace(/^embedding:/, "");
  }

  function isFiniteNumber(value: number | null | undefined): value is number {
    return Number.isFinite(value);
  }

  function num(value: number | null | undefined): number {
    return Number.isFinite(value) ? Number(value) : 0;
  }

  function percentile(values: number[], p: number): number | null {
    if (!values.length) return null;
    const sorted = [...values].sort((a, b) => a - b);
    const rank = Math.round((p / 100) * (sorted.length - 1));
    return sorted[Math.max(0, Math.min(sorted.length - 1, rank))];
  }

  function pct(value: number, max: number): number {
    return Math.max(0, Math.min(100, (value / Math.max(1, max)) * 100));
  }

  function blockLeft(startMs: number): number {
    return pct(startMs, waterfall?.duration_ms ?? 1);
  }

  function blockWidth(block: { start_ms: number; end_ms: number; duration_ms: number }): number {
    const width = pct(Math.max(block.duration_ms, block.end_ms - block.start_ms, 1), waterfall?.duration_ms ?? 1);
    return Math.max(0.18, width);
  }

  function blockTitle(block: { label: string; duration_ms: number; source: string; item_count: number; item_unit: string; status: string }): string {
    const items = block.item_count ? ` · ${tokens(block.item_count)} ${block.item_unit}` : "";
    return `${block.label} · ${ms(block.duration_ms)} · ${block.status}${items} · ${block.source}`;
  }

  function depLeft(startMs: number): number {
    return pct(startMs, dependencyWaterfall?.duration_ms ?? 1);
  }

  function depWidth(block: { start_ms: number; end_ms: number; duration_ms: number }): number {
    const width = pct(Math.max(block.duration_ms, block.end_ms - block.start_ms, 1), dependencyWaterfall?.duration_ms ?? 1);
    return Math.max(0.18, width);
  }

  function depTitle(block: { label: string; duration_ms: number; item_count: number; item_unit: string }): string {
    const items = block.item_count ? ` · ${tokens(block.item_count)} ${block.item_unit}` : "";
    return `${block.label} · ${ms(block.duration_ms)}${items}`;
  }

  function itemText(count: number | null | undefined, unit: string | null | undefined): string {
    return count ? `${tokens(count)} ${unit ?? "items"}` : "—";
  }

  function metricPair(row: TracesResponse["memory_stage_timing"][number], key: string, label: string): string | null {
    const metric = row.numeric_metrics?.[key];
    if (!metric) return null;
    return `${label} ${ms(metric.p80)}/${ms(metric.p98)}`;
  }

  function subtimings(row: TracesResponse["memory_stage_timing"][number]): string {
    return [
      metricPair(row, "store_open_ms", "store open"),
      metricPair(row, "zvec_cache_ms", "zvec"),
      metricPair(row, "load_existing_ms", "existing"),
      metricPair(row, "manifest_ms", "manifest"),
      metricPair(row, "load_counts_ms", "counts"),
      metricPair(row, "ensure_recall_index_ms", "ensure"),
      metricPair(row, "embed_ms", "embed"),
      metricPair(row, "store_upsert_ms", "store"),
      metricPair(row, "post_provider_ms", "post"),
    ].filter(Boolean).join(" · ") || "—";
  }
</script>

{#if loading}
  <div class="load">LOADING TRACES…</div>
{:else if data}
  <div class="tr fade-in">
    {#if !data.model_rollup && !data.memory_traces.total && !data.queue_timing && !data.workflow_queue}
      <div class="none">NO NATIVE TRACES — this run is artifact-only (no model/memory/queue traces captured).</div>
    {/if}

    {#if dependencyWaterfall?.lanes?.length}
      <Panel title="Dependency Waterfall" tag="{dependencyWaterfall.lanes.length} sources · setup + blocked spans first" flush>
        <div class="wf dep">
          <div class="wf-axis">
            <div></div>
            <div class="wf-track axis">
              {#each dependencyTicks as tick (tick.ratio)}
                <span class="wf-tick" class:last={tick.ratio === 1} style:left={`${tick.ratio * 100}%`}><i></i><b>{tick.label}</b></span>
              {/each}
            </div>
          </div>
          {#each dependencyWaterfall.lanes as lane (lane.source)}
            <div class="wf-row dep-row">
              <div class="wf-name">
                <span class="bkind memory">source</span>
                <span class="wf-name-text">{lane.source}</span>
                {#if lane.setup_ms}<span class="dep-setup mono-num">setup {ms(lane.setup_ms)}</span>{/if}
                <span class="dep-wait mono-num">wait {ms(lane.wait_ms)}</span>
              </div>
              <div class="wf-track">
                {#each lane.blocks as block, i (block.kind + block.start_ms + i)}
                  <span
                    class="wf-block dep-block {block.kind}"
                    style:left={`${depLeft(block.start_ms)}%`}
                    style:width={`${depWidth(block)}%`}
                    title={depTitle(block)}
                  ></span>
                {/each}
              </div>
            </div>
          {/each}
        </div>
        <div class="bkey">
          <span><i class="setup"></i>setup</span>
          <span><i class="capture"></i>capture</span>
          <span><i class="parallel"></i>raw + distill</span>
          <span><i class="blocked"></i>blocked</span>
          <span><i class="archive"></i>archive/index</span>
          <span><i class="answer"></i>answer</span>
        </div>
      </Panel>
    {/if}

    {#if data.memory_stage_timing?.length}
      <Panel title="Memory Work Timing" tag="batch/window cadence" flush scroll>
        <table class="grid">
          <thead><tr><th>Operation</th><th class="num">Items</th><th class="num">Windows</th><th class="num">p50 Cad</th><th class="num">p80 Cad</th><th class="num">p95 Cad</th><th class="num">p98 Cad</th><th>Subtimings p80/p98</th><th class="num">Mid Err</th></tr></thead>
          <tbody>
            {#each data.memory_stage_timing as s (s.operation)}
              <tr>
                <td class="amber">{s.operation}</td>
                <td class="num mono-num">{s.item_count ? `${tokens(s.item_count)} ${s.item_unit}` : "—"}</td>
                <td class="num mono-num dim">{s.batch_events || "—"}</td>
                <td class="num mono-num dim">{ms(s.work_ms_p50)}</td>
                <td class="num mono-num">{ms(s.work_ms_p80)}</td>
                <td class="num mono-num dim">{ms(s.work_ms_p95)}</td>
                <td class="num mono-num">{ms(s.work_ms_p98)}</td>
                <td class="subtime">{subtimings(s)}</td>
                <td class="num mono-num" class:down={s.intermediate_failed > 0}>{s.intermediate_failed || "—"}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </Panel>
    {/if}

    {#if data.trace_events?.rows?.length}
      <Panel title="Unified Trace Log" tag="{filteredTraceEvents.length}/{data.trace_events.total} events{data.trace_events.truncated ? ' (capped)' : ''}" flush scroll>
        <div class="trace-controls">
          <select class="field mini" bind:value={traceKind}>
            <option value="all">all</option>
            <option value="memory">memory</option>
            <option value="provider">provider</option>
            <option value="error">errors</option>
          </select>
          <input class="field search" bind:value={traceFilter} placeholder="filter op, queue, source, error…" />
        </div>
        <table class="grid trace-log">
          <thead><tr><th>Time</th><th>Kind</th><th>Operation</th><th>Event</th><th class="num">Dur</th><th class="num">Wait</th><th class="num">Run</th><th class="num">Total</th><th class="num">Try</th><th class="num">Items</th><th>Source / Error</th></tr></thead>
          <tbody>
            {#each filteredTraceEvents.slice(0, 1200) as row, i (row.timestamp + row.kind + row.source + row.event + i)}
              <tr>
                <td class="mono-num faint">{(row.timestamp ?? "").slice(11, 23)}</td>
                <td><span class="bkind {row.kind}">{row.kind}</span></td>
                <td>
                  <div class="stacked">
                    <span class="amber">{row.operation}</span>
                    <span class="dim clip">{row.lane}</span>
                  </div>
                </td>
                <td><span class:up={row.status === "succeeded"} class:down={row.status === "failed"}>{row.event}</span></td>
                <td class="num mono-num dim">{ms(row.duration_ms)}</td>
                <td class="num mono-num dim">{ms(row.wait_ms)}</td>
                <td class="num mono-num dim">{ms(row.run_ms)}</td>
                <td class="num mono-num">{ms(row.total_ms)}</td>
                <td class="num mono-num dim">{row.attempt || "—"}</td>
                <td class="num mono-num dim">{itemText(row.item_count, row.item_unit)}</td>
                <td class="clip" class:down={!!row.error}>{row.error ?? row.source ?? "—"}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </Panel>
    {/if}

    {#if bottlenecks.length}
      <Panel title="Bottleneck Overview" tag="p98 slow paths" flush>
        <div class="bottles">
          {#each bottlenecks as row (row.kind + row.name)}
            <div class="brow">
              <div class="bmeta">
                <span class="bkind {row.kind}">{row.kind}</span>
                <span class="bname">{row.name}</span>
              </div>
              <div class="bbar" title={row.meta}>
                {#if row.kind === "provider"}
                  <span class="bseg wait" style:width={`${pct(row.left, bottleneckMax)}%`}></span>
                  <span class="bseg run" style:width={`${pct(row.right, bottleneckMax)}%`}></span>
                {:else}
                  <span class="bseg work" style:width={`${pct(row.right, bottleneckMax)}%`}></span>
                {/if}
              </div>
              <div class="bval mono-num">{row.label}</div>
              <div class="bsub">{row.meta}</div>
            </div>
          {/each}
        </div>
        <div class="bkey">
          <span><i class="work"></i>memory work</span>
          <span><i class="wait"></i>provider wait</span>
          <span><i class="run"></i>provider run</span>
        </div>
      </Panel>
    {/if}

    {#if waterfall?.lanes?.length}
      <Panel title="Trace Waterfall" tag="{waterfall.lanes.length} lanes · {waterfall.block_count} blocks{waterfall.truncated ? ' (capped)' : ''}" flush>
        <div class="wf">
          <div class="wf-axis">
            <div></div>
            <div class="wf-track axis">
              {#each waterfallTicks as tick (tick.ratio)}
                <span class="wf-tick" class:last={tick.ratio === 1} style:left={`${tick.ratio * 100}%`}><i></i><b>{tick.label}</b></span>
              {/each}
            </div>
          </div>
          {#each waterfall.lanes as lane (lane.kind + lane.name)}
            <div class="wf-row">
              <div class="wf-name">
                <span class="bkind {lane.kind}">{lane.kind}</span>
                <span class="wf-name-text">{lane.name}</span>
              </div>
              <div class="wf-track">
                {#each lane.blocks as block, i (block.source + block.kind + block.start_ms + i)}
                  <span
                    class="wf-block {block.kind} {block.status}"
                    style:left={`${blockLeft(block.start_ms)}%`}
                    style:width={`${blockWidth(block)}%`}
                    title={blockTitle(block)}
                  ></span>
                {/each}
              </div>
            </div>
          {/each}
        </div>
        <div class="bkey">
          <span><i class="work"></i>memory work</span>
          <span><i class="wait"></i>provider wait</span>
          <span><i class="run"></i>provider run</span>
          <span><i class="fail"></i>failed</span>
        </div>
      </Panel>
    {/if}

    {#if queueSummaries.length}
      <Panel title="Provider Queue Summary" tag="{queueSummaries.length} queues" flush scroll>
        <table class="grid">
          <thead><tr><th>Queue</th><th class="num">Items</th><th class="num">Fail</th><th class="num">Wait p80</th><th class="num">Wait p95</th><th class="num">Run p80</th><th class="num">Run p95</th><th class="num">Total p80</th><th class="num">Total p98</th></tr></thead>
          <tbody>
            {#each queueSummaries as q (q.name)}
              <tr>
                <td class="amber">{q.name}</td>
                <td class="num mono-num">{q.count}</td>
                <td class="num mono-num" class:down={q.failed > 0}>{q.failed || "—"}</td>
                <td class="num mono-num">{ms(q.wait_p80)}</td>
                <td class="num mono-num dim">{ms(q.wait_p95)}</td>
                <td class="num mono-num">{ms(q.run_p80)}</td>
                <td class="num mono-num dim">{ms(q.run_p95)}</td>
                <td class="num mono-num">{ms(q.total_p80)}</td>
                <td class="num mono-num">{ms(q.total_p98)}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </Panel>
    {/if}

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
    min-width: 0;
  }
  .none {
    padding: 10px;
    background: var(--bg-panel);
    border: 1px solid var(--border-bright);
    color: var(--text-dim);
    font-size: 11px;
  }
  .trace-controls {
    display: grid;
    grid-template-columns: 120px minmax(220px, 1fr);
    gap: 6px;
    padding: 8px;
    border-bottom: 1px solid var(--border);
    background: var(--bg);
  }
  .field {
    background: var(--bg-panel);
    border: 1px solid var(--border-bright);
    color: var(--text);
    font: inherit;
    font-size: 10px;
    min-height: 24px;
    padding: 3px 7px;
  }
  .field.mini {
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }
  .field.search {
    width: 100%;
  }
  .trace-log th,
  .trace-log td {
    white-space: nowrap;
  }
  .stacked {
    display: flex;
    min-width: 0;
    flex-direction: column;
    gap: 1px;
  }
  .stacked .clip {
    max-width: 360px;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .bottles {
    display: flex;
    flex-direction: column;
    gap: 3px;
    padding: 8px;
  }
  .brow {
    display: grid;
    grid-template-columns: minmax(180px, 260px) minmax(140px, 1fr) 110px minmax(180px, 260px);
    gap: 8px;
    align-items: center;
    font-size: 10px;
  }
  .bmeta {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
  }
  .bkind {
    border: 1px solid var(--border-bright);
    padding: 1px 5px;
    font-size: 8px;
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }
  .bkind.memory {
    color: var(--cyan);
  }
  .bkind.provider {
    color: var(--amber);
  }
  .bname {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text);
  }
  .bbar {
    height: 9px;
    background: var(--bg-elev);
    border: 1px solid var(--border);
    display: flex;
    overflow: hidden;
  }
  .bseg {
    display: block;
    height: 100%;
  }
  .bseg.work,
  .bkey .work {
    background: var(--cyan);
  }
  .bseg.wait,
  .bkey .wait {
    background: var(--amber);
  }
  .bseg.run,
  .bkey .run {
    background: var(--green);
  }
  .bkey .fail {
    background: var(--red);
  }
  .bkey .setup {
    background: #2f80ed;
  }
  .bkey .capture {
    background: var(--cyan);
  }
  .bkey .parallel {
    background: var(--cyan);
  }
  .bkey .blocked {
    background: var(--amber);
  }
  .bkey .archive {
    background: var(--green);
  }
  .bkey .answer {
    background: var(--violet);
  }
  .bval {
    color: var(--text);
    text-align: right;
    white-space: nowrap;
  }
  .bsub {
    color: var(--text-faint);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .bkey {
    display: flex;
    flex-wrap: wrap;
    gap: 12px;
    padding: 0 8px 8px;
    color: var(--text-faint);
    font-size: 9px;
  }
  .bkey i {
    display: inline-block;
    width: 10px;
    height: 8px;
    margin-right: 4px;
    vertical-align: -1px;
  }
  .wf {
    max-height: 360px;
    overflow-y: auto;
    overflow-x: hidden;
    padding: 8px;
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
  }
  .wf-axis,
  .wf-row {
    display: grid;
    grid-template-columns: minmax(170px, 280px) minmax(0, 1fr);
    gap: 8px;
    align-items: center;
    min-width: 0;
  }
  .wf.dep {
    max-height: 300px;
  }
  .dep-row .wf-track {
    height: 18px;
  }
  .dep-setup,
  .dep-wait {
    margin-left: auto;
    font-size: 9px;
  }
  .dep-setup {
    color: #46a9ff;
  }
  .dep-wait {
    margin-left: 0;
    color: var(--amber);
  }
  .wf-axis {
    position: sticky;
    top: 0;
    z-index: 2;
    background: var(--bg);
    padding-bottom: 2px;
  }
  .wf-track {
    position: relative;
    height: 16px;
    background: var(--bg-elev);
    border: 1px solid var(--border);
    overflow: hidden;
    min-width: 0;
  }
  .wf-track.axis {
    height: 18px;
    overflow: hidden;
    background:
      linear-gradient(to right, rgba(255,255,255,0.04) 1px, transparent 1px) 0 0 / 25% 100%,
      var(--bg-elev);
  }
  .wf-tick {
    position: absolute;
    top: 0;
    height: 100%;
    color: var(--text-faint);
    font-size: 8px;
    transform: translateX(-1px);
  }
  .wf-tick i {
    display: block;
    width: 1px;
    height: 100%;
    background: var(--border-bright);
  }
  .wf-tick b {
    position: absolute;
    top: 2px;
    left: 4px;
    font-weight: 500;
    white-space: nowrap;
  }
  .wf-tick.last b {
    left: auto;
    right: 4px;
  }
  .wf-name {
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 10px;
  }
  .wf-name-text {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text);
  }
  .wf-block {
    position: absolute;
    top: 3px;
    height: 8px;
    min-width: 2px;
    opacity: 0.92;
    border: 1px solid rgba(255,255,255,0.12);
  }
  .wf-block.memory_work {
    background: var(--cyan);
  }
  .wf-block.dep-block {
    top: 4px;
    height: 8px;
  }
  .wf-block.dep-block.setup {
    background: #2f80ed;
  }
  .wf-block.dep-block.capture {
    background: var(--cyan);
  }
  .wf-block.dep-block.parallel {
    background: var(--cyan);
  }
  .wf-block.dep-block.blocked_distill,
  .wf-block.dep-block.blocked_raw {
    background: var(--amber);
    border-color: rgba(255, 174, 31, 0.7);
  }
  .wf-block.dep-block.archive,
  .wf-block.dep-block.index,
  .wf-block.dep-block.consolidate {
    background: var(--green);
  }
  .wf-block.dep-block.answer {
    background: var(--violet);
  }
  .wf-block.provider_wait {
    background: var(--amber);
  }
  .wf-block.provider_run {
    background: var(--green);
  }
  .wf-block.memory_failed,
  .wf-block.provider_failed,
  .wf-block.failed,
  .wf-block.dead {
    background: var(--red);
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
