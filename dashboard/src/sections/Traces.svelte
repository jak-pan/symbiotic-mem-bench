<script lang="ts">
  import { api } from "../lib/api";
  import type { QueueSummaryRow, TracesResponse } from "../lib/types";
  import { ms, tokens, num, shortQueue, isFiniteNumber, clampPct } from "../lib/format";
  import { createAsyncData } from "../lib/async.svelte";
  import Panel from "../components/Panel.svelte";
  import TraceLog from "../components/TraceLog.svelte";
  import TraceWaterfall, { type WfBlock, type WfLane, type WfTick } from "../components/TraceWaterfall.svelte";
  import QueueSummaryTable from "../components/QueueSummary.svelte";

  let { id }: { id: string } = $props();
  const ad = createAsyncData<TracesResponse>();

  $effect(() => {
    const runId = id;
    ad.reset();
    api.traces(runId).then((d) => {
      if (runId !== id) return; // user switched runs mid-flight
      ad.set(d);
    });
  });

  const data = $derived(ad.data);
  const loading = $derived(ad.loading);

  // Aggregated per-queue timing. Sort each sample once and index for all four
  // percentiles (previously every percentile re-sorted a fresh copy).
  const queueSummaries = $derived.by<QueueSummaryRow[]>(() => {
    const groups = new Map<string, NonNullable<TracesResponse["queue_timing"]>[number][]>();
    for (const row of data?.queue_timing ?? []) {
      const key = `${row.operation}:${shortQueue(row.queue_id)}`;
      const rows = groups.get(key) ?? [];
      rows.push(row);
      groups.set(key, rows);
    }
    return [...groups.entries()]
      .map(([name, rows]) => {
        const wait = rows.map((row) => row.wait_ms).filter(isFiniteNumber).sort((a, b) => a - b);
        const run = rows.map((row) => row.run_ms).filter(isFiniteNumber).sort((a, b) => a - b);
        const total = rows.map((row) => row.total_ms).filter(isFiniteNumber).sort((a, b) => a - b);
        const failed = rows.filter((row) => row.final_status === "failed" || row.final_status === "dead").length;
        return {
          name,
          count: rows.length,
          failed,
          wait_p50: percentileSorted(wait, 50),
          wait_p80: percentileSorted(wait, 80),
          wait_p95: percentileSorted(wait, 95),
          wait_p98: percentileSorted(wait, 98),
          run_p50: percentileSorted(run, 50),
          run_p80: percentileSorted(run, 80),
          run_p95: percentileSorted(run, 95),
          run_p98: percentileSorted(run, 98),
          total_p50: percentileSorted(total, 50),
          total_p80: percentileSorted(total, 80),
          total_p95: percentileSorted(total, 95),
          total_p98: percentileSorted(total, 98),
        };
      })
      .sort((a, b) => num(b.total_p98) - num(a.total_p98));
  });

  function percentileSorted(values: number[], p: number): number | null {
    if (!values.length) return null;
    const rank = Math.round((p / 100) * (values.length - 1));
    return values[Math.max(0, Math.min(values.length - 1, rank))];
  }

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

  const waterfallTicks = $derived.by<WfTick[]>(() => {
    const duration = Math.max(1, waterfall?.duration_ms ?? 1);
    return [0, 0.25, 0.5, 0.75, 1].map((ratio) => ({ ratio, label: ms(duration * ratio) }));
  });
  const dependencyTicks = $derived.by<WfTick[]>(() => {
    const duration = Math.max(1, dependencyWaterfall?.duration_ms ?? 1);
    return [0, 0.25, 0.5, 0.75, 1].map((ratio) => ({ ratio, label: ms(duration * ratio) }));
  });

  // Normalise both waterfall shapes into the shared component's lane type.
  const traceLanes = $derived.by<WfLane[]>(
    () =>
      (waterfall?.lanes ?? []).map((lane) => ({
        id: `${lane.kind}:${lane.name}`,
        label: lane.name,
        chipKind: lane.kind,
        blocks: lane.blocks,
      })),
  );
  const dependencyLanes = $derived.by<WfLane[]>(
    () =>
      (dependencyWaterfall?.lanes ?? []).map((lane) => ({
        id: lane.source,
        label: lane.source,
        waitMs: lane.wait_ms,
        setupMs: lane.setup_ms,
        blocks: lane.blocks,
      })),
  );

  function traceBlockTitle(block: WfBlock): string {
    const items = block.item_count ? ` · ${tokens(block.item_count)} ${block.item_unit}` : "";
    return `${block.label} · ${ms(block.duration_ms)} · ${block.status ?? ""}${items} · ${block.source ?? ""}`;
  }
  function depBlockTitle(block: WfBlock): string {
    const items = block.item_count ? ` · ${tokens(block.item_count)} ${block.item_unit}` : "";
    return `${block.label} · ${ms(block.duration_ms)}${items}`;
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
        <TraceWaterfall
          variant="dependency"
          lanes={dependencyLanes}
          ticks={dependencyTicks}
          duration={dependencyWaterfall.duration_ms}
          blockTitle={depBlockTitle}
        />
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
      <Panel title="Unified Trace Log" tag="{data.trace_events.total} events{data.trace_events.truncated ? ' (capped)' : ''}" flush scroll>
        <TraceLog events={data.trace_events.rows} total={data.trace_events.total} truncated={data.trace_events.truncated} />
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
                  <span class="bseg wait" style:width={`${clampPct(row.left, bottleneckMax)}%`}></span>
                  <span class="bseg run" style:width={`${clampPct(row.right, bottleneckMax)}%`}></span>
                {:else}
                  <span class="bseg work" style:width={`${clampPct(row.right, bottleneckMax)}%`}></span>
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
        <TraceWaterfall
          variant="trace"
          lanes={traceLanes}
          ticks={waterfallTicks}
          duration={waterfall.duration_ms}
          blockTitle={traceBlockTitle}
        />
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
        <QueueSummaryTable rows={queueSummaries} />
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
  .subtime {
    font-size: 9.5px;
    color: var(--text-dim);
    white-space: normal;
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
    background: var(--blue);
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
