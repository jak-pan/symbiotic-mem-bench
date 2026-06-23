<script lang="ts">
  import { onDestroy } from "svelte";
  import { api } from "../lib/api";
  import type { LiveResponse, QueueBreakdown, StageProgress, StageSegment } from "../lib/types";
  import { tokens } from "../lib/format";
  import Panel from "../components/Panel.svelte";
  import Bar from "../components/Bar.svelte";

  let { id }: { id: string } = $props();
  let data = $state<LiveResponse | null>(null);
  let err = $state<string | null>(null);
  let timer: ReturnType<typeof setInterval> | undefined;

  async function poll(runId: string) {
    try {
      const d = await api.live(runId);
      if (id === runId) {
        data = d;
        err = null;
      }
    } catch (e) {
      err = (e as Error).message;
    }
  }

  // Re-poll every 2s; restart cleanly when the selected run changes.
  $effect(() => {
    const runId = id;
    data = null;
    err = null;
    poll(runId);
    clearInterval(timer);
    timer = setInterval(() => poll(runId), 2000);
    return () => clearInterval(timer);
  });
  onDestroy(() => clearInterval(timer));

  function fmtAge(secs: number | null): string {
    if (secs == null) return "—";
    if (secs < 60) return `${secs}s`;
    if (secs < 3600) return `${Math.floor(secs / 60)}m ${secs % 60}s`;
    return `${Math.floor(secs / 3600)}h ${Math.floor((secs % 3600) / 60)}m`;
  }

  const queueSegs = $derived.by(() => {
    if (!data) return [];
    const q = data.detail.queue;
    const total = Math.max(1, q.queued + q.running + q.succeeded + q.failed + q.dead);
    return [
      { k: "succeeded", v: q.succeeded, c: "var(--green)" },
      { k: "running", v: q.running, c: "var(--amber)" },
      { k: "queued", v: q.queued, c: "var(--cyan)" },
      { k: "failed", v: q.failed, c: "var(--red)" },
      { k: "dead", v: q.dead, c: "var(--red-dim)" },
    ].map((s) => ({ ...s, pct: (s.v / total) * 100 }));
  });

  const queueSummary = $derived.by(() => {
    const queues = data?.detail.queues ?? [];
    return {
      avgRunning: queues.reduce((sum, q) => sum + num(q.avg_running), 0),
      avgRunningUnits: queues.reduce((sum, q) => sum + num(q.avg_running_units || q.avg_running), 0),
      avgQueued: queues.reduce((sum, q) => sum + num(q.avg_queued), 0),
      avgRpm: queues.reduce((sum, q) => sum + num(q.avg_starts_per_minute), 0),
      avgRpmUnits: queues.reduce((sum, q) => sum + num(q.avg_starts_per_minute_units || q.avg_starts_per_minute), 0),
      peakRunning: queues.reduce((peak, q) => Math.max(peak, num(q.observed_peak_running)), 0),
      peakRunningUnits: queues.reduce((peak, q) => Math.max(peak, num(q.observed_peak_running_units || q.observed_peak_running)), 0),
      peakRpm: queues.reduce((peak, q) => Math.max(peak, num(q.peak_starts_per_minute)), 0),
      peakRpmUnits: queues.reduce((peak, q) => Math.max(peak, num(q.peak_starts_per_minute_units || q.peak_starts_per_minute)), 0),
    };
  });

  function statusLabel(status: string): string {
    if (status === "running") return "LIVE";
    if (status === "warning") return "IDLE";
    if (status === "complete") return "DONE";
    return "STALLED";
  }

  function opLabel(op: string): string {
    if (op === "pre_capture_setup") return "setup";
    if (op === "pre_recall_setup") return "recall setup";
    if (op === "consolidate") return "briefs";
    if (op === "query_plan") return "prompt plan";
    if (op === "embed_query") return "answer embed";
    if (op === "fact_search") return "fact search";
    if (op === "raw_search") return "raw search";
    if (op === "support_check") return "support";
    if (op === "answer_context") return "answer ctx";
    return op;
  }

  function shortQueue(id: string): string {
    return id.replace(/^chat:/, "").replace(/^embedding:/, "");
  }

  function queueUnit(q: QueueBreakdown): string {
    return q.operation === "embedding" ? "texts" : "calls";
  }

  function unitValue(value: number | null | undefined, fallback: number | null | undefined): number {
    const v = num(value);
    return v > 0 ? v : num(fallback);
  }

  function unitText(value: number | null | undefined, fallback: number | null | undefined, unit: string): string {
    const v = unitValue(value, fallback);
    if (unit === "calls") return `${tokens(v)} calls`;
    return `${tokens(v)} ${unit}`;
  }

  function timeOnly(timestamp: string | null): string {
    return (timestamp ?? "").slice(11, 19);
  }

  function num(value: number | null | undefined): number {
    return Number.isFinite(value) ? Number(value) : 0;
  }

  function oneDecimal(value: number | null | undefined): string {
    const v = num(value);
    return v.toFixed(v >= 10 ? 0 : 1);
  }

  function pct(value: number): number {
    return Math.max(0, Math.min(100, Math.round(num(value) * 100)));
  }

  function segmentFill(segment: StageSegment): string {
    const progress = pct(segment.progress);
    if (segment.status === "done") return "var(--green)";
    if (segment.status === "failed") return "var(--red)";
    if (segment.status === "partial") return `linear-gradient(90deg, var(--cyan) 0 ${progress}%, var(--amber) ${progress}% 100%)`;
    if (segment.status === "running") return "var(--amber)";
    return "var(--bg-elev)";
  }

  function stageTitle(stage: StageProgress): string {
    const parts = [
      `${stage.succeeded} jobs done`,
      `${stage.in_flight} in-flight`,
      `${stage.failed} failed`,
      `${stage.started} started`,
      `${stage.item_succeeded} ${stage.item_unit} processed`,
      `intermediate errors ${stage.intermediate_failed}`,
      `last ${stage.last_event ?? "none"} ${timeOnly(stage.last_event_at)}`,
    ];
    return parts.join(" · ");
  }

  function errorLabel(category: string): string {
    return category.replaceAll("_", " ");
  }

  function errorSummary(categories: Array<{ category: string; count: number }>): string {
    const bits = categories
      .slice(0, 4)
      .map((category) => `${errorLabel(category.category)} ${tokens(category.count)}`);
    return bits.length ? ` · ${bits.join(" · ")}` : "";
  }
</script>

<div class="live">
  {#if !data}
    <div class="load">{err ? `ERROR: ${err}` : "READING LIVE STATE…"}</div>
  {:else}
    {@const p = data.pending}
    {@const d = data.detail}
    <div class="lhead">
      <span class="badge" class:running={p.status === "running"} class:warning={p.status === "warning"} class:complete={p.status === "complete"} class:stalled={p.status === "stalled"}>
        <span class="dot"></span>{statusLabel(p.status)}
      </span>
      <span class="ltitle">
        <span class="lname">{p.run_name}</span>
        <span class="lmeta">{p.limit}Q · {p.config_label}{#if p.settings_label} · {p.settings_label}{/if}</span>
      </span>
      <span class="lage">updated {fmtAge(p.age_secs)} ago{#if err} · <span class="down">poll err</span>{/if}</span>
    </div>

    <div class="grid-top">
      <Panel title="Progress" tag="{p.status === 'complete' ? 'run summary' : 'live'} · {d.queue.window} queue events">
        <div class="prog">
          <div class="progress-lines">
            <div class="prow">
              <span class="pl">INGESTED</span>
              <Bar value={p.limit ? p.ingested / p.limit : null} max={1} color="var(--cyan)" height={12} />
              <span class="pv mono-num">{p.ingested}<i>/{p.limit ?? "?"}</i></span>
            </div>
            <div class="prow">
              <span class="pl">ANSWERED</span>
              <Bar value={p.limit ? p.hypotheses / p.limit : null} max={1} color="var(--amber)" height={12} />
              <span class="pv mono-num">{p.hypotheses}<i>/{p.limit ?? "?"}</i></span>
            </div>
          </div>

          <div class="mstats compact">
            <div class="ms"><span class="qt">CALLS</span><b class="mono-num">{d.model.window_calls}</b></div>
            <div class="ms"><span class="qt">FAILED</span><b class="mono-num" class:down={d.model.window_failed > 0}>{d.model.window_failed}</b></div>
            <div class="ms"><span class="qt">IN TOK</span><b class="mono-num">{tokens(d.model.input_tokens)}</b></div>
            <div class="ms"><span class="qt">OUT TOK</span><b class="mono-num">{tokens(d.model.output_tokens)}</b></div>
            <div class="ms"><span class="qt">TRACE</span><b class="mono-num">{(d.model.total_bytes / 1e6).toFixed(1)}MB</b></div>
          </div>
          <div class="queue-block">
            <div class="qbig compact">
              {#if p.status === "complete"}
                <div class="qtile"><span class="qt">AVG RUN</span><b class="amber">{oneDecimal(queueSummary.avgRunning)}</b></div>
                <div class="qtile"><span class="qt">PEAK UNIT</span><b>{tokens(queueSummary.peakRunningUnits)}</b></div>
                <div class="qtile"><span class="qt">AVG RPM</span><b class="cyan">{oneDecimal(queueSummary.avgRpmUnits)}</b></div>
              {:else}
                <div class="qtile"><span class="qt">OPEN</span><b class="amber">{d.queue.in_flight}</b></div>
                <div class="qtile"><span class="qt">RUNNING</span><b>{d.queue.running}</b></div>
                <div class="qtile"><span class="qt">QUEUED</span><b class="cyan">{d.queue.queued}</b></div>
              {/if}
              <div class="qtile"><span class="qt">DONE</span><b class="up">{d.queue.succeeded}</b></div>
              <div class="qtile"><span class="qt">FAILED</span><b class:down={d.queue.failed > 0}>{d.queue.failed}</b></div>
              <div class="qtile"><span class="qt">DEAD</span><b class:down={d.queue.dead > 0}>{d.queue.dead}</b></div>
            </div>
            <div class="qbar">
              {#each queueSegs as s (s.k)}
                {#if s.v > 0}<div class="qseg" style="width:{s.pct}%;background:{s.c}" title="{s.k}: {s.v}"></div>{/if}
              {/each}
            </div>
            {#if d.queues.length}
              <div class="qrows compact">
                {#each d.queues as q (q.queue_id)}
                  {@const unit = queueUnit(q)}
                  <div class="qrow" title={q.queue_id}>
                    <div class="qtop">
                      <span class="qname">{shortQueue(q.queue_id)}</span>
                      <span class="qop">{q.operation}</span>
                    </div>
                    <div class="qmetrics">
                      {#if p.status === "complete"}
                        <span class="mono-num amber">avg {oneDecimal(q.avg_running)}</span>
                      {:else}
                        <span class="mono-num amber">run {num(q.running)}</span>
                        <span class="mono-num cyan">queued {num(q.queued)}</span>
                      {/if}
                      <span class="mono-num">peak {unitText(q.observed_peak_running_units, q.observed_peak_running, unit)}</span>
                      <span class="mono-num dim">calls {num(q.observed_peak_running)}</span>
                      <span class="mono-num cyan">{p.status === "complete" ? "avgrpm" : "rpm"} {p.status === "complete" ? oneDecimal(q.avg_starts_per_minute_units || q.avg_starts_per_minute) : tokens(unitValue(q.starts_last_minute_units, q.starts_last_minute))}</span>
                      <span class="mono-num up">done {q.succeeded}</span>
                      <span class="mono-num" class:down={q.failed + q.dead > 0}>fail {q.failed + q.dead}</span>
                    </div>
                  </div>
                {/each}
              </div>
            {/if}
          </div>
        </div>
      </Panel>

      <Panel title="Pipeline Drilldown" tag="{d.memory_failures} failures">
        <div class="ops">
          {#each d.memory_stages as s (s.operation)}
            <div class="strow">
              <span class="sname">{opLabel(s.operation)}</span>
              <div class="sbar" title={stageTitle(s)}>
                {#if s.segments?.length}
                  {#each s.segments as segment (segment.id)}
                    <div class="vseg {segment.status}" style:flex={1} style:background={segmentFill(segment)} title="{segment.id}: {segment.succeeded}/{segment.started} · {segment.item_succeeded} {s.item_unit}"></div>
                  {/each}
                {:else}
                  <div class="sbar-in">
                    {#if s.succeeded}<div class="seg ok" style="flex:{s.succeeded}"></div>{/if}
                    {#if s.in_flight}<div class="seg fly" style="flex:{s.in_flight}"></div>{/if}
                    {#if s.failed}<div class="seg err" style="flex:{s.failed}"></div>{/if}
                  </div>
                {/if}
              </div>
              <span class="scount mono-num">
                {#if s.item_succeeded}
                  <span class="item-count"><b>{tokens(s.item_succeeded)}</b><i>{s.item_unit}</i></span>
                {/if}
                <span class="job-count">{s.succeeded}<i>/{s.started}</i></span>
                {#if s.intermediate_failed}<span class="mid-err">mid ✗{s.intermediate_failed}</span>{/if}
                {#if s.in_flight}<span class="fly-n">⟳{s.in_flight}</span>{/if}{#if s.failed}<span class="err-n">✗{s.failed}</span>{/if}
              </span>
            </div>
          {/each}
          {#if !d.memory_stages.length}<div class="faint">no memory operations yet</div>{/if}
          <div class="seg-key">
            <span><i class="seg ok"></i>done</span>
            <span><i class="seg part"></i>partial</span>
            <span><i class="seg fly"></i>in-flight</span>
            <span><i class="seg err"></i>failed</span>
          </div>
        </div>
      </Panel>
    </div>

    <div class="grid-bot">
      {#if d.errors.length}
        <Panel title="Error Log" tag="{d.errors.length} retained{errorSummary(d.error_categories)}" scroll>
          <div class="error-log">
            {#each d.errors as e, i (i)}
              <div class="erow">
                <span class="ets mono-num">{timeOnly(e.timestamp)}</span>
                <span class="esrc {e.source}">{e.source}</span>
                <span class="ekind">{e.kind ?? "error"}</span>
                <span class="emsg">{e.message}</span>
              </div>
            {/each}
          </div>
        </Panel>
      {/if}
      <Panel title="Recent Activity" tag="{d.errors.length} errors" scroll>
        {#if d.activity.length}
          <div class="activity">
            {#each d.activity as a, i (i)}
              <div class="arow" class:error={a.severity === "error"}>
                <span class="ets mono-num">{timeOnly(a.timestamp)}</span>
                <span class="esrc {a.source}">{a.source}</span>
                <span class="aop">{opLabel(a.operation)}</span>
                <span class="astatus">{a.status}</span>
                <span class="emsg">{a.message}</span>
              </div>
            {/each}
          </div>
        {:else}
          <div class="ok">✓ no activity in recent window</div>
        {/if}
      </Panel>
    </div>

    <div class="poll-note">↻ auto-refreshing every 2s · live stats read from the run root (tailed)</div>
  {/if}
</div>

<style>
  .live {
    padding: 6px;
    display: flex;
    flex-direction: column;
    gap: 6px;
    height: 100%;
    min-height: 0;
  }
  .load {
    padding: 40px;
    color: var(--text-faint);
    letter-spacing: 0.2em;
  }
  .lhead {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 6px 10px;
    background: var(--bg-panel);
    border: 1px solid var(--border-bright);
  }
  .badge {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-family: var(--sans);
    font-weight: 800;
    font-size: 10px;
    letter-spacing: 0.12em;
    padding: 3px 8px;
    border: 1px solid;
  }
  .badge .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
  }
  .badge.running {
    color: var(--green);
    border-color: var(--green-dim);
    background: rgba(47, 207, 122, 0.08);
  }
  .badge.running .dot {
    background: var(--green);
    box-shadow: 0 0 6px var(--green);
    animation: pulse 1.4s ease infinite;
  }
  .badge.warning {
    color: var(--amber);
    border-color: var(--amber-dim);
    background: rgba(255, 165, 36, 0.08);
  }
  .badge.warning .dot {
    background: var(--amber);
    box-shadow: 0 0 5px var(--amber-dim);
  }
  .badge.stalled {
    color: var(--text-dim);
    border-color: var(--border-bright);
  }
  .badge.stalled .dot {
    background: var(--text-faint);
  }
  .badge.complete {
    color: var(--cyan);
    border-color: var(--cyan-dim);
    background: rgba(82, 166, 255, 0.08);
  }
  .badge.complete .dot {
    background: var(--cyan);
  }
  @keyframes pulse {
    50% {
      opacity: 0.35;
    }
  }
  .lname {
    font-weight: 700;
    color: var(--text);
  }
  .ltitle {
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .lmeta {
    color: var(--text-dim);
    font-size: 11px;
    overflow-wrap: anywhere;
  }
  .lage {
    margin-left: auto;
    color: var(--text-faint);
    font-size: 11px;
  }

  .grid-top {
    display: grid;
    grid-template-columns: minmax(340px, 0.9fr) minmax(420px, 1.15fr);
    gap: 6px;
    min-height: 0;
    min-width: 0;
  }
  .grid-top :global(.panel) {
    min-width: 0;
  }
  .grid-bot {
    display: grid;
    grid-template-columns: 1fr;
    gap: 6px;
    min-height: 0;
    flex: 1;
  }
  .grid-bot :global(.panel) {
    min-height: 0;
  }

  .prog {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 0;
  }
  .progress-lines {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .prow {
    display: grid;
    grid-template-columns: 72px 1fr 70px;
    align-items: center;
    gap: 8px;
  }
  .pl {
    font-family: var(--sans);
    font-size: 9px;
    font-weight: 700;
    letter-spacing: 0.1em;
    color: var(--text-faint);
  }
  .pv {
    text-align: right;
    font-size: 14px;
    color: var(--text);
  }
  .pv i {
    font-size: 10px;
    color: var(--text-faint);
    font-style: normal;
  }

  .qbig {
    display: grid;
    grid-template-columns: repeat(6, 1fr);
    gap: 1px;
    background: var(--border);
    border: 1px solid var(--border);
    margin-bottom: 8px;
  }
  .qbig.compact {
    margin-bottom: 6px;
  }
  .qtile {
    background: var(--bg-panel);
    padding: 4px 6px;
    display: flex;
    flex-direction: column;
    gap: 1px;
    align-items: flex-start;
  }
  .qt {
    font-family: var(--sans);
    font-size: 7.5px;
    font-weight: 700;
    letter-spacing: 0.08em;
    color: var(--text-faint);
  }
  .qtile b {
    font-size: 16px;
    font-weight: 600;
    color: var(--text);
  }
  .qbar {
    display: flex;
    height: 9px;
    border: 1px solid var(--border);
    overflow: hidden;
  }
  .qseg {
    height: 100%;
  }
  .qrows {
    display: flex;
    flex-direction: column;
    gap: 4px;
    max-height: 116px;
    overflow: auto;
    padding-top: 3px;
  }
  .qrow {
    display: flex;
    flex-direction: column;
    gap: 2px;
    font-size: 9.5px;
    color: var(--text-dim);
    border-top: 1px solid var(--border);
    padding: 3px 0 2px;
    min-width: 0;
  }
  .qrows.compact {
    max-height: 92px;
    overflow-x: hidden;
  }
  .qtop {
    display: flex;
    align-items: baseline;
    gap: 8px;
    min-width: 0;
  }
  .qmetrics {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
    overflow: hidden;
    color: var(--text-faint);
  }
  .qname {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text);
    min-width: 0;
  }
  .qop {
    flex: none;
    color: var(--text-faint);
    font-family: var(--sans);
    font-size: 7.5px;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }
  .cyan {
    color: var(--cyan);
  }

  .mstats {
    display: grid;
    grid-template-columns: repeat(5, 1fr);
    gap: 1px;
    background: var(--border);
    border: 1px solid var(--border);
  }
  .mstats.compact .ms {
    min-width: 0;
    padding: 3px 6px;
  }
  .mstats.compact .ms b {
    font-size: 12px;
  }
  .ms {
    background: var(--bg-panel);
    padding: 8px 6px;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .ms b {
    font-size: 15px;
    font-weight: 600;
    color: var(--text);
  }

  .ops {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .progress-ops {
    gap: 4px;
    padding-top: 2px;
  }
  .strow {
    display: grid;
    grid-template-columns: 92px minmax(72px, 150px) minmax(184px, 1fr);
    align-items: center;
    gap: 5px;
    min-width: 0;
    min-height: 15px;
    justify-content: start;
  }
  .sname {
    font-size: 10px;
    color: var(--text-dim);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .sbar {
    height: 4px;
    background: var(--bg-elev);
    border: 1px solid var(--border);
    min-width: 0;
    max-width: 150px;
    display: flex;
    gap: 1px;
    overflow: hidden;
  }
  .sbar-in {
    width: 100%;
    height: 100%;
    display: flex;
    transition: width 0.4s ease;
  }
  .seg {
    height: 100%;
  }
  .seg.ok {
    background: var(--green);
  }
  .seg.fly {
    background: var(--amber);
  }
  .seg.err {
    background: var(--red);
  }
  .seg.part {
    background: var(--cyan);
  }
  .vseg {
    min-width: 2px;
    height: 100%;
    opacity: 0.95;
  }
  .vseg.partial {
    opacity: 1;
  }
  .vseg.running {
    opacity: 0.9;
  }
  .scount {
    text-align: right;
    font-size: 8.5px;
    color: var(--text);
    white-space: nowrap;
    display: flex;
    justify-content: flex-end;
    align-items: baseline;
    gap: 3px;
    line-height: 1.05;
    min-width: 0;
  }
  .scount i {
    color: var(--text-faint);
    font-style: normal;
  }
  .job-count {
    color: var(--text);
  }
  .item-count {
    color: var(--cyan);
    display: inline-flex;
    align-items: baseline;
    gap: 2px;
    font-size: 9px;
  }
  .item-count b {
    color: var(--cyan);
    font-weight: 600;
  }
  .item-count i {
    color: var(--text-faint);
    font-size: 8px;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }
  .mid-err {
    color: var(--red);
    font-size: 8.5px;
  }
  .fly-n {
    color: var(--amber);
    margin-left: 4px;
  }
  .err-n {
    color: var(--red);
    margin-left: 4px;
  }
  .seg-key {
    display: flex;
    gap: 12px;
    margin-top: 5px;
    font-size: 8.5px;
    color: var(--text-faint);
    letter-spacing: 0.04em;
  }
  .seg-key i {
    display: inline-block;
    width: 9px;
    height: 9px;
    vertical-align: middle;
    margin-right: 3px;
  }
  .activity {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .error-log {
    display: flex;
    flex-direction: column;
    gap: 2px;
    max-height: 220px;
  }
  .erow {
    display: grid;
    grid-template-columns: 54px 58px 86px 1fr;
    gap: 8px;
    align-items: baseline;
    padding: 3px 0;
    border-bottom: 1px solid var(--border);
    background: rgba(255, 79, 79, 0.04);
    font-size: 10.5px;
  }
  .ekind {
    color: var(--red);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .arow {
    display: grid;
    grid-template-columns: 54px 58px 82px 94px 1fr;
    gap: 8px;
    align-items: baseline;
    padding: 3px 0;
    border-bottom: 1px solid var(--border);
    font-size: 10.5px;
  }
  .arow.error {
    background: rgba(255, 79, 79, 0.04);
  }
  .esrc {
    font-family: var(--sans);
    font-size: 8px;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    padding: 1px 4px;
    border: 1px solid var(--border-bright);
    color: var(--text-faint);
    text-align: center;
  }
  .esrc.model {
    color: var(--amber);
  }
  .esrc.memory {
    color: var(--violet);
  }
  .esrc.provider {
    color: var(--cyan);
  }
  .aop {
    color: var(--text-dim);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .astatus {
    color: var(--text-faint);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .emsg {
    color: var(--text-dim);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .arow.error .emsg,
  .arow.error .astatus {
    color: var(--red);
  }
  .ets {
    color: var(--text-faint);
    font-size: 9.5px;
  }
  .ok {
    color: var(--green);
    font-size: 11px;
  }
  .faint {
    color: var(--text-faint);
  }
  .poll-note {
    font-size: 9.5px;
    color: var(--text-faint);
    letter-spacing: 0.04em;
  }

  @media (max-width: 980px) {
    .grid-top {
      grid-template-columns: 1fr;
    }
  }
</style>
