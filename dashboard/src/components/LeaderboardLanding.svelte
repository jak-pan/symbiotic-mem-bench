<script lang="ts">
  import type {
    Cohort,
    LeaderboardSnapshot,
    UnrankedRecord,
  } from "../lib/types";

  let {
    cohorts,
    unranked,
    snapshot,
    loadError,
    loading,
  }: {
    cohorts: Cohort[];
    unranked: UnrankedRecord[];
    snapshot: LeaderboardSnapshot | null;
    loadError: string | null;
    loading: boolean;
  } = $props();

  const available = $derived(!loading && loadError === null);
  const ranked = $derived(cohorts.reduce((total, cohort) => total + cohort.rows.length, 0));
  const comparable = $derived(cohorts.filter((cohort) => cohort.strictly_comparable).length);
  const generated = $derived(snapshot?.generated_at.slice(0, 10) ?? "live");
  const sourceLabel = $derived(snapshot?.schema ?? "live registry");
</script>

<section class="landing" aria-labelledby="leaderboard-title">
  <div class="identity">
    <span class="eyebrow">OPEN-SOURCE MEMORY EVALUATION</span>
    <div class="title-row">
      <h1 id="leaderboard-title">Public leaderboard</h1>
      <span
        class="version"
        title="This labels the product interface. It is not a claim of official LongMemEval v2 results."
      >PRODUCT UI V2</span>
    </div>
    <p>
      Reproducible results from tracked artifacts. Only reviewed, comparable
      records receive a rank.
    </p>
  </div>

  <dl class="numbers" aria-label="Leaderboard release status">
    <div>
      <dt>RANKED</dt>
      <dd class:ranked={available}>{available ? ranked : "—"}</dd>
    </div>
    <div>
      <dt>HELD BACK</dt>
      <dd>{available ? unranked.length : "—"}</dd>
    </div>
    <div>
      <dt>STRICT COHORTS</dt>
      <dd>{available ? `${comparable}/${cohorts.length}` : "—"}</dd>
    </div>
    <div>
      <dt>DATA</dt>
      <dd class="data" title={loadError ?? sourceLabel}>
        {available ? generated : loading ? "loading" : "unavailable"}
      </dd>
    </div>
  </dl>

  <div class="trust">
    {#if loading}
      <span><i class="guard"></i> loading leaderboard data — no ranking claims yet</span>
    {:else if available}
      <span><i class="ok"></i> reviewed artifacts</span>
      <span><i class="ok"></i> cohort-locked ranking</span>
      <span title="The experimental LongMemEval v2 text projection is non-official and cannot enter published rankings.">
        <i class="guard"></i> projections never rank
      </span>
    {:else}
      <span class="load-error" title={loadError ?? "leaderboard request failed"}>
        <i class="bad"></i> live leaderboard request failed — no ranking claims shown
      </span>
    {/if}
    <div class="links">
      <a
        href="https://github.com/jak-pan/symbiotic-mem-bench"
        target="_blank"
        rel="noreferrer"
      >SOURCE ↗</a>
      <a
        href="https://github.com/jak-pan/symbiotic-mem-bench/blob/master/docs/longmemeval-methodology.md"
        target="_blank"
        rel="noreferrer"
      >METHODOLOGY ↗</a>
    </div>
  </div>
</section>

<style>
  .landing {
    flex: none;
    min-height: 86px;
    display: grid;
    grid-template-columns: minmax(300px, 1.25fr) minmax(360px, 1fr);
    grid-template-rows: 1fr auto;
    column-gap: 24px;
    padding: 11px 14px 8px;
    overflow: hidden;
    background:
      linear-gradient(100deg, rgba(255, 165, 36, 0.09), transparent 42%),
      var(--bg-panel);
    border: 1px solid var(--border-bright);
  }
  .identity {
    min-width: 0;
  }
  .eyebrow {
    display: block;
    color: var(--amber);
    font-family: var(--sans);
    font-size: 8px;
    font-weight: 800;
    letter-spacing: 0.18em;
  }
  .title-row {
    display: flex;
    align-items: center;
    gap: 9px;
    margin-top: 1px;
  }
  h1 {
    color: var(--text);
    font-size: 21px;
    line-height: 1.05;
    letter-spacing: -0.015em;
  }
  .version {
    padding: 1px 5px;
    border: 1px solid var(--amber-dim);
    color: var(--amber-soft);
    font-family: var(--sans);
    font-size: 7.5px;
    font-weight: 800;
    letter-spacing: 0.1em;
    white-space: nowrap;
  }
  p {
    max-width: 650px;
    margin-top: 4px;
    overflow: hidden;
    color: var(--text-dim);
    font-size: 9.5px;
    line-height: 1.3;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .numbers {
    display: grid;
    grid-template-columns: repeat(4, minmax(70px, 1fr));
    align-items: center;
    min-width: 0;
  }
  .numbers div {
    min-width: 0;
    padding: 2px 10px;
    border-left: 1px solid var(--border);
  }
  dt {
    overflow: hidden;
    color: var(--text-faint);
    font-family: var(--sans);
    font-size: 7.5px;
    font-weight: 800;
    letter-spacing: 0.1em;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  dd {
    overflow: hidden;
    color: var(--text);
    font-size: 17px;
    font-weight: 600;
    line-height: 1.25;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  dd.ranked {
    color: var(--green);
  }
  dd.data {
    font-size: 11px;
  }
  .trust {
    grid-column: 1 / -1;
    display: flex;
    align-items: center;
    gap: 14px;
    min-width: 0;
    padding-top: 5px;
    border-top: 1px solid var(--border);
    color: var(--text-faint);
    font-size: 8.5px;
    letter-spacing: 0.03em;
    white-space: nowrap;
  }
  .trust span {
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
  .trust i {
    width: 5px;
    height: 5px;
    display: inline-block;
    border-radius: 50%;
  }
  .trust .ok {
    background: var(--green);
    box-shadow: 0 0 5px rgba(47, 207, 122, 0.45);
  }
  .trust .guard {
    background: var(--amber);
    box-shadow: 0 0 5px rgba(255, 165, 36, 0.4);
  }
  .trust .bad {
    background: var(--red);
    box-shadow: 0 0 5px rgba(255, 79, 79, 0.45);
  }
  .trust .load-error {
    color: var(--red);
  }
  .links {
    display: flex;
    gap: 12px;
    margin-left: auto;
  }
  a {
    color: var(--cyan);
    font-family: var(--sans);
    font-size: 8px;
    font-weight: 800;
    letter-spacing: 0.09em;
  }
  a:hover {
    color: var(--text);
  }

  @media (max-width: 980px) {
    .landing {
      grid-template-columns: minmax(260px, 1fr) minmax(290px, 1fr);
      column-gap: 12px;
    }
    .trust {
      gap: 8px;
    }
    .trust span:nth-child(2) {
      display: none;
    }
  }
</style>
