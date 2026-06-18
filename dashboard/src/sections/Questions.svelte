<script lang="ts">
  import { api } from "../lib/api";
  import type { QuestionRow } from "../lib/types";
  import { qtypeShort } from "../lib/format";

  let { id }: { id: string } = $props();
  let rows = $state<QuestionRow[]>([]);
  let loading = $state(true);
  let verdict = $state<"all" | "correct" | "wrong" | "abstain" | "error">("all");
  let qtype = $state<string>("all");
  let search = $state("");
  let active = $state<QuestionRow | null>(null);

  $effect(() => {
    const runId = id;
    loading = true;
    active = null;
    api.questions(runId).then((q) => { rows = q; loading = false; });
  });

  const types = $derived(["all", ...new Set(rows.map((r) => r.question_type).filter(Boolean) as string[])]);

  const filtered = $derived.by(() => {
    const q = search.trim().toLowerCase();
    return rows.filter((r) => {
      if (verdict === "correct" && r.label !== true) return false;
      if (verdict === "wrong" && r.label !== false) return false;
      if (verdict === "abstain" && !r.is_abstention) return false;
      if (verdict === "error" && !r.error) return false;
      if (qtype !== "all" && r.question_type !== qtype) return false;
      if (q && !(`${r.question ?? ""} ${r.gold_answer ?? ""} ${r.hypothesis ?? ""} ${r.question_id}`.toLowerCase().includes(q))) return false;
      return true;
    });
  });

  const stats = $derived.by(() => {
    const correct = rows.filter((r) => r.label === true).length;
    const wrong = rows.filter((r) => r.label === false).length;
    return { correct, wrong, total: rows.length };
  });
</script>

<div class="q">
  <div class="qbar">
    <div class="seg">
      {#each [["all", "ALL"], ["correct", "CORRECT"], ["wrong", "WRONG"], ["abstain", "ABSTAIN"], ["error", "ERROR"]] as [v, l] (v)}
        <button class="sgb" class:on={verdict === v} onclick={() => (verdict = v as any)}>{l}</button>
      {/each}
    </div>
    <select class="field qsel" bind:value={qtype}>
      {#each types as t (t)}<option value={t}>{t === "all" ? "ALL TYPES" : qtypeShort(t)}</option>{/each}
    </select>
    <input class="field" bind:value={search} placeholder="search question / answer / id…" spellcheck="false" />
    <div class="qstat">
      <span class="up">{stats.correct}✓</span>
      <span class="down">{stats.wrong}✗</span>
      <span class="faint">/ {filtered.length} shown</span>
    </div>
  </div>

  <div class="qmain" class:split={active}>
    <div class="qtable">
      {#if loading}
        <div class="load">LOADING QUESTIONS…</div>
      {:else}
        <table class="grid">
          <thead>
            <tr>
              <th style="width:30px">V</th>
              <th style="width:78px">ID</th>
              <th style="width:96px">Type</th>
              <th>Question</th>
              <th>Answer (gold)</th>
              <th>Hypothesis</th>
              <th style="width:70px">Route</th>
            </tr>
          </thead>
          <tbody>
            {#each filtered as r (r.question_id)}
              <tr class:selected={active?.question_id === r.question_id} onclick={() => (active = r)}>
                <td class="vc">
                  {#if r.error}<span class="down" title={r.error}>!</span>
                  {:else if r.label === true}<span class="up">✓</span>
                  {:else if r.label === false}<span class="down">✗</span>
                  {:else}<span class="faint">·</span>{/if}
                </td>
                <td class="mono-num faint">{r.question_id}</td>
                <td class="ty">{qtypeShort(r.question_type)}{#if r.is_abstention}<span class="abst">A</span>{/if}</td>
                <td class="clip">{r.question ?? "—"}</td>
                <td class="clip dim">{r.gold_answer ?? "—"}</td>
                <td class="clip" class:wronghyp={r.label === false}>{r.hypothesis ?? "—"}</td>
                <td class="mono-num faint">{r.router_pick ?? r.final_pick ?? "—"}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      {/if}
    </div>

    {#if active}
      <aside class="drawer fade-in">
        <div class="dh">
          <span class="chip {active.label === true ? 'green' : active.label === false ? 'red' : ''}">
            {active.label === true ? "CORRECT" : active.label === false ? "WRONG" : "UNSCORED"}
          </span>
          <span class="did mono-num">{active.question_id}</span>
          <button class="x" onclick={() => (active = null)}>✕</button>
        </div>
        <div class="db">
          <div class="fld"><span class="label">TYPE</span><div>{active.question_type ?? "—"} {#if active.is_abstention}<span class="chip amber">abstention</span>{/if}</div></div>
          <div class="fld"><span class="label">QUESTION</span><div class="txt">{active.question ?? "—"}</div></div>
          <div class="fld"><span class="label">GOLD ANSWER</span><div class="txt gold">{active.gold_answer ?? "—"}</div></div>
          <div class="fld"><span class="label">HYPOTHESIS</span><div class="txt" class:wrongbox={active.label === false} class:rightbox={active.label === true}>{active.hypothesis ?? "—"}</div></div>
          <div class="fld2">
            <div><span class="label">JUDGE</span><div>{active.judge_model ?? "—"} → <b class:up={active.label} class:down={active.label === false}>{active.judge_raw ?? "—"}</b></div></div>
          </div>
          <div class="fld2">
            <div><span class="label">ROUTER PICK</span><div class="mono-num">{active.router_pick ?? "—"}</div></div>
            <div><span class="label">INITIAL→FINAL</span><div class="mono-num">{active.initial_pick ?? "—"} → {active.final_pick ?? "—"}</div></div>
          </div>
          {#if active.error}<div class="fld"><span class="label">ERROR</span><div class="txt down">{active.error}</div></div>{/if}
        </div>
      </aside>
    {/if}
  </div>
</div>

<style>
  .q {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }
  .qbar {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 10px;
    border-bottom: 1px solid var(--border-bright);
    background: var(--bg-panel);
    flex: none;
  }
  .seg {
    display: flex;
  }
  .sgb {
    padding: 3px 9px;
    background: var(--bg-elev);
    border: 1px solid var(--border);
    border-right: none;
    color: var(--text-dim);
    font-size: 9.5px;
    font-weight: 700;
    letter-spacing: 0.06em;
    cursor: pointer;
  }
  .sgb:last-child {
    border-right: 1px solid var(--border);
  }
  .sgb.on {
    background: rgba(255, 165, 36, 0.1);
    color: var(--amber);
    border-color: var(--amber-dim);
  }
  .qsel {
    width: 150px;
  }
  .qbar .field:not(.qsel) {
    flex: 1;
    max-width: 340px;
  }
  .qstat {
    margin-left: auto;
    display: flex;
    gap: 8px;
    font-size: 11px;
    font-weight: 600;
  }
  .faint {
    color: var(--text-faint);
  }

  .qmain {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: 1fr;
  }
  .qmain.split {
    grid-template-columns: 1fr 380px;
  }
  .qtable {
    overflow: auto;
    min-height: 0;
  }
  .vc {
    text-align: center;
    font-weight: 700;
  }
  .ty {
    font-size: 9px;
    color: var(--text-dim);
    letter-spacing: 0.03em;
  }
  .abst {
    color: var(--amber);
    margin-left: 3px;
  }
  .clip {
    max-width: 320px;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  td.dim {
    color: var(--text-dim);
  }
  .wronghyp {
    color: var(--red);
  }

  .drawer {
    border-left: 1px solid var(--border-bright);
    background: var(--bg-panel);
    overflow: auto;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .dh {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 10px;
    border-bottom: 1px solid var(--border-bright);
    position: sticky;
    top: 0;
    background: var(--bg-panel);
  }
  .did {
    color: var(--text-dim);
    font-size: 11px;
  }
  .x {
    margin-left: auto;
    background: none;
    border: none;
    color: var(--text-faint);
    cursor: pointer;
    font-size: 13px;
  }
  .x:hover {
    color: var(--red);
  }
  .db {
    padding: 10px;
    display: flex;
    flex-direction: column;
    gap: 11px;
  }
  .fld2 {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px;
  }
  .fld .label,
  .fld2 .label {
    display: block;
    margin-bottom: 3px;
  }
  .txt {
    font-size: 12px;
    line-height: 1.5;
    color: var(--text);
    background: var(--bg);
    border: 1px solid var(--border);
    padding: 6px 8px;
  }
  .gold {
    color: var(--gold);
  }
  .wrongbox {
    border-color: var(--red-dim);
    color: var(--red);
  }
  .rightbox {
    border-color: var(--green-dim);
  }
  .load {
    padding: 30px;
    color: var(--text-faint);
    letter-spacing: 0.2em;
  }
</style>
