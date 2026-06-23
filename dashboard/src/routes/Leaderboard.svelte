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
  } from "../lib/format";
  import Panel from "../components/Panel.svelte";
  import CategoryHeat from "../components/CategoryHeat.svelte";
  import RingGauge from "../components/RingGauge.svelte";
  import Radar from "../components/Radar.svelte";

  let cohorts = $state<Cohort[]>([]);
  let activeId = $state<string>("");
  let loading = $state(true);
  let sortKey = $state<string>("rank");
  let sortDir = $state<1 | -1>(1);
  let selected = $state<string[]>([]);
  let genericOpen = $state(true);
  let comparisonOpen = $state(false);
  let sectionMode = $state<"generic" | "comparison">("generic");

  const SEL_COLORS = ["var(--amber)", "var(--cyan)", "var(--green)", "var(--violet)"];

  onMount(async () => {
    cohorts = await api.leaderboard();
    if (cohorts.length) activeId = cohorts[0].cohort_id;
    loading = false;
  });

  const active = $derived(cohorts.find((c) => c.cohort_id === activeId));
  const hasLatency = $derived(active?.rows.some((r) => r.latency_ms_p50 != null) ?? false);
  const visibleCompareMetrics = $derived(
    COMPARE_METRICS.filter((metric) => metric.key !== "lat" || hasLatency),
  );

  $effect(() => {
    const nextMode = selected.length >= 2 ? "comparison" : "generic";
    if (nextMode !== sectionMode) {
      sectionMode = nextMode;
      genericOpen = nextMode === "generic";
      comparisonOpen = nextMode === "comparison";
    }
  });

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

  function trialBadge(r: RankedRow): string {
    return r.trial_markers.some((marker) => marker.focused) ? "FOCUSED" : "TRIAL";
  }

  type CompareMetric = {
    key: string;
    label: string;
    kind: "ratio" | "money" | "ms";
    qtype?: string;
  };

  const CORE_METRICS: CompareMetric[] = [
    { key: "acc", label: "ACC", kind: "ratio" },
    { key: "tavg", label: "T·AVG", kind: "ratio" },
    { key: "abst", label: "ABST", kind: "ratio" },
    { key: "cost", label: "COST", kind: "money" },
    { key: "lat", label: "LAT", kind: "ms" },
  ];
  const COMPARE_METRICS: CompareMetric[] = [
    ...CORE_METRICS,
    ...QTYPES.map((qtype) => ({
      key: `qtype:${qtype}`,
      label: qtypeShort(qtype),
      kind: "ratio" as const,
      qtype,
    })),
  ];

  // Resolve the selected ids to rows (called inline in the template so it stays
  // reactive to `selected`).
  function pickRows(ids: string[], c: Cohort | undefined): RankedRow[] {
    if (!c) return [];
    return ids
      .map((id) => c.rows.find((r) => r.run_id === id))
      .filter((r): r is RankedRow => !!r);
  }

  function metricValue(metric: CompareMetric, row: RankedRow): number | null {
    if (metric.qtype) return row.per_question_type?.[metric.qtype]?.accuracy ?? null;
    switch (metric.key) {
      case "acc": return row.accuracy;
      case "tavg": return row.task_averaged_accuracy;
      case "abst": return row.abstention_accuracy;
      case "cost": return row.cost_micro_usd;
      case "lat": return row.latency_ms_p50;
      default: return null;
    }
  }

  function metricDisplay(metric: CompareMetric, value: number | null): string {
    if (value == null) return "—";
    if (metric.kind === "money") return money(value);
    if (metric.kind === "ms") return ms(value);
    return pct(value);
  }

  function metricDelta(metric: CompareMetric, value: number | null, leader: RankedRow | undefined): number | null {
    const baseline = leader ? metricValue(metric, leader) : null;
    if (value == null || baseline == null) return null;
    if (metric.kind === "ratio") return value - baseline;
    if (baseline === 0) return null;
    return (value - baseline) / baseline;
  }

  function deltaTone(metric: CompareMetric, delta: number | null): string {
    if (delta == null || Math.abs(delta) < 1e-9) return "flat";
    if (metric.kind === "money" || metric.kind === "ms") return delta < 0 ? "up" : "down";
    return delta > 0 ? "up" : "down";
  }

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

      <section class="fold" class:open={genericOpen}>
        <button class="fold-head" onclick={() => genericOpen = !genericOpen}>
          <span>{genericOpen ? "▾" : "▸"}</span>
          <b>Leaderboard</b>
          <i>{active.benchmark} · {active.limit}Q · {active.run_count} systems</i>
        </button>
        {#if genericOpen}
          <div class="hero">
          <Panel title="Cohort" tag={active.benchmark}>
            <div class="cohort-hero">
              <RingGauge value={active.best_accuracy} label="peak acc" color="var(--amber)" size={96} />
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

          <Panel title="Field Ranking" tag="overall + question type" scroll>
            <div class="field-matrix">
              <div class="fm-head">
                <span>#</span>
                <span>System</span>
                <span>Accuracy</span>
                {#each QTYPES as qt (qt)}<span class="fm-col" title={qt}>{qtypeAbbr(qt)}</span>{/each}
              </div>
              {#each active.rows as r, i (r.run_id)}
                <button class="fm-row" onclick={() => toggle(r.run_id)} class:sel={selected.includes(r.run_id)}>
                  <span class="fr-rank" class:gold={i === 0}>{r.rank}</span>
                  <span class="fr-name" title={r.run_name}>{r.run_name}</span>
                  <span class="metric-bar" class:lead={i === 0}>
                    <span class="metric-fill" style="width:{Math.max(0, Math.min(1, r.accuracy)) * 100}%"></span>
                    <span class="metric-mark" style="left:{Math.max(0, Math.min(1, r.task_averaged_accuracy)) * 100}%"></span>
                    <span class="metric-label mono-num">{pct(r.accuracy)}</span>
                  </span>
                  {#each QTYPES as qt (qt)}
                    {@const s = r.per_question_type?.[qt]}
                    <span class="mx-cell" style="background:{s ? heatColor(s.accuracy) : 'var(--bg-elev)'}"
                      title={s ? `${qtypeShort(qt)} ${pct(s.accuracy)}% (${s.correct}/${s.n})` : 'no data'}>
                      {s ? Math.round(s.accuracy * 100) : ""}
                    </span>
                  {/each}
                </button>
              {/each}
              <div class="field-key">
                <span><i class="bar-k"></i> overall</span>
                <span><i class="mark-k"></i> task-avg marker</span>
              </div>
            </div>
          </Panel>
          </div>
        {/if}
      </section>

      <section class="fold" class:open={comparisonOpen}>
        <button class="fold-head" onclick={() => comparisonOpen = !comparisonOpen}>
          <span>{comparisonOpen ? "▾" : "▸"}</span>
          <b>Comparison</b>
          <i>{selected.length >= 2 ? `${selected.length} selected` : "select 2+ rows"}</i>
        </button>
        {#if comparisonOpen}
          {#if selected.length >= 2}
            {@const srows = pickRows(selected, active)}
            {@const leader = active.rows[0]}
            <div class="hero compare-hero">
              <Panel title="Comparison" tag={`${srows.length} selected`}>
                <div class="cmp-merged">
                  <div class="cmp-chart">
                    <div class="radar-wrap main-radar">
                      <Radar
                        size={188}
                        axes={QTYPES.map((q) => qtypeAbbr(q))}
                        series={srows.map((r, i) => ({ label: r.run_name, color: SEL_COLORS[i], values: radarValues(r) }))}
                      />
                    </div>
                  </div>

                  <div class="cmp-metrics">
                    <table class="cmp-table cmp-table-main">
                      <thead>
                        <tr>
                          <th>System</th>
                          {#each visibleCompareMetrics as metric (metric.key)}
                            <th>{metric.label}</th>
                          {/each}
                        </tr>
                      </thead>
                      <tbody>
                        {#each srows as r, i (r.run_id)}
                          <tr>
                            <td>
                              <span class="cmp-system">
                                <i style="background:{SEL_COLORS[i]}"></i>
                                <span title={r.run_name}>{r.run_name}</span>
                              </span>
                            </td>
                            {#each visibleCompareMetrics as metric (metric.key)}
                              {@const value = metricValue(metric, r)}
                              {@const delta = metricDelta(metric, value, leader)}
                              <td>
                                <span class="metric-pack">
                                  <span class="metric-val mono-num">{metricDisplay(metric, value)}</span>
                                {#if delta != null && r.run_id !== leader?.run_id}
                                  <span class="metric-delta mono-num {deltaTone(metric, delta)}">{pctSign(delta)}</span>
                                {/if}
                                </span>
                              </td>
                            {/each}
                          </tr>
                        {/each}
                      </tbody>
                    </table>
                  </div>
                </div>
              </Panel>
            </div>
          {:else}
            <div class="section-empty">Select two to four leaderboard rows to expand the comparison workspace.</div>
          {/if}
        {/if}
      </section>

      <!-- ranked table -->
      <Panel title="Leaderboard" tag="{active.benchmark} · {active.limit}Q" flush scroll>
        <table class="grid leaderboard-table" class:withLatency={hasLatency}>
          <thead>
            <tr>
              <th class="sortable col-rank" onclick={() => setSort("rank")}>#</th>
              <th class="sortable col-system" onclick={() => setSort("run")}>System / Config</th>
              <th class="col-kind">Kind</th>
              <th class="sortable num col-accuracy" onclick={() => setSort("acc")}>Accuracy</th>
              <th class="sortable num col-small" onclick={() => setSort("tavg")}>Task·Avg</th>
              <th class="sortable num col-small" onclick={() => setSort("abst")}>Abst</th>
              <th class="col-categories">Categories</th>
              <th class="sortable num col-money" onclick={() => setSort("cost")}>Cost</th>
              {#if hasLatency}
                <th class="sortable num col-latency" onclick={() => setSort("lat")}>P50 Lat</th>
              {/if}
              <th class="sortable num col-updated" onclick={() => setSort("age")}>Updated</th>
            </tr>
          </thead>
          <tbody>
            {#each rows as r (r.run_id)}
              <tr class:selected={selected.includes(r.run_id)} onclick={() => toggle(r.run_id)}>
                <td class="rank" class:gold={r.rank === 1}>
                  {#if selected.includes(r.run_id)}<span style="color:{colorFor(r.run_id)}">◆</span>{:else}{r.rank}{/if}
                </td>
                <td class="system-cell">
                  <button class="runlink" onclick={(e) => { e.stopPropagation(); router.openRun(r.run_id); }}>{r.run_name}</button>
                  {#if r.is_trial_run}<span class="trial-badge">{trialBadge(r)}</span>{/if}
                  <span class="cfg">{r.config_label}</span>
                </td>
                <td><span class="chip {r.run_kind === 'native' ? 'green' : 'cyan'}">{r.run_kind === "imported-artifact" ? "import" : r.run_kind}</span></td>
                <td class="num">
                  <span class="metric-bar table-bar" class:lead={r.rank === 1}>
                    <span class="metric-fill" style="width:{Math.max(0, Math.min(1, r.accuracy)) * 100}%"></span>
                    <span class="metric-mark" style="left:{Math.max(0, Math.min(1, r.task_averaged_accuracy)) * 100}%"></span>
                    <span class="metric-label mono-num">{pct(r.accuracy)}</span>
                  </span>
                </td>
                <td class="num mono-num dim">{pct(r.task_averaged_accuracy)}</td>
                <td class="num mono-num dim">{pct(r.abstention_accuracy)}</td>
                <td><CategoryHeat scores={r.per_question_type} /></td>
                <td class="num mono-num dim">{money(r.cost_micro_usd)}</td>
                {#if hasLatency}
                  <td class="num mono-num dim">{ms(r.latency_ms_p50)}</td>
                {/if}
                <td class="num mono-num faint">{ago(r.modified_ms)}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </Panel>
    {/if}
  </div>
</div>

<style>
  .lb {
    flex: 1;
    display: flex;
    background: var(--bg);
    min-height: 0;
  }
  .main {
    background: var(--bg);
    overflow: hidden;
    padding: 10px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    flex: 1;
    min-width: 0;
  }
  /* The leaderboard table panel (direct child of .main, outside .hero) fills the
     remaining height and scrolls internally so the hero stays fixed. */
  .main > :global(.panel) {
    flex: 1;
    min-height: 0;
  }
  .cohort-strip {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
    padding-right: 2px;
  }
  .cohort-chip {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 9px;
    background: var(--bg-panel);
    border: 1px solid var(--border-bright);
    color: var(--text-dim);
    cursor: pointer;
    font-size: 10.5px;
    min-width: 0;
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
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
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

  .fold {
    border: 1px solid var(--border);
    background: var(--bg-panel);
    min-width: 0;
  }
  .fold.open {
    background: transparent;
  }
  .fold-head {
    width: 100%;
    min-height: 30px;
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 6px 10px;
    border: none;
    border-bottom: 1px solid transparent;
    background: linear-gradient(var(--bg-elev), var(--bg-panel));
    color: var(--text-dim);
    text-align: left;
    cursor: pointer;
  }
  .fold.open .fold-head {
    border-bottom-color: var(--border);
  }
  .fold-head span {
    color: var(--amber);
    font-size: 11px;
  }
  .fold-head b {
    font-family: var(--sans);
    font-size: 10.5px;
    font-weight: 800;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--text);
  }
  .fold-head i {
    margin-left: auto;
    color: var(--amber);
    font-family: var(--sans);
    font-size: 10px;
    font-style: normal;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }

  .hero {
    display: grid;
    grid-template-columns: 236px minmax(0, 1fr);
    gap: 10px;
    height: 236px;
    flex: none;
    padding: 10px;
  }
  .hero :global(.panel) {
    min-height: 0;
  }
  .compare-hero {
    grid-template-columns: minmax(0, 1fr);
    height: 248px;
  }
  .compare-hero :global(.panel) {
    min-height: 0;
  }

  .cohort-hero {
    display: grid;
    gap: 10px;
    justify-items: center;
    align-content: start;
    padding: 4px 2px 0;
  }
  .meta {
    display: grid;
    grid-template-columns: 72px minmax(0, 1fr);
    gap: 5px 10px;
    width: 100%;
    align-content: start;
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
    font-size: 10px;
    color: var(--text);
    line-height: 1.3;
    word-break: break-word;
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
    font-size: 10.5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .field-matrix {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .fm-head,
  .fm-row {
    display: grid;
    grid-template-columns:
      20px minmax(128px, 1.1fr) minmax(78px, 0.62fr)
      repeat(6, minmax(28px, 0.42fr));
    gap: 3px;
    align-items: center;
  }
  .fm-head {
    position: sticky;
    top: 0;
    z-index: 1;
    background: var(--bg-panel);
    padding: 0 3px 2px;
    color: var(--text-faint);
    font-family: var(--sans);
    font-size: 8px;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }
  .fm-col {
    text-align: center;
  }
  .fm-row {
    width: 100%;
    background: transparent;
    border: none;
    border-left: 2px solid transparent;
    padding: 0 3px;
    cursor: pointer;
    text-align: left;
  }
  .fm-row:hover {
    background: var(--bg-elev);
  }
  .fm-row.sel {
    border-left-color: var(--amber);
    background: var(--bg-sel);
  }
  .field-key {
    display: flex;
    gap: 14px;
    margin-top: 2px;
    font-size: 8.5px;
    color: var(--text-faint);
    letter-spacing: 0.05em;
  }
  .field-key i {
    display: inline-block;
    width: 12px;
    height: 6px;
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

  .mx-cell {
    height: 16px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: #06080a;
    font-weight: 700;
    font-size: 8.5px;
    border: 1px solid rgba(0, 0, 0, 0.3);
  }

  .metric-bar {
    position: relative;
    display: block;
    width: 100%;
    min-width: 76px;
    height: 16px;
    overflow: hidden;
    border: 1px solid var(--border);
    background: var(--bg-elev);
  }
  .metric-fill {
    position: absolute;
    inset: 0 auto 0 0;
    background: var(--amber-dim);
  }
  .metric-bar.lead .metric-fill {
    background: var(--amber);
  }
  .metric-mark {
    position: absolute;
    top: -1px;
    bottom: -1px;
    width: 2px;
    background: var(--text);
    box-shadow: 0 0 2px #000;
  }
  .metric-label {
    position: relative;
    z-index: 1;
    display: block;
    height: 100%;
    padding: 0 4px;
    color: var(--text);
    font-size: 10px;
    font-weight: 700;
    line-height: 14px;
    text-align: left;
    text-shadow: 0 1px 2px #000;
  }
  .table-bar {
    height: 18px;
    min-width: 88px;
  }
  .table-bar .metric-label {
    line-height: 16px;
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
    display: block;
    width: 100%;
    max-width: 100%;
    background: none;
    border: none;
    color: var(--text);
    font-weight: 600;
    cursor: pointer;
    padding: 0;
    font-size: 12px;
    overflow: hidden;
    text-align: left;
    text-overflow: ellipsis;
    white-space: nowrap;
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
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .system-cell {
    min-width: 0;
    overflow: hidden;
  }
  .trial-badge {
    display: inline-block;
    margin-left: 6px;
    padding: 1px 5px;
    border: 1px solid var(--amber-dim);
    background: rgba(255, 165, 36, 0.08);
    color: var(--amber);
    font-family: var(--sans);
    font-size: 8px;
    font-weight: 700;
    letter-spacing: 0.08em;
    vertical-align: 1px;
  }
  .dim {
    color: var(--text-dim);
  }
  .faint {
    color: var(--text-faint);
  }

  .leaderboard-table {
    table-layout: fixed;
    min-width: 980px;
  }
  .leaderboard-table.withLatency {
    min-width: 1052px;
  }
  .leaderboard-table th,
  .leaderboard-table td {
    padding-left: 8px;
    padding-right: 8px;
  }
  .col-rank,
  .leaderboard-table td:nth-child(1) {
    width: 34px;
  }
  .col-system,
  .leaderboard-table td:nth-child(2) {
    width: 270px;
    overflow: hidden;
  }
  .col-kind,
  .leaderboard-table td:nth-child(3) {
    width: 76px;
  }
  .col-accuracy,
  .leaderboard-table td:nth-child(4) {
    width: 112px;
  }
  .col-small,
  .leaderboard-table td:nth-child(5),
  .leaderboard-table td:nth-child(6) {
    width: 76px;
  }
  .col-categories,
  .leaderboard-table td:nth-child(7) {
    width: 108px;
  }
  .col-money,
  .leaderboard-table td:nth-child(8),
  .col-latency,
  .leaderboard-table.withLatency td:nth-last-child(2),
  .col-updated,
  .leaderboard-table td:last-child {
    width: 72px;
  }

  .section-empty {
    padding: 18px;
    color: var(--text-dim);
    font-size: 11px;
    line-height: 1.6;
    background: var(--bg-panel);
  }
  .radar-wrap {
    display: flex;
    justify-content: center;
    padding: 4px 0 8px;
  }
  .cmp-merged {
    display: grid;
    grid-template-columns: 232px minmax(0, 1fr);
    gap: 14px;
    min-height: 0;
    height: 100%;
  }
  .cmp-chart {
    display: flex;
    align-items: center;
    justify-content: center;
    min-width: 0;
  }
  .cmp-metrics {
    min-width: 0;
    overflow: auto;
  }
  .main-radar {
    padding: 0;
  }
  .cmp-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 10px;
  }
  .cmp-table-main {
    table-layout: auto;
    min-width: 820px;
  }
  .cmp-table th,
  .cmp-table td {
    padding: 4px 5px;
    border-bottom: 1px solid var(--border);
    text-align: right;
    vertical-align: middle;
  }
  .cmp-table-main th,
  .cmp-system span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .cmp-table th:first-child,
  .cmp-table td:first-child {
    text-align: left;
  }
  .cmp-table th:first-child {
    color: var(--text-faint);
    font-family: var(--sans);
    font-size: 8.5px;
    font-weight: 700;
    letter-spacing: 0.08em;
  }
  .cmp-table td:first-child {
    color: var(--text-dim);
  }
  .cmp-table th {
    font-family: var(--sans);
    font-size: 8.5px;
    letter-spacing: 0.02em;
    color: var(--text-faint);
  }
  .cmp-table-main th:first-child,
  .cmp-table-main td:first-child {
    width: 190px;
    min-width: 190px;
    max-width: 190px;
  }
  .cmp-table-main th:not(:first-child),
  .cmp-table-main td:not(:first-child) {
    width: 62px;
    min-width: 56px;
  }
  .cmp-system {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    width: 100%;
    max-width: 180px;
    min-width: 0;
  }
  .cmp-system i {
    flex: 0 0 auto;
    width: 9px;
    height: 9px;
  }
  .cmp-system span {
    display: block;
    min-width: 0;
    max-width: 100%;
  }
  .metric-pack {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    justify-content: flex-end;
    gap: 1px;
    width: 100%;
    white-space: nowrap;
  }
  .metric-delta {
    font-size: 8px;
    color: var(--text-faint);
  }
  .metric-delta.up {
    color: var(--green);
  }
  .metric-delta.down {
    color: var(--red);
  }
  .empty {
    padding: 40px;
    text-align: center;
    color: var(--text-faint);
    letter-spacing: 0.2em;
    font-size: 12px;
  }

  @media (max-width: 1280px) {
    .hero {
      grid-template-columns: 236px minmax(0, 1fr);
      height: auto;
    }
    .hero:not(.compare-hero) > :global(.panel) {
      min-height: 236px;
    }
    .compare-hero {
      grid-template-columns: minmax(0, 1fr);
      height: 248px;
    }
    .compare-hero > :global(.panel) {
      min-height: 0;
    }
    .cmp-merged {
      grid-template-columns: 232px minmax(0, 1fr);
    }
  }
</style>
