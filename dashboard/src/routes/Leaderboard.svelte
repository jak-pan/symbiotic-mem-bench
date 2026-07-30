<script lang="ts">
  import { api } from "../lib/api";
  import { router } from "../lib/router.svelte";
  import { store } from "../lib/store.svelte";
  import {
    QTYPES,
    type Cohort,
    type GateFailure,
    type LeaderboardSnapshot,
    type RankedRow,
    type RowVerification,
    type SnapshotRankedRow,
    type UnrankedRecord,
  } from "../lib/types";
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
  import { trialBadge, runKindLabel, runKindChipClass } from "../lib/run";
  import LeaderboardLanding from "../components/LeaderboardLanding.svelte";

  let cohorts = $state<Cohort[]>([]);
  // Records held back from ranking. Present in both modes — the live API and
  // the static export apply the same review gate.
  let unranked = $state<UnrankedRecord[]>([]);
  // Non-null when the page is serving the bundled `membench.leaderboard.v1`
  // export because no /api backend is present (a static deploy).
  let snapshot = $state<LeaderboardSnapshot | null>(null);
  let loadError = $state<string | null>(null);
  let activeId = $state<string>("");
  let loading = $state(true);
  let sortKey = $state<string>("rank");
  let sortDir = $state<1 | -1>(1);
  let selected = $state<string[]>([]);
  let genericOpen = $state(true);
  let comparisonOpen = $state(false);
  let sectionMode = $state<"generic" | "comparison">("generic");

  const SEL_COLORS = ["var(--amber)", "var(--cyan)", "var(--green)", "var(--violet)"];

  // The store decides how this page is being served (live backend vs bundled
  // static export); the board reads from whichever source that mode has. A
  // static deploy is a supported mode, not a failed live load, so nothing here
  // treats the absence of /api as an error. Mode is resolved asynchronously at
  // boot, so this reacts to it rather than sampling it once on mount.
  let loadedFor = $state<string | null>(null);
  $effect(() => {
    const mode = store.mode;
    if (mode === "boot" || loadedFor === mode) return;
    loadedFor = mode;
    void loadBoard(mode);
  });

  async function loadBoard(mode: string) {
    loadError = null;
    if (mode === "snapshot") {
      snapshot = store.snapshot;
      cohorts = snapshot?.cohorts ?? [];
      unranked = snapshot?.unranked ?? [];
    } else if (mode === "live") {
      snapshot = null;
      try {
        const view = await api.leaderboard();
        cohorts = view.cohorts;
        unranked = view.unranked;
      } catch (error) {
        // A failed live request is not an empty leaderboard. Keep the failure
        // explicit so zeroes and trust claims cannot be inferred from missing
        // data.
        cohorts = [];
        unranked = [];
        loadError = (error as Error).message || "leaderboard request failed";
      }
    }
    if (cohorts.length && !cohorts.some((c) => c.cohort_id === activeId)) {
      activeId = cohorts[0].cohort_id;
    }
    loading = false;
  }

  function verification(r: RankedRow): RowVerification | null {
    return (r as SnapshotRankedRow).verification ?? r.eligibility ?? null;
  }

  function reviewTitle(r: RankedRow): string {
    const review = verification(r)?.review;
    if (!review) return "Passed every review gate.";
    return `Reviewed by ${review.reviewer} on ${review.reviewed_at}${
      review.reviewed_commit ? ` (${review.reviewed_commit})` : ""
    }; scoring artifacts still hash to what was reviewed.`;
  }

  function reasonHelp(reason: string): string {
    switch (reason) {
      case "meta-record":
        return "Dashboard-safe rollup: timing/trace evidence is kept, but question-level scoring artifacts were deliberately omitted, so the score cannot be independently verified.";
      case "unscored":
        return "The record has no accuracy metric in its report.";
      case "gate-failed":
        return "The record did not pass every condition of the published review gate (docs/longmemeval-methodology.md).";
      default:
        return reason;
    }
  }

  /** True when another cohort shares this benchmark, size and judge — then the
   *  question-set fingerprint is what tells them apart. */
  function judgeIsAmbiguous(cohort: Cohort): boolean {
    return cohorts.some(
      (other) =>
        other.cohort_id !== cohort.cohort_id &&
        other.benchmark === cohort.benchmark &&
        other.limit === cohort.limit &&
        other.judge_model === cohort.judge_model &&
        other.judge_prompt_mode === cohort.judge_prompt_mode,
    );
  }

  function gateSummary(gates: GateFailure[]): string {
    return gates.map((g) => `${g.gate}: ${g.detail}`).join("\n");
  }

  const active = $derived(cohorts.find((c) => c.cohort_id === activeId));
  const hasLatency = $derived(active?.rows.some((r) => r.latency_ms_p50 != null) ?? false);

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
    <LeaderboardLanding {cohorts} {unranked} {snapshot} {loadError} />
    {#if snapshot}
      <div class="snapshot-note">
        <b>STATIC SNAPSHOT</b> — this deployment has no registry backend by
        design. Showing the committed <code>{snapshot.schema}</code> export of
        tracked records (<code>{snapshot.source.records_root}</code>,
        {snapshot.source.run_count} run{snapshot.source.run_count === 1 ? "" : "s"} scanned,
        {snapshot.source.ranked_count} ranked, {snapshot.source.unranked_count} unranked,
        generated {snapshot.generated_at}).
        {#if snapshot.source.records_digest}
          Records digest <code
            title="SHA-256 over every file in the exported records tree. Recompute it from a checkout to prove this document describes those records."
          >{shortHash(snapshot.source.records_digest, 12)}</code>
          (exporter <code>{shortHash(snapshot.source.git_sha, 12)}</code>).
        {/if}
        {#if snapshot.source.contains_fixtures}
          <b class="fixture-warn">Contains synthetic fixtures — not measured results.</b>
        {/if}
        Methodology: <code>{snapshot.methodology}</code>.
      </div>
    {/if}
    <!-- cohort selector -->
    <div class="cohort-strip">
      <span class="label" style="margin-right:4px">COHORT</span>
      {#each cohorts as c (c.cohort_id)}
        <!-- Benchmark and size alone do not identify a board: two cohorts can
             share both and still be incomparable. Show the judge that separates
             them, and the question set when even the judge matches. -->
        <button
          class="cohort-chip"
          class:on={c.cohort_id === activeId}
          title={c.cohort_id}
          onclick={() => { activeId = c.cohort_id; selected = []; }}
        >
          <span class="cb">{c.benchmark}</span>
          <span class="cs">·{c.limit ?? "?"}Q</span>
          <span class="cj">{c.judge_model ?? "judge?"}{c.judge_prompt_mode ? `/${c.judge_prompt_mode}` : ""}</span>
          {#if judgeIsAmbiguous(c)}
            <span class="cj mono-num">{shortHash(c.dataset_fingerprint, 8)}</span>
          {/if}
          <span class="cn">{c.run_count}</span>
          {#if !c.strictly_comparable}<span class="warn" title="cohort identity incomplete">⚠</span>{/if}
        </button>
      {/each}
    </div>

    {#if loading}
      <div class="empty">SCANNING REGISTRY…</div>
    {:else if loadError}
      <div class="empty error-state">
        LEADERBOARD UNAVAILABLE<br />
        <span class="empty-detail">
          The live registry could not provide ranking data. No zero counts or
          review claims are being inferred from this failed request.
        </span>
      </div>
    {:else if !active}
      <div class="empty">
        NO VERIFIED COHORTS<br />
        <span class="empty-detail">
          No record passed every condition of the review gate, so there is
          nothing to rank. Records excluded from ranking are listed below with
          the gates they failed — their reported scores are not promoted into a
          ranking.
        </span>
      </div>
    {:else}
      {#if !active.strictly_comparable}
        <div class="integrity">
          ⚠ COHORT IDENTITY INCOMPLETE — this cohort does not record all of
          question set, judge model and judge prompt mode, so its rows cannot be
          asserted comparable.
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
                <dt>JUDGE</dt><dd>{active.judge_model ?? "—"}</dd>
                <dt>RUBRIC</dt><dd>{active.judge_prompt_mode ?? "—"}</dd>
                <dt>QSET</dt><dd class="mono-num">{shortHash(active.dataset_fingerprint, 12)}</dd>
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
                    <span class="metric-fill" style="width:{Math.max(0, Math.min(1, r.accuracy ?? 0)) * 100}%"></span>
                    <span class="metric-mark" style="left:{Math.max(0, Math.min(1, r.task_averaged_accuracy ?? 0)) * 100}%"></span>
                    <span class="metric-label mono-num">{pct(r.accuracy)}</span>
                  </span>
                  {#each QTYPES as qt (qt)}
                    {@const s = r.per_question_type?.[qt]}
                    <span class="mx-cell" style="background:{s ? heatColor(s.accuracy) : 'var(--bg-elev)'}"
                      title={s ? `${qtypeShort(qt)} ${pct(s.accuracy)}% (${s.correct}/${s.total})` : 'no data'}>
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

      {#snippet sortableTh(key: string, label: string, cls: string)}
        <th class={`sortable ${cls}`} scope="col" tabindex="0" onclick={() => setSort(key)} onkeydown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); setSort(key); } }}>{label}</th>
      {/snippet}

      <!-- ranked table -->
      <Panel title="Leaderboard" tag="{active.benchmark} · {active.limit}Q" flush scroll>
        <table class="grid leaderboard-table" class:withLatency={hasLatency}>
          <thead>
            <tr>
              {@render sortableTh("rank", "#", "col-rank")}
              {@render sortableTh("run", "System / Config", "col-system")}
              <th class="col-kind" scope="col">Kind</th>
              {@render sortableTh("acc", "Accuracy", "num col-accuracy")}
              {@render sortableTh("tavg", "Task·Avg", "num col-small")}
              {@render sortableTh("abst", "Abst", "num col-small")}
              <th class="col-categories" scope="col">Categories</th>
              {@render sortableTh("cost", "Cost", "num col-money")}
              {#if hasLatency}
                {@render sortableTh("lat", "P50 Lat", "num col-latency")}
              {/if}
              {@render sortableTh("age", "Updated", "num col-updated")}
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
                  {#if r.is_meta_record}
                    <span
                      class="verif-badge"
                      title="Meta record: dashboard-safe rollup without question-level scoring artifacts. Its score cannot be independently verified and is excluded from published leaderboard exports."
                    >META</span>
                  {/if}
                  {#if r.fixture}
                    <span
                      class="verif-badge fixture"
                      title="Synthetic contract fixture. Not a measured benchmark result."
                    >FIXTURE</span>
                  {/if}
                  {#if verification(r)?.level === "verified"}
                    <span class="verif-badge verified" title={reviewTitle(r)}>VERIFIED</span>
                  {/if}
                  <span class="cfg">{r.config_label}</span>
                </td>
                <td><span class="chip {runKindChipClass(r.run_kind)}">{runKindLabel(r.run_kind)}</span></td>
                <td class="num">
                  <span class="metric-bar table-bar" class:lead={r.rank === 1}>
                    <span class="metric-fill" style="width:{Math.max(0, Math.min(1, r.accuracy ?? 0)) * 100}%"></span>
                    <span class="metric-mark" style="left:{Math.max(0, Math.min(1, r.task_averaged_accuracy ?? 0)) * 100}%"></span>
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

    {#if unranked.length}
      <Panel title="Unranked Records" tag="excluded from ranking" flush scroll>
        <table class="grid unranked-table">
          <thead>
            <tr>
              <th scope="col">Record</th>
              <th scope="col">Benchmark</th>
              <th scope="col">Why unranked</th>
              <th class="num" scope="col">Reported score</th>
            </tr>
          </thead>
          <tbody>
            {#each unranked as u (u.run_id)}
              <tr>
                <td class="system-cell">
                  <span class="unranked-name" title={u.run_id}>{u.run_name}</span>
                  {#if u.fixture}<span class="verif-badge fixture">FIXTURE</span>{/if}
                </td>
                <td class="dim">{u.benchmark}{u.limit != null ? ` · ${u.limit}Q` : ""}</td>
                <td>
                  <span class="reason-badge" title={reasonHelp(u.reason)}>{u.reason.toUpperCase()}</span>
                  {#if u.failed_gates?.length}
                    <span class="gate-list" title={gateSummary(u.failed_gates)}>
                      {#each u.failed_gates as g (g.gate)}<span class="gate-chip">{g.gate}</span>{/each}
                    </span>
                  {/if}
                </td>
                <td class="num mono-num dim">
                  {#if u.accuracy != null}
                    {pct(u.accuracy)}%{#if u.accuracy_correct != null && u.accuracy_total != null}&nbsp;({u.accuracy_correct}/{u.accuracy_total}){/if}
                    <span class="faint"> unverified</span>
                  {:else}
                    —
                  {/if}
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
        <div class="unranked-footnote">
          Unranked records are excluded from every cohort. A reported score shown
          here comes from the record's own report and cannot be independently
          reproduced from tracked question-level artifacts.
        </div>
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
  .cohort-chip .cj {
    color: var(--text-faint);
    font-size: 9.5px;
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

  .snapshot-note {
    flex: none;
    padding: 7px 10px;
    background: rgba(80, 170, 255, 0.07);
    border: 1px solid var(--cyan);
    color: var(--text-dim);
    font-size: 10.5px;
    line-height: 1.5;
    letter-spacing: 0.02em;
  }
  .snapshot-note b {
    color: var(--cyan);
    letter-spacing: 0.1em;
  }
  .snapshot-note code {
    color: var(--text);
    font-size: 10px;
  }

  .verif-badge {
    display: inline-block;
    margin-left: 6px;
    padding: 1px 5px;
    border: 1px solid var(--gold);
    background: rgba(232, 195, 74, 0.08);
    color: var(--gold);
    font-family: var(--sans);
    font-size: 8px;
    font-weight: 700;
    letter-spacing: 0.08em;
    vertical-align: 1px;
  }
  .verif-badge.verified {
    border-color: var(--green-dim);
    background: rgba(47, 207, 122, 0.08);
    color: var(--green);
  }
  .verif-badge.fixture {
    border-color: var(--violet);
    background: rgba(150, 120, 255, 0.08);
    color: var(--violet);
  }
  .fixture-warn {
    color: var(--violet);
  }
  .gate-list {
    display: inline-flex;
    flex-wrap: wrap;
    gap: 3px;
    margin-left: 6px;
  }
  .gate-chip {
    padding: 0 4px;
    border: 1px solid var(--border-bright);
    color: var(--text-faint);
    font-family: var(--mono);
    font-size: 8.5px;
    letter-spacing: 0.04em;
  }

  .unranked-table {
    table-layout: fixed;
    min-width: 640px;
  }
  .unranked-table th,
  .unranked-table td {
    padding-left: 8px;
    padding-right: 8px;
  }
  .unranked-name {
    display: block;
    color: var(--text);
    font-weight: 600;
    font-size: 11.5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .reason-badge {
    display: inline-block;
    padding: 1px 6px;
    border: 1px solid var(--border-bright);
    background: var(--bg-elev);
    color: var(--text-dim);
    font-family: var(--sans);
    font-size: 8px;
    font-weight: 700;
    letter-spacing: 0.08em;
  }
  .unranked-footnote {
    padding: 6px 10px;
    color: var(--text-faint);
    font-size: 9.5px;
    line-height: 1.5;
  }

  .empty-detail {
    display: inline-block;
    margin-top: 8px;
    max-width: 520px;
    color: var(--text-dim);
    font-size: 10.5px;
    letter-spacing: 0.02em;
    line-height: 1.6;
    text-transform: none;
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
