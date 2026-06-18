<script lang="ts">
  import { onMount } from "svelte";
  import { api } from "../lib/api";
  import { router } from "../lib/router.svelte";
  import { QTYPES, type Cohort, type RankedRow } from "../lib/types";
  import {
    pct,
    pctSign,
    money,
    ms,
    ago,
    shortHash,
    qtypeShort,
    qtypeAbbr,
    heatColor,
    deltaClass,
  } from "../lib/format";
  import Panel from "../components/Panel.svelte";
  import Bar from "../components/Bar.svelte";
  import CategoryHeat from "../components/CategoryHeat.svelte";
  import RingGauge from "../components/RingGauge.svelte";
  import Radar from "../components/Radar.svelte";
  import DeltaBars from "../components/DeltaBars.svelte";

  let cohorts = $state<Cohort[]>([]);
  let activeId = $state<string>("");
  let loading = $state(true);
  let sortKey = $state<string>("rank");
  let sortDir = $state<1 | -1>(1);
  let selected = $state<string[]>([]);

  const SEL_COLORS = ["var(--amber)", "var(--cyan)", "var(--green)", "var(--violet)"];

  onMount(async () => {
    cohorts = await api.leaderboard();
    if (cohorts.length) activeId = cohorts[0].cohort_id;
    loading = false;
  });

  const active = $derived(cohorts.find((c) => c.cohort_id === activeId));

  const rows = $derived.by(() => {
    if (!active) return [];
    const rs = [...active.rows];
    const dir = sortDir;
    rs.sort((a, b) => {
      const av = sortVal(a, sortKey);
      const bv = sortVal(b, sortKey);
      if (av == null && bv == null) return 0;
      if (av == null) return 1;
      if (bv == null) return -1;
      return av < bv ? -dir : av > bv ? dir : 0;
    });
    return rs;
  });

  function sortVal(r: RankedRow, key: string): number | string | null {
    switch (key) {
      case "rank": return r.rank;
      case "run": return r.run_name;
      case "acc": return r.accuracy;
      case "tavg": return r.task_averaged_accuracy;
      case "abst": return r.abstention_accuracy;
      case "cost": return r.cost_micro_usd;
      case "lat": return r.latency_ms_p50;
      case "age": return r.modified_ms;
      default: return r.rank;
    }
  }
  function setSort(key: string) {
    if (sortKey === key) sortDir = (sortDir * -1) as 1 | -1;
    else {
      sortKey = key;
      sortDir = key === "run" ? 1 : key === "rank" ? 1 : -1;
    }
  }

  function toggle(id: string) {
    if (selected.includes(id)) selected = selected.filter((s) => s !== id);
    else if (selected.length < 4) selected = [...selected, id];
  }
  function colorFor(id: string): string {
    const i = selected.indexOf(id);
    return i >= 0 ? SEL_COLORS[i] : "var(--text-faint)";
  }

  function radarValues(r: RankedRow): (number | null)[] {
    return QTYPES.map((qt) => r.per_question_type?.[qt]?.accuracy ?? null);
  }

  // Resolve the selected ids to rows (called inline in the template so it stays
  // reactive to `selected`).
  function pickRows(ids: string[], c: Cohort | undefined): RankedRow[] {
    if (!c) return [];
    return ids
      .map((id) => c.rows.find((r) => r.run_id === id))
      .filter((r): r is RankedRow => !!r);
  }

  // Per-category delta of the last-selected run vs the cohort leader.
  function deltaVsLeader(rowsSel: RankedRow[], c: Cohort | undefined) {
    if (!rowsSel.length || !c) return [];
    const leader = c.rows[0];
    const target = rowsSel[rowsSel.length - 1];
    if (!leader || target.run_id === leader.run_id) return [];
    return QTYPES.map((qt) => ({
      label: qtypeShort(qt),
      delta:
        (target.per_question_type?.[qt]?.accuracy ?? 0) -
        (leader.per_question_type?.[qt]?.accuracy ?? 0),
    }));
  }

  const hasCostAxis = $derived(
    active?.rows.some((r) => r.latency_ms_p50 != null || r.cost_micro_usd != null) ?? false,
  );
</script>

<div class="lb">
  <div class="main">
    <!-- cohort selector -->
    <div class="cohort-strip">
      <span class="label" style="margin-right:4px">COHORT</span>
      {#each cohorts as c (c.cohort_id)}
        <button class="cohort-chip" class:on={c.cohort_id === activeId} onclick={() => { activeId = c.cohort_id; selected = []; }}>
          <span class="cb">{c.benchmark}</span>
          <span class="cs">·{c.limit ?? "?"}Q</span>
          <span class="cn">{c.run_count}</span>
          {#if !c.strictly_comparable}<span class="warn" title="mixed question set or judge">⚠</span>{/if}
        </button>
      {/each}
    </div>

    {#if loading}
      <div class="empty">SCANNING REGISTRY…</div>
    {:else if !active}
      <div class="empty">NO COHORTS</div>
    {:else}
      {#if !active.strictly_comparable}
        <div class="integrity">
          ⚠ COHORT NOT STRICTLY COMPARABLE —
          {active.dataset_fingerprints.length} question-set fingerprint(s),
          {active.judge_models.length} judge(s). Rank by sub-group with care.
        </div>
      {/if}

      <!-- hero row -->
      <div class="hero">
        <Panel title="Cohort" tag={active.benchmark}>
          <div class="cohort-hero">
            <RingGauge value={active.best_accuracy} label="peak acc" color="var(--amber)" size={118} />
            <dl class="meta">
              <dt>SIZE</dt><dd>{active.limit ?? "?"} questions</dd>
              <dt>FIELD</dt><dd>{active.run_count} systems</dd>
              <dt>JUDGE</dt><dd>{active.judge_models.join(", ") || "—"}</dd>
              <dt>QSET</dt><dd class="mono-num">{shortHash(active.dataset_fingerprints[0], 12)}</dd>
              <dt>COMPARABLE</dt>
              <dd>{#if active.strictly_comparable}<span class="up">● STRICT</span>{:else}<span class="down">● MIXED</span>{/if}</dd>
            </dl>
          </div>
        </Panel>

        <Panel title="Field Ranking" tag="overall accuracy" scroll>
          <div class="field">
            {#each active.rows as r, i (r.run_id)}
              <button class="frow" onclick={() => toggle(r.run_id)} class:sel={selected.includes(r.run_id)}>
                <span class="fr-rank" class:gold={i === 0}>{r.rank}</span>
                <span class="fr-name" title={r.run_name}>{r.run_name}</span>
                <span class="fr-bar">
                  <Bar value={r.accuracy} max={1} marker={r.task_averaged_accuracy}
                    color={i === 0 ? "var(--amber)" : "var(--amber-dim)"} height={10} />
                </span>
                <span class="fr-val mono-num" class:amber={i === 0}>{pct(r.accuracy)}</span>
              </button>
            {/each}
            <div class="field-key">
              <span><i class="bar-k"></i> overall</span>
              <span><i class="mark-k"></i> task-avg</span>
            </div>
          </div>
        </Panel>

        <Panel title="Category Matrix" tag="by question type" scroll>
          <div class="matrix">
            <div class="mx-head">
              <span></span>
              {#each QTYPES as qt (qt)}<span class="mx-col" title={qt}>{qtypeAbbr(qt)}</span>{/each}
            </div>
            {#each active.rows as r (r.run_id)}
              <div class="mx-row">
                <span class="mx-name" title={r.run_name}>{r.run_name}</span>
                {#each QTYPES as qt (qt)}
                  {@const s = r.per_question_type?.[qt]}
                  <span class="mx-cell" style="background:{s ? heatColor(s.accuracy) : 'var(--bg-elev)'}"
                    title={s ? `${qtypeShort(qt)} ${pct(s.accuracy)}% (${s.correct}/${s.n})` : 'no data'}>
                    {s ? Math.round(s.accuracy * 100) : ""}
                  </span>
                {/each}
              </div>
            {/each}
          </div>
        </Panel>
      </div>

      <!-- ranked table -->
      <Panel title="Leaderboard" tag="{active.benchmark} · {active.limit}Q" flush scroll>
        <table class="grid">
          <thead>
            <tr>
              <th class="sortable" onclick={() => setSort("rank")} style="width:34px">#</th>
              <th class="sortable" onclick={() => setSort("run")}>System / Config</th>
              <th style="width:70px">Kind</th>
              <th class="sortable num" onclick={() => setSort("acc")} style="width:150px">Accuracy</th>
              <th class="sortable num" onclick={() => setSort("tavg")}>Task·Avg</th>
              <th class="sortable num" onclick={() => setSort("abst")}>Abst</th>
              <th style="width:108px">Categories</th>
              <th class="sortable num" onclick={() => setSort("cost")}>Cost</th>
              <th class="sortable num" onclick={() => setSort("lat")}>Lat·p50</th>
              <th class="sortable num" onclick={() => setSort("age")}>Updated</th>
            </tr>
          </thead>
          <tbody>
            {#each rows as r (r.run_id)}
              <tr class:selected={selected.includes(r.run_id)} onclick={() => toggle(r.run_id)}>
                <td class="rank" class:gold={r.rank === 1}>
                  {#if selected.includes(r.run_id)}<span style="color:{colorFor(r.run_id)}">◆</span>{:else}{r.rank}{/if}
                </td>
                <td>
                  <button class="runlink" onclick={(e) => { e.stopPropagation(); router.openRun(r.run_id); }}>{r.run_name}</button>
                  <span class="cfg">{r.config_label}</span>
                </td>
                <td><span class="chip {r.run_kind === 'native' ? 'green' : 'cyan'}">{r.run_kind === "imported-artifact" ? "import" : r.run_kind}</span></td>
                <td class="num">
                  <div class="acccell">
                    <Bar value={r.accuracy} max={1} marker={r.task_averaged_accuracy} height={8}
                      color={r.rank === 1 ? "var(--amber)" : "var(--amber-dim)"} />
                    <b class="mono-num">{pct(r.accuracy)}</b>
                  </div>
                </td>
                <td class="num mono-num dim">{pct(r.task_averaged_accuracy)}</td>
                <td class="num mono-num dim">{pct(r.abstention_accuracy)}</td>
                <td><CategoryHeat scores={r.per_question_type} /></td>
                <td class="num mono-num dim">{money(r.cost_micro_usd)}</td>
                <td class="num mono-num dim">{ms(r.latency_ms_p50)}</td>
                <td class="num mono-num faint">{ago(r.modified_ms)}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </Panel>
    {/if}
  </div>

  <!-- compare / inspector rail -->
  <aside class="rail">
    <Panel title="Compare" tag={selected.length ? `${selected.length} selected` : "select rows"} scroll>
      {#if selected.length === 0}
        <div class="rail-empty">
          <p>Click rows to stack up to 4 systems for head-to-head comparison.</p>
          {#if active && hasCostAxis}
            <p class="dim">Accuracy vs latency available for this cohort.</p>
          {/if}
        </div>
      {:else}
        {@const srows = pickRows(selected, active)}
        {@const dItems = deltaVsLeader(srows, active)}
        <div class="cmp-legend">
          {#each srows as r, i (r.run_id)}
            <div class="cl"><i style="background:{SEL_COLORS[i]}"></i>{r.run_name}</div>
          {/each}
        </div>

        <div class="radar-wrap">
          <Radar
            axes={QTYPES.map((q) => qtypeAbbr(q))}
            series={srows.map((r, i) => ({ label: r.run_name, color: SEL_COLORS[i], values: radarValues(r) }))}
          />
        </div>

        <table class="cmp-table">
          <tbody>
            <tr><th></th>{#each srows as r, i (r.run_id)}<th style="color:{SEL_COLORS[i]}">{r.run_name.slice(0, 10)}</th>{/each}</tr>
            <tr><td>ACC</td>{#each srows as r (r.run_id)}<td class="mono-num">{pct(r.accuracy)}</td>{/each}</tr>
            <tr><td>T·AVG</td>{#each srows as r (r.run_id)}<td class="mono-num">{pct(r.task_averaged_accuracy)}</td>{/each}</tr>
            <tr><td>ABST</td>{#each srows as r (r.run_id)}<td class="mono-num">{pct(r.abstention_accuracy)}</td>{/each}</tr>
            <tr><td>COST</td>{#each srows as r (r.run_id)}<td class="mono-num">{money(r.cost_micro_usd)}</td>{/each}</tr>
            <tr><td>LAT</td>{#each srows as r (r.run_id)}<td class="mono-num">{ms(r.latency_ms_p50)}</td>{/each}</tr>
          </tbody>
        </table>

        {#if dItems.length}
          <div class="rail-sub label">Δ vs leader ({active?.rows[0]?.run_name.slice(0, 14)})</div>
          <DeltaBars items={dItems} />
        {/if}

        <button class="btn" style="width:100%;margin-top:10px;justify-content:center" onclick={() => router.openRun(srows[0].run_id)}>
          OPEN {srows[0].run_name.slice(0, 16)} IN DEBUGGER →
        </button>
      {/if}
    </Panel>
  </aside>
</div>

<style>
  .lb {
    flex: 1;
    display: grid;
    grid-template-columns: 1fr 360px;
    gap: 1px;
    background: var(--border);
    min-height: 0;
  }
  .main {
    background: var(--bg);
    overflow: hidden;
    padding: 10px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  /* The leaderboard table panel (direct child of .main, outside .hero) fills the
     remaining height and scrolls internally so the hero stays fixed. */
  .main > :global(.panel) {
    flex: 1;
    min-height: 0;
  }
  .rail {
    background: var(--bg);
    min-height: 0;
    display: flex;
  }
  .rail :global(.panel) {
    flex: 1;
  }

  .cohort-strip {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
  }
  .cohort-chip {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 10px;
    background: var(--bg-panel);
    border: 1px solid var(--border-bright);
    color: var(--text-dim);
    cursor: pointer;
    font-size: 11px;
  }
  .cohort-chip:hover {
    border-color: var(--amber-dim);
  }
  .cohort-chip.on {
    background: rgba(255, 165, 36, 0.08);
    border-color: var(--amber);
    color: var(--text);
  }
  .cohort-chip .cb {
    font-weight: 600;
  }
  .cohort-chip .cs {
    color: var(--amber);
  }
  .cohort-chip .cn {
    font-size: 9px;
    background: var(--bg-elev);
    border: 1px solid var(--border);
    padding: 0 4px;
    color: var(--text-faint);
  }
  .cohort-chip .warn {
    color: var(--gold);
  }

  .integrity {
    padding: 6px 10px;
    background: rgba(232, 195, 74, 0.08);
    border: 1px solid var(--gold);
    color: var(--gold);
    font-size: 10.5px;
    letter-spacing: 0.03em;
  }

  .hero {
    display: grid;
    grid-template-columns: 312px 1fr 1fr;
    gap: 10px;
    height: 244px;
    flex: none;
  }
  .hero :global(.panel) {
    min-height: 0;
  }

  .cohort-hero {
    display: flex;
    gap: 14px;
    align-items: center;
  }
  .meta {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 2px 10px;
    align-content: center;
  }
  .meta dt {
    font-family: var(--sans);
    font-size: 8.5px;
    font-weight: 700;
    letter-spacing: 0.1em;
    color: var(--text-faint);
    align-self: center;
  }
  .meta dd {
    font-size: 10.5px;
    color: var(--text);
    line-height: 1.3;
    word-break: break-word;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .frow {
    display: grid;
    grid-template-columns: 22px 1fr 1fr 44px;
    align-items: center;
    gap: 8px;
    background: transparent;
    border: none;
    border-left: 2px solid transparent;
    padding: 1px 2px 1px 4px;
    cursor: pointer;
    text-align: left;
  }
  .frow:hover {
    background: var(--bg-elev);
  }
  .frow.sel {
    border-left-color: var(--amber);
    background: var(--bg-sel);
  }
  .fr-rank {
    color: var(--text-faint);
    font-size: 11px;
    text-align: right;
  }
  .fr-rank.gold {
    color: var(--gold);
    font-weight: 700;
  }
  .fr-name {
    color: var(--text-dim);
    font-size: 11px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .fr-val {
    text-align: right;
    font-size: 11.5px;
    color: var(--text);
  }
  .field-key {
    display: flex;
    gap: 14px;
    margin-top: 4px;
    font-size: 9px;
    color: var(--text-faint);
    letter-spacing: 0.05em;
  }
  .field-key i {
    display: inline-block;
    width: 14px;
    height: 7px;
    vertical-align: middle;
    margin-right: 4px;
  }
  .bar-k {
    background: var(--amber-dim);
  }
  .mark-k {
    width: 2px !important;
    height: 10px !important;
    background: var(--text);
  }

  .matrix {
    display: flex;
    flex-direction: column;
    gap: 2px;
    font-size: 9px;
  }
  .mx-head,
  .mx-row {
    display: grid;
    grid-template-columns: 86px repeat(6, 1fr);
    gap: 2px;
    align-items: center;
  }
  .mx-head {
    position: sticky;
    top: 0;
    z-index: 1;
    background: var(--bg-panel);
    padding-bottom: 2px;
  }
  .mx-col {
    text-align: center;
    font-family: var(--sans);
    font-weight: 700;
    letter-spacing: 0.03em;
    color: var(--text-faint);
    font-size: 7.5px;
  }
  .mx-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text-dim);
    font-size: 9.5px;
  }
  .mx-cell {
    height: 17px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: #06080a;
    font-weight: 700;
    font-size: 9px;
    border: 1px solid rgba(0, 0, 0, 0.3);
  }

  .acccell {
    display: grid;
    grid-template-columns: 1fr 38px;
    gap: 7px;
    align-items: center;
  }
  .acccell b {
    color: var(--text);
    font-weight: 600;
  }
  td.rank {
    text-align: right;
    color: var(--text-faint);
  }
  td.rank.gold {
    color: var(--gold);
    font-weight: 700;
  }
  .runlink {
    background: none;
    border: none;
    color: var(--text);
    font-weight: 600;
    cursor: pointer;
    padding: 0;
    font-size: 12px;
  }
  .runlink:hover {
    color: var(--amber);
    text-decoration: underline;
  }
  .cfg {
    display: block;
    color: var(--text-faint);
    font-size: 9.5px;
    margin-top: 1px;
  }
  .dim {
    color: var(--text-dim);
  }
  .faint {
    color: var(--text-faint);
  }

  .rail-empty {
    color: var(--text-dim);
    font-size: 11px;
    line-height: 1.6;
  }
  .cmp-legend {
    display: flex;
    flex-direction: column;
    gap: 3px;
    margin-bottom: 8px;
  }
  .cl {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 10.5px;
    color: var(--text-dim);
  }
  .cl i {
    width: 9px;
    height: 9px;
  }
  .radar-wrap {
    display: flex;
    justify-content: center;
    padding: 4px 0 8px;
  }
  .cmp-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 10.5px;
  }
  .cmp-table th,
  .cmp-table td {
    padding: 3px 5px;
    border-bottom: 1px solid var(--border);
    text-align: right;
  }
  .cmp-table th:first-child,
  .cmp-table td:first-child {
    text-align: left;
    color: var(--text-faint);
    font-family: var(--sans);
    font-size: 8.5px;
    font-weight: 700;
    letter-spacing: 0.08em;
  }
  .cmp-table th {
    font-family: var(--sans);
    font-size: 8.5px;
    letter-spacing: 0.02em;
  }
  .rail-sub {
    margin: 12px 0 6px;
  }
  .empty {
    padding: 40px;
    text-align: center;
    color: var(--text-faint);
    letter-spacing: 0.2em;
    font-size: 12px;
  }
</style>
