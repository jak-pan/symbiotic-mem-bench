<script lang="ts">
  import { onDestroy } from "svelte";
  import { api } from "../lib/api";
  import type { LiveResponse } from "../lib/types";
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

  const maxStarted = $derived(
    Math.max(1, ...(data?.detail.memory_stages.map((s) => s.started) ?? [1])),
  );
</script>

<div class="live">
  {#if !data}
    <div class="load">{err ? `ERROR: ${err}` : "READING LIVE STATE…"}</div>
  {:else}
    {@const p = data.pending}
    {@const d = data.detail}
    <div class="lhead">
      <span class="badge" class:running={p.status === "running"} class:warning={p.status === "warning"} class:stalled={p.status === "stalled"}>
        <span class="dot"></span>{p.status === "running" ? "LIVE" : p.status === "warning" ? "IDLE" : "STALLED"}
      </span>
      <span class="lname">{p.run_name}</span>
      <span class="lmeta">{p.limit}Q · {p.config_label}</span>
      <span class="lage">updated {fmtAge(p.age_secs)} ago{#if err} · <span class="down">poll err</span>{/if}</span>
    </div>

    <div class="grid-top">
      <Panel title="Progress">
        <div class="prog">
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
      </Panel>

      <Panel title="Provider Queue" tag="pressure · {d.queue.window} recent">
        <div class="qbig">
          <div class="qtile"><span class="qt">IN-FLIGHT</span><b class="amber">{d.queue.in_flight}</b></div>
          <div class="qtile"><span class="qt">RUNNING</span><b>{d.queue.running}</b></div>
          <div class="qtile"><span class="qt">QUEUED</span><b class="cyan">{d.queue.queued}</b></div>
          <div class="qtile"><span class="qt">DONE</span><b class="up">{d.queue.succeeded}</b></div>
          <div class="qtile"><span class="qt">FAILED</span><b class:down={d.queue.failed > 0}>{d.queue.failed}</b></div>
          <div class="qtile"><span class="qt">DEAD</span><b class:down={d.queue.dead > 0}>{d.queue.dead}</b></div>
        </div>
        <div class="qbar">
          {#each queueSegs as s (s.k)}
            {#if s.v > 0}<div class="qseg" style="width:{s.pct}%;background:{s.c}" title="{s.k}: {s.v}"></div>{/if}
          {/each}
        </div>
      </Panel>
    </div>

    <div class="grid-bot">
      <Panel title="Model Throughput" tag="recent window">
        <div class="mstats">
          <div class="ms"><span class="qt">CALLS</span><b class="mono-num">{d.model.window_calls}</b></div>
          <div class="ms"><span class="qt">FAILED</span><b class="mono-num" class:down={d.model.window_failed > 0}>{d.model.window_failed}</b></div>
          <div class="ms"><span class="qt">IN TOK</span><b class="mono-num">{tokens(d.model.input_tokens)}</b></div>
          <div class="ms"><span class="qt">OUT TOK</span><b class="mono-num">{tokens(d.model.output_tokens)}</b></div>
          <div class="ms"><span class="qt">TRACE</span><b class="mono-num">{(d.model.total_bytes / 1e6).toFixed(1)}MB</b></div>
        </div>
      </Panel>

      <Panel title="Memory Pipeline" tag="{d.memory_failures} failures">
        <div class="ops">
          {#each d.memory_stages as s (s.operation)}
            <div class="strow">
              <span class="sname">{s.operation}</span>
              <div class="sbar" title="{s.succeeded} done · {s.in_flight} in-flight · {s.failed} failed of {s.started} started">
                <div class="sbar-in" style="width:{(s.started / maxStarted) * 100}%">
                  {#if s.succeeded}<div class="seg ok" style="flex:{s.succeeded}"></div>{/if}
                  {#if s.in_flight}<div class="seg fly" style="flex:{s.in_flight}"></div>{/if}
                  {#if s.failed}<div class="seg err" style="flex:{s.failed}"></div>{/if}
                </div>
              </div>
              <span class="scount mono-num">
                {s.succeeded}<i>/{s.started}</i>{#if s.in_flight}<span class="fly-n">⟳{s.in_flight}</span>{/if}{#if s.failed}<span class="err-n">✗{s.failed}</span>{/if}
              </span>
            </div>
          {/each}
          {#if !d.memory_stages.length}<div class="faint">no memory operations yet</div>{/if}
          <div class="seg-key">
            <span><i class="seg ok"></i>done</span>
            <span><i class="seg fly"></i>in-flight</span>
            <span><i class="seg err"></i>failed</span>
          </div>
        </div>
      </Panel>

      <Panel title="Recent Errors" tag={String(d.errors.length)} scroll>
        {#if d.errors.length}
          <div class="errs">
            {#each d.errors as e, i (i)}
              <div class="erow">
                <span class="esrc {e.source}">{e.source}</span>
                <span class="emsg">{e.kind ? `[${e.kind}] ` : ""}{e.message}</span>
                <span class="ets mono-num">{(e.timestamp ?? "").slice(11, 19)}</span>
              </div>
            {/each}
          </div>
        {:else}
          <div class="ok">✓ no errors in recent window</div>
        {/if}
      </Panel>
    </div>

    <div class="poll-note">↻ auto-refreshing every 2s · live stats read from the run root (tailed)</div>
  {/if}
</div>

<style>
  .live {
    padding: 10px;
    display: flex;
    flex-direction: column;
    gap: 10px;
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
    padding: 8px 12px;
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
  @keyframes pulse {
    50% {
      opacity: 0.35;
    }
  }
  .lname {
    font-weight: 700;
    color: var(--text);
  }
  .lmeta {
    color: var(--text-dim);
    font-size: 11px;
  }
  .lage {
    margin-left: auto;
    color: var(--text-faint);
    font-size: 11px;
  }

  .grid-top {
    display: grid;
    grid-template-columns: 1fr 1.4fr;
    gap: 10px;
  }
  .grid-bot {
    display: grid;
    grid-template-columns: 1fr 1fr 1.3fr;
    gap: 10px;
    min-height: 0;
    flex: 1;
  }
  .grid-bot :global(.panel) {
    min-height: 0;
  }

  .prog {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 6px 0;
  }
  .prow {
    display: grid;
    grid-template-columns: 72px 1fr 70px;
    align-items: center;
    gap: 10px;
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
  .qtile {
    background: var(--bg-panel);
    padding: 7px 6px;
    display: flex;
    flex-direction: column;
    gap: 2px;
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
    font-size: 18px;
    font-weight: 600;
    color: var(--text);
  }
  .qbar {
    display: flex;
    height: 12px;
    border: 1px solid var(--border);
    overflow: hidden;
  }
  .qseg {
    height: 100%;
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
    gap: 5px;
  }
  .strow {
    display: grid;
    grid-template-columns: 88px 1fr 88px;
    align-items: center;
    gap: 8px;
  }
  .sname {
    font-size: 10px;
    color: var(--text-dim);
  }
  .sbar {
    height: 11px;
    background: var(--bg-elev);
    border: 1px solid var(--border);
  }
  .sbar-in {
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
  .scount {
    text-align: right;
    font-size: 10.5px;
    color: var(--text);
    white-space: nowrap;
  }
  .scount i {
    color: var(--text-faint);
    font-style: normal;
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

  .errs {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .erow {
    display: grid;
    grid-template-columns: 54px 1fr auto;
    gap: 8px;
    align-items: baseline;
    padding: 3px 0;
    border-bottom: 1px solid var(--border);
    font-size: 10.5px;
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
  .emsg {
    color: var(--red);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
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
</style>
