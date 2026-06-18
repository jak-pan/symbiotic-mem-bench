<script lang="ts">
  import { api } from "../lib/api";
  import { store } from "../lib/store.svelte";
  import type { CompareResponse, RunSummary } from "../lib/types";
  import { pct, pctSign, qtypeShort, deltaClass } from "../lib/format";
  import Panel from "../components/Panel.svelte";
  import DeltaBars from "../components/DeltaBars.svelte";

  let { id, selected }: { id: string; selected: RunSummary } = $props();
  let baseId = $state<string>("");
  let data = $state<CompareResponse | null>(null);
  let loading = $state(false);

  // Candidate is the focused run; baseline defaults to the cohort leader.
  const candidates = $derived(store.runs.filter((r) => r.run_id !== id));
  const cohortMates = $derived(candidates.filter((r) => r.cohort_id === selected.cohort_id));

  $effect(() => {
    if (!baseId || !store.byId(baseId) || baseId === id) {
      const pool = cohortMates.length ? cohortMates : candidates;
      baseId = pool[0]?.run_id ?? "";
    }
  });

  $effect(() => {
    const b = baseId;
    const c = id;
    if (!b || !c) return;
    loading = true;
    data = null;
    api.compare(b, c).then((d) => { data = d; loading = false; });
  });

  const typeItems = $derived(
    data?.result.per_type.map((t) => ({ label: qtypeShort(t.question_type), delta: t.delta, n: t.n })) ?? [],
  );
</script>

<div class="cmp">
  <div class="cbar">
    <span class="label">BASELINE</span>
    <select class="field bsel" bind:value={baseId}>
      <optgroup label="same cohort">
        {#each cohortMates as r (r.run_id)}<option value={r.run_id}>{r.run_name} ({pct(r.accuracy)}%)</option>{/each}
      </optgroup>
      <optgroup label="other">
        {#each candidates.filter((r) => r.cohort_id !== selected.cohort_id) as r (r.run_id)}<option value={r.run_id}>{r.run_name} · {r.benchmark}/{r.limit}Q</option>{/each}
      </optgroup>
    </select>
    <span class="vs">vs</span>
    <span class="cand chip amber">{selected.run_name}</span>
  </div>

  {#if loading}
    <div class="load">DIFFING…</div>
  {:else if data}
    {@const r = data.result}
    <div class="cmp-body fade-in">
      <div class="strip">
        <div class="big">
          <div class="bl">
            <span class="label">{data.base.run_name}</span>
            <b class="mono-num">{pct(r.base_accuracy)}<i>%</i></b>
          </div>
          <div class="arrow {deltaClass(r.accuracy_delta)}">
            →
            <span class="dv mono-num">{pctSign(r.accuracy_delta)}</span>
          </div>
          <div class="bl">
            <span class="label amber">{data.candidate.run_name}</span>
            <b class="mono-num amber">{pct(r.candidate_accuracy)}<i>%</i></b>
          </div>
        </div>

        <div class="buckets">
          <div class="bk up"><b class="mono-num">{r.counts.newly_correct}</b><span>FIXED</span></div>
          <div class="bk down"><b class="mono-num">{r.counts.newly_wrong}</b><span>REGRESSED</span></div>
          <div class="bk"><b class="mono-num">{r.counts.unchanged_wrong}</b><span>STILL WRONG</span></div>
          <div class="bk"><b class="mono-num">{r.counts.unchanged_correct}</b><span>STILL RIGHT</span></div>
          <div class="bk"><b class="mono-num">{r.counts.abstention_changes}</b><span>ABST Δ</span></div>
          <div class="bk"><b class="mono-num">{r.counts.common}</b><span>COMMON</span></div>
        </div>
      </div>

      <div class="cmp-grid">
        <Panel title="Per-Category Δ" tag="candidate − baseline">
          {#if typeItems.length}<DeltaBars items={typeItems} />{:else}<div class="faint">no shared categories</div>{/if}
        </Panel>

        <Panel title="Changed Verdicts" tag="{r.changed.length} flips" flush scroll>
          <table class="grid">
            <thead><tr><th style="width:30px"></th><th style="width:74px">ID</th><th>Question</th><th>Baseline</th><th>Candidate</th></tr></thead>
            <tbody>
              {#each r.changed as ch (ch.question_id)}
                <tr>
                  <td class="tc">{#if ch.transition === "newly_correct"}<span class="up">▲</span>{:else}<span class="down">▼</span>{/if}</td>
                  <td class="mono-num faint">{ch.question_id}</td>
                  <td class="clip">{ch.question ?? "—"}</td>
                  <td class="clip" class:down={ch.base_label === false}>{ch.base_hypothesis ?? "—"}</td>
                  <td class="clip" class:up={ch.candidate_label === true} class:down={ch.candidate_label === false}>{ch.candidate_hypothesis ?? "—"}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        </Panel>
      </div>
    </div>
  {/if}
</div>

<style>
  .cmp {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }
  .cbar {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 10px;
    border-bottom: 1px solid var(--border-bright);
    background: var(--bg-panel);
  }
  .bsel {
    width: 300px;
  }
  .vs {
    color: var(--text-faint);
    font-style: italic;
  }
  .cmp-body {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    padding: 10px;
    gap: 10px;
  }
  .strip {
    display: grid;
    grid-template-columns: 1fr 1.3fr;
    gap: 10px;
  }
  .big {
    display: flex;
    align-items: center;
    justify-content: space-around;
    background: var(--bg-panel);
    border: 1px solid var(--border);
    padding: 14px;
  }
  .bl {
    display: flex;
    flex-direction: column;
    gap: 4px;
    text-align: center;
  }
  .bl b {
    font-size: 28px;
    font-weight: 600;
  }
  .bl b i {
    font-size: 13px;
    color: var(--text-faint);
    font-style: normal;
  }
  .arrow {
    display: flex;
    flex-direction: column;
    align-items: center;
    font-size: 22px;
  }
  .arrow .dv {
    font-size: 15px;
    font-weight: 700;
  }
  .buckets {
    display: grid;
    grid-template-columns: repeat(6, 1fr);
    gap: 1px;
    background: var(--border);
    border: 1px solid var(--border);
  }
  .bk {
    background: var(--bg-panel);
    padding: 10px 6px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 3px;
  }
  .bk b {
    font-size: 22px;
    font-weight: 600;
    color: var(--text);
  }
  .bk span {
    font-family: var(--sans);
    font-size: 7.5px;
    font-weight: 700;
    letter-spacing: 0.08em;
    color: var(--text-faint);
  }
  .bk.up b {
    color: var(--green);
  }
  .bk.down b {
    color: var(--red);
  }
  .cmp-grid {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: 320px 1fr;
    gap: 10px;
  }
  .cmp-grid :global(.panel) {
    min-height: 0;
  }
  .tc {
    text-align: center;
  }
  .clip {
    max-width: 280px;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .faint {
    color: var(--text-faint);
  }
  .load {
    padding: 30px;
    color: var(--text-faint);
    letter-spacing: 0.2em;
  }
</style>
