<script lang="ts">
  // Unified trace log — memory + provider events with a kind/filter control.
  // Owns its filter state so the parent doesn't re-render when typing here.
  import { ms, tokens } from "../lib/format";
  import type { TraceEventRow } from "../lib/types";

  let {
    events,
    total,
    truncated,
  }: {
    events: TraceEventRow[];
    total: number;
    truncated: boolean;
  } = $props();

  let traceKind = $state("all");
  let traceFilter = $state("");

  const filtered = $derived.by(() => {
    const needle = traceFilter.trim().toLowerCase();
    return events.filter((row) => {
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
      ]
        .join(" ")
        .toLowerCase()
        .includes(needle);
    });
  });

  function itemText(count: number, unit: string | null | undefined): string {
    return count ? `${tokens(count)} ${unit ?? "items"}` : "—";
  }
</script>

<div class="trace-controls">
  <select class="field mini" bind:value={traceKind} aria-label="Filter trace kind">
    <option value="all">all</option>
    <option value="memory">memory</option>
    <option value="provider">provider</option>
    <option value="error">errors</option>
  </select>
  <input class="field search" bind:value={traceFilter} placeholder="filter op, queue, source, error…" spellcheck="false" />
</div>
<table class="grid trace-log">
  <thead>
    <tr>
      <th>Time</th><th>Kind</th><th>Operation</th><th>Event</th>
      <th class="num">Dur</th><th class="num">Wait</th><th class="num">Run</th><th class="num">Total</th>
      <th class="num">Try</th><th class="num">Items</th><th>Source / Error</th>
    </tr>
  </thead>
  <tbody>
    {#each filtered.slice(0, 1200) as row, i (row.timestamp + row.kind + row.source + row.event + i)}
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

<style>
  .trace-controls {
    display: grid;
    grid-template-columns: 120px minmax(220px, 1fr);
    gap: 6px;
    padding: 8px;
    border-bottom: 1px solid var(--border);
    background: var(--bg);
  }
  /* Tighter field sizing than the global .field for this dense table. */
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
  .dim {
    color: var(--text-dim);
  }
  .faint {
    color: var(--text-faint);
  }
</style>
