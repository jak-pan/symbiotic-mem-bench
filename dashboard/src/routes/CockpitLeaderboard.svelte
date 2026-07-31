<script lang="ts">
  import { store } from "../lib/store.svelte";
  import type { Cohort, RankedRow } from "../lib/types";

  const qtypeLabels: Record<string, string> = {
    "single-session-user": "USR",
    "single-session-preference": "PRF",
    "single-session-assistant": "AST",
    "multi-session": "MLT",
    "knowledge-update": "KUP",
    "temporal-reasoning": "TMP",
  };

  const liveCohorts = $derived.by((): Cohort[] => {
    if (store.snapshot) return store.snapshot.cohorts;
    const groups = new Map<string, RankedRow[]>();
    for (const row of store.runs.filter((run) => run.eligibility?.eligible)) {
      const rows = groups.get(row.cohort_id) ?? [];
      rows.push({ ...row, rank: 0 });
      groups.set(row.cohort_id, rows);
    }
    return [...groups.entries()].map(([cohort_id, rows]) => {
      rows.sort((a, b) => (b.accuracy ?? -1) - (a.accuracy ?? -1));
      rows.forEach((row, index) => row.rank = index + 1);
      const first = rows[0];
      return {
        cohort_id,
        benchmark: first.benchmark,
        limit: first.limit,
        run_count: rows.length,
        dataset_fingerprint: first.dataset_fingerprint,
        judge_model: first.judge_model,
        judge_prompt_mode: first.judge_prompt_mode,
        dataset_fingerprints: [...new Set(rows.map((row) => row.dataset_fingerprint).filter((value): value is string => !!value))],
        judge_models: [...new Set(rows.map((row) => row.judge_model).filter((value): value is string => !!value))],
        judge_prompt_modes: [...new Set(rows.map((row) => row.judge_prompt_mode).filter((value): value is string => !!value))],
        strictly_comparable: true,
        best_accuracy: first.accuracy,
        rows,
      };
    });
  });

  let selectedCohortId = $state("");
  const selected = $derived(liveCohorts.find((cohort) => cohort.cohort_id === selectedCohortId) ?? liveCohorts[0]);
  const rankedCount = $derived(liveCohorts.reduce((sum, cohort) => sum + cohort.rows.length, 0));
  const heldCount = $derived(store.snapshot?.unranked.length ?? store.runs.filter((run) => !run.eligibility?.eligible).length);
  const generated = $derived(!store.loaded ? "—" : store.snapshot?.generated_at.slice(0, 10) ?? "live");

  function pct(value: number | null | undefined) {
    return value == null ? "—" : `${(value * 100).toFixed(1)}%`;
  }

  function money(microUsd: number | null) {
    return microUsd == null ? "—" : `$${(microUsd / 1_000_000).toFixed(2)}`;
  }
</script>

<aside class="rail">
  <div class="rail-title"><span>COHORTS</span><b>{liveCohorts.length}</b></div>
  {#each liveCohorts as cohort}
    <button class="rail-item" class:active={selected?.cohort_id === cohort.cohort_id} onclick={() => selectedCohortId = cohort.cohort_id}>
      <span class="cohort-name">{cohort.benchmark} · {cohort.limit ?? "all"}</span>
      <span class="cohort-meta">{cohort.rows.length} ranked</span>
      <span class="status">{cohort.strictly_comparable ? "STRICT" : "SPLIT"}</span>
    </button>
  {:else}
    <div class="empty-rail">No verified cohort loaded.</div>
  {/each}
  <div class="rail-law">
    <span class="label">RANKING LAW</span>
    <p>Reviewed records rank only inside an exact dataset × limit × judge × prompt-mode cohort.</p>
    <p><b>Projected and incomplete records stay below the rule.</b></p>
  </div>
</aside>

<section class="work">
  <div class="hero">
    <div>
      <span class="eyebrow">OPEN-SOURCE MEMORY EVALUATION</span>
      <h1>Leaderboard cockpit <span>VERIFIED DATA</span></h1>
      <p>Dense comparative view from tracked artifacts. No mock score can enter this build.</p>
    </div>
    <div class="metrics">
      <div><small>RANKED</small><b class="green">{store.loaded ? rankedCount : "—"}</b></div>
      <div><small>HELD BACK</small><b>{store.loaded ? heldCount : "—"}</b></div>
      <div><small>STRICT COHORTS</small><b>{store.loaded ? liveCohorts.filter((c) => c.strictly_comparable).length : "—"}/{store.loaded ? liveCohorts.length : "—"}</b></div>
      <div><small>DATA</small><b class="date">{generated}</b></div>
    </div>
  </div>

  {#if selected}
    <div class="cohort-strip">
      <div><span>COHORT</span><b>{selected.benchmark} × {selected.limit ?? "all"}</b></div>
      <div><span>JUDGE</span><b>{selected.judge_model ?? "unrecorded"}</b></div>
      <div><span>PROMPT MODE</span><b>{selected.judge_prompt_mode ?? "unrecorded"}</b></div>
      <div><span>FINGERPRINT</span><b title={selected.dataset_fingerprint ?? ""}>{selected.dataset_fingerprint?.slice(0, 12) ?? "unrecorded"}</b></div>
      <div class="strict"><span>COMPARABILITY</span><b>✓ STRICT</b></div>
    </div>

    <div class="scroll-area">
      <section class="panel ranking">
        <div class="panel-head">
          <div><span>RANKING</span><b>{selected.rows.length} verified record{selected.rows.length === 1 ? "" : "s"}</b></div>
          <p>Accuracy ranks only inside this exact cohort. Cost and categories are descriptive.</p>
        </div>
        <div class="table-wrap">
          <table>
            <thead><tr>
              <th>#</th><th>SYSTEM / RUN</th><th class="num">ACC</th><th class="num">T-AVG</th><th class="num">ABST</th>
              {#each Object.values(qtypeLabels) as label}<th class="num type">{label}</th>{/each}
              <th class="num">COST</th><th>REVIEW</th>
            </tr></thead>
            <tbody>
              {#each selected.rows as row}
                <tr>
                  <td class="rank">{row.rank}</td>
                  <td><strong>{row.display_name || row.run_name}</strong><span>{row.system} · {row.run_name}</span></td>
                  <td class="num score">{pct(row.accuracy)}</td>
                  <td class="num">{pct(row.task_averaged_accuracy)}</td>
                  <td class="num">{pct(row.abstention_accuracy)}</td>
                  {#each Object.keys(qtypeLabels) as key}
                    <td class="num type-score" title={key}>{pct(row.per_question_type?.[key]?.accuracy)}</td>
                  {/each}
                  <td class="num">{money(row.cost_micro_usd)}</td>
                  <td><span class="verified">✓ {row.eligibility?.review?.verdict ?? "verified"}</span></td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      </section>

      <div class="lower-grid">
        <section class="panel">
          <div class="panel-head"><div><span>CATEGORY ROBUSTNESS</span><b>question-type evidence</b></div></div>
          <div class="category-grid">
            {#each selected.rows.slice(0, 3) as row}
              <div class="category-row">
                <strong>{row.display_name || row.run_name}</strong>
                {#each Object.entries(qtypeLabels) as [key, label]}
                  <div title={`${key}: ${pct(row.per_question_type?.[key]?.accuracy)}`}>
                    <span>{label}</span><i style={`width:${Math.max(0, Math.min(100, (row.per_question_type?.[key]?.accuracy ?? 0) * 100))}%`}></i>
                    <b>{pct(row.per_question_type?.[key]?.accuracy)}</b>
                  </div>
                {/each}
              </div>
            {/each}
          </div>
        </section>
        <section class="panel trust">
          <div class="panel-head"><div><span>TRUST / METHODOLOGY</span><b>reproducible</b></div></div>
          <dl>
            <div><dt>DATASET</dt><dd>{selected.benchmark} · {selected.limit ?? "all"} questions</dd></div>
            <div><dt>JUDGE</dt><dd>{selected.judge_model ?? "unrecorded"}</dd></div>
            <div><dt>ORACLE GOLD</dt><dd>{selected.rows.some((row) => row.oracle_gold) ? "mixed — inspect rows" : "no"}</dd></div>
            <div><dt>COHORT KEY</dt><dd title={selected.cohort_id}>{selected.cohort_id.slice(0, 54)}…</dd></div>
            {#if store.snapshot}
              <div><dt>RECORDS ROOT</dt><dd title={store.snapshot.source.records_root}>{store.snapshot.source.records_root}</dd></div>
              <div><dt>RECORDS DIGEST</dt><dd title={store.snapshot.source.records_digest ?? "unavailable"}>{store.snapshot.source.records_digest?.slice(0, 24) ?? "unavailable"}</dd></div>
              <div><dt>EXPORTER SHA</dt><dd title={store.snapshot.source.git_sha}>{store.snapshot.source.git_sha}</dd></div>
              <div><dt>GENERATED</dt><dd>{store.snapshot.generated_at}</dd></div>
            {/if}
            <div><dt>SOURCE</dt><dd><a href="https://github.com/jak-pan/symbiotic-mem-bench" target="_blank" rel="noreferrer">repository ↗</a> · <a href="https://github.com/jak-pan/symbiotic-mem-bench/blob/master/docs/longmemeval-methodology.md" target="_blank" rel="noreferrer">methodology ↗</a></dd></div>
          </dl>
        </section>
      </div>
    </div>
  {:else if store.mode !== "boot"}
    <div class="no-data"><b>NO VERIFIED RANKING DATA</b><span>The public claim surface remains empty until a record passes every review gate.</span></div>
  {/if}
</section>

<style>
  .rail { width: 252px; flex: none; overflow-y: auto; border-right: 1px solid var(--border); background: var(--bg-panel); }
  .rail-title { display: flex; justify-content: space-between; padding: 9px 12px; border-bottom: 1px solid var(--border); color: var(--text-faint); font-family: var(--sans); font-size: 9.5px; font-weight: 700; letter-spacing: .13em; }
  .rail-title b { color: var(--amber); }
  .rail-item { width: 100%; display: grid; grid-template-columns: 1fr auto; gap: 2px 8px; padding: 8px 12px; cursor: pointer; text-align: left; background: none; border: 0; border-bottom: 1px solid var(--border); color: var(--text-dim); }
  .rail-item:hover, .rail-item.active { background: var(--bg-hover); color: var(--text); }
  .rail-item.active { box-shadow: inset 2px 0 0 var(--amber); }
  .cohort-name { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .cohort-meta { color: var(--text-faint); font-size: 10px; }
  .status { grid-column: 1 / -1; color: var(--green); font-size: 8px; letter-spacing: .12em; }
  .empty-rail, .rail-law { padding: 12px; color: var(--text-faint); font-size: 10px; }
  .rail-law { border-top: 1px solid var(--border); }
  .rail-law p { margin-top: 7px; line-height: 1.45; }
  .rail-law b { color: var(--amber); font-weight: 500; }
  .work { min-width: 0; flex: 1; display: flex; flex-direction: column; overflow: hidden; }
  .hero { flex: none; display: grid; grid-template-columns: minmax(400px, 1fr) minmax(420px, .9fr); align-items: center; gap: 20px; padding: 12px 14px; border-bottom: 1px solid var(--border-bright); background: linear-gradient(100deg, rgba(255,165,36,.09), transparent 44%), var(--bg-panel); }
  .eyebrow { color: var(--amber); font-family: var(--sans); font-size: 8px; font-weight: 800; letter-spacing: .18em; }
  h1 { margin-top: 2px; font-size: 22px; line-height: 1.1; }
  h1 span { display: inline-block; padding: 1px 5px; vertical-align: middle; border: 1px solid var(--amber-dim); color: var(--amber-soft); font-size: 7.5px; letter-spacing: .1em; }
  .hero p { margin-top: 4px; color: var(--text-dim); font-size: 9.5px; }
  .metrics { display: grid; grid-template-columns: repeat(4, 1fr); }
  .metrics div { min-width: 0; padding: 2px 10px; border-left: 1px solid var(--border); }
  .metrics small { display: block; color: var(--text-faint); font-family: var(--sans); font-size: 7.5px; font-weight: 800; letter-spacing: .1em; }
  .metrics b { display: block; overflow: hidden; font-size: 17px; text-overflow: ellipsis; white-space: nowrap; }
  .metrics b.green { color: var(--green); }
  .metrics b.date { font-size: 11px; line-height: 25px; }
  .cohort-strip { flex: none; display: grid; grid-template-columns: 1.15fr 1fr 1.1fr 1fr .8fr; gap: 1px; background: var(--border); border-bottom: 1px solid var(--border); }
  .cohort-strip div { min-width: 0; padding: 7px 10px; background: var(--bg-panel); }
  .cohort-strip span { display: block; color: var(--text-faint); font-family: var(--sans); font-size: 8px; font-weight: 700; letter-spacing: .1em; }
  .cohort-strip b { display: block; overflow: hidden; color: var(--text-dim); font-size: 10px; font-weight: 500; text-overflow: ellipsis; white-space: nowrap; }
  .cohort-strip .strict b { color: var(--green); }
  .scroll-area { flex: 1; overflow: auto; padding: 12px; }
  .panel { border: 1px solid var(--border); background: var(--bg-panel); }
  .panel-head { min-height: 36px; display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 7px 10px; border-bottom: 1px solid var(--border); }
  .panel-head div { display: flex; align-items: baseline; gap: 10px; }
  .panel-head span { color: var(--text); font-family: var(--sans); font-size: 10px; font-weight: 700; letter-spacing: .12em; }
  .panel-head b { color: var(--amber); font-size: 9px; font-weight: 500; text-transform: uppercase; }
  .panel-head p { color: var(--text-faint); font-size: 9px; }
  .table-wrap { overflow-x: auto; }
  table { width: 100%; border-collapse: collapse; font-variant-numeric: tabular-nums; }
  th { padding: 7px 9px; border-bottom: 1px solid var(--border-bright); color: var(--text-faint); font-family: var(--sans); font-size: 8.5px; letter-spacing: .08em; text-align: left; white-space: nowrap; }
  td { padding: 7px 9px; border-bottom: 1px solid var(--border); white-space: nowrap; }
  tbody tr:nth-child(even) { background: var(--bg-row-alt); }
  tbody tr:hover { background: var(--bg-hover); }
  td strong, td span { display: block; }
  td span { color: var(--text-faint); font-size: 9px; }
  .num { text-align: right; }
  .rank, .score { color: var(--amber); font-size: 15px; font-weight: 700; }
  .type { color: var(--text-dim); }
  .type-score { color: var(--text-dim); font-size: 10px; }
  .verified { display: inline-block; color: var(--green); font-size: 9px; text-transform: uppercase; }
  .lower-grid { display: grid; grid-template-columns: minmax(520px, 1.35fr) minmax(360px, .85fr); gap: 12px; margin-top: 12px; }
  .category-grid { padding: 10px; }
  .category-row { display: grid; grid-template-columns: 180px repeat(6, minmax(68px, 1fr)); gap: 5px; align-items: center; margin-bottom: 8px; }
  .category-row strong { overflow: hidden; color: var(--text-dim); font-size: 9px; text-overflow: ellipsis; white-space: nowrap; }
  .category-row div { height: 30px; position: relative; overflow: hidden; border: 1px solid var(--border); background: var(--bg-elev); }
  .category-row div span, .category-row div b { position: relative; z-index: 1; display: block; padding: 1px 4px; font-size: 8px; }
  .category-row div span { color: var(--text-faint); }
  .category-row div b { color: var(--text); }
  .category-row i { position: absolute; inset: auto auto 0 0; height: 3px; background: var(--amber); }
  .trust dl { padding: 8px 10px; }
  .trust dl div { display: grid; grid-template-columns: 100px 1fr; gap: 8px; padding: 6px 0; border-bottom: 1px solid var(--border); }
  dt { color: var(--text-faint); font-family: var(--sans); font-size: 8px; font-weight: 700; letter-spacing: .1em; }
  dd { min-width: 0; overflow: hidden; color: var(--text-dim); font-size: 10px; text-overflow: ellipsis; white-space: nowrap; }
  .no-data { margin: auto; display: flex; flex-direction: column; align-items: center; gap: 8px; color: var(--text-faint); }
  .no-data b { color: var(--amber); font-family: var(--sans); letter-spacing: .12em; }
</style>
