<script lang="ts">
  import { api } from "../lib/api";
  import type { RunDetail } from "../lib/types";
  import { pct, money, ms, tokens, shortHash } from "../lib/format";
  import Panel from "../components/Panel.svelte";
  import RingGauge from "../components/RingGauge.svelte";

  let { id }: { id: string } = $props();
  let detail = $state<RunDetail | null>(null);
  let loading = $state(true);

  $effect(() => {
    const runId = id;
    loading = true;
    detail = null;
    api.run(runId).then((d) => { detail = d; loading = false; });
  });

  const SKIP = new Set(["schema", "run_root", "run_name", "system", "benchmark", "artifact_manifest", "imported_artifacts"]);
  const paramEntries = $derived(
    detail ? Object.entries(detail.params ?? {}).filter(([k, v]) => !SKIP.has(k) && v != null && v !== "") : [],
  );
  const ALL_KINDS = ["hypotheses", "verdicts", "partial_verdicts", "provenance", "scored", "score_summary", "memory_traces", "model_traces"];
</script>

{#if loading}
  <div class="load">LOADING RUN…</div>
{:else if detail}
  {@const s = detail.summary}
  {@const c = detail.cohort}
  <div class="ov fade-in">
    <div class="ov-top">
      <Panel title="Score" tag={s.run_kind}>
        <div class="score-row">
          <RingGauge value={s.accuracy} label="overall" color="var(--amber)" />
          <div class="tiles">
            <div class="tile"><span class="tl">TASK·AVG</span><b class="mono-num">{pct(s.task_averaged_accuracy)}<i>%</i></b></div>
            <div class="tile"><span class="tl">ABSTENTION</span><b class="mono-num">{pct(s.abstention_accuracy)}<i>%</i></b></div>
            <div class="tile"><span class="tl">CORRECT</span><b class="mono-num">{s.accuracy_correct ?? "—"}<i>/{s.accuracy_total ?? "—"}</i></b></div>
            <div class="tile">
              <span class="tl">COST</span>
              <b class="mono-num">{money(s.cost_micro_usd ?? c.cost_micro_usd)}</b>
              {#if detail.cost?.cost_estimated}<span class="op">est</span>{/if}
            </div>
            <div class="tile"><span class="tl">LAT·P50</span><b class="mono-num">{ms(s.latency_ms_p50 ?? c.latency_ms_p50)}</b></div>
            <div class="tile"><span class="tl">LAT·P95</span><b class="mono-num">{ms(s.latency_ms_p95 ?? c.latency_ms_p95)}</b></div>
          </div>
        </div>
      </Panel>

      <Panel title="Cohort & Models">
        <dl class="kv">
          <dt>JUDGE</dt><dd class="amber">{c.judge_model ?? "—"}</dd>
          <dt>ANSWER</dt><dd>{c.models?.answer ?? "—"}</dd>
          <dt>DISTILL</dt><dd>{c.models?.distill ?? "—"}</dd>
          <dt>EMBED</dt><dd>{c.models?.embed ?? "—"}</dd>
          <dt>QSET·FP</dt><dd class="mono-num">{shortHash(c.dataset_fingerprint, 16)}</dd>
          <dt>CFG·SIG</dt><dd class="mono-num">{shortHash(c.config_signature, 16)}</dd>
          <dt>COHORT</dt><dd class="mono-num">{shortHash(s.cohort_id, 16)}</dd>
        </dl>
      </Panel>
    </div>

    <div class="ov-grid">
      <Panel title="Run Parameters" tag="{paramEntries.length} fields" scroll>
        <dl class="kv params">
          {#each paramEntries as [k, v] (k)}
            <dt>{k}</dt><dd class="mono-num">{typeof v === "object" ? JSON.stringify(v) : String(v)}</dd>
          {/each}
        </dl>
      </Panel>

      <Panel title="Artifacts">
        <div class="art">
          {#each ALL_KINDS as kind (kind)}
            {@const has = s.artifacts_available.includes(kind)}
            <div class="artrow" class:miss={!has}>
              <span class="aci">{has ? "●" : "○"}</span>
              <span class="acn">{kind}</span>
              <span class="acs">{has ? "present" : "missing"}</span>
            </div>
          {/each}
          <div class="art-state">
            NATIVE STATE: {#if s.native_state_available}<span class="up">available</span>{:else}<span class="down">artifact-only</span>{/if}
          </div>
        </div>
      </Panel>

      {#if detail.cost && detail.cost.models.length}
        <Panel title="Model Calls" tag="{detail.cost.calls} calls" scroll>
          <table class="grid">
            <thead><tr><th>Model</th><th class="num">Calls</th><th class="num">In</th><th class="num">Out</th><th class="num">Cost</th><th class="num">p50</th></tr></thead>
            <tbody>
              {#each detail.cost.models as m (m.model)}
                <tr>
                  <td>{m.model}<span class="op">{m.operator}·{m.operation}</span></td>
                  <td class="num mono-num">{m.calls}</td>
                  <td class="num mono-num dim">{tokens(m.input_tokens)}</td>
                  <td class="num mono-num dim">{tokens(m.output_tokens)}</td>
                  <td class="num mono-num dim">{money(m.cost_micro_usd)}{m.cost_estimated ? " est" : ""}</td>
                  <td class="num mono-num dim">{ms(m.latency_ms_p50)}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        </Panel>
      {/if}
    </div>
  </div>
{/if}

<style>
  .load {
    padding: 40px;
    color: var(--text-faint);
    letter-spacing: 0.2em;
  }
  .ov {
    padding: 10px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .ov-top {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
  }
  .score-row {
    display: flex;
    gap: 18px;
    align-items: center;
  }
  .tiles {
    flex: 1;
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 1px;
    background: var(--border);
    border: 1px solid var(--border);
  }
  .tile {
    background: var(--bg-panel);
    padding: 8px 10px;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .tl {
    font-family: var(--sans);
    font-size: 8px;
    font-weight: 700;
    letter-spacing: 0.1em;
    color: var(--text-faint);
  }
  .tile b {
    font-size: 17px;
    font-weight: 600;
    color: var(--text);
  }
  .tile b i {
    font-size: 10px;
    color: var(--text-faint);
    font-style: normal;
  }

  .kv {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 3px 12px;
    align-content: start;
  }
  .kv dt {
    font-family: var(--sans);
    font-size: 8.5px;
    font-weight: 700;
    letter-spacing: 0.08em;
    color: var(--text-faint);
    align-self: center;
  }
  .kv dd {
    font-size: 11px;
    color: var(--text);
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .params {
    gap: 2px 12px;
  }
  .params dd {
    color: var(--text-dim);
    white-space: nowrap;
  }

  .ov-grid {
    display: grid;
    grid-template-columns: 1.2fr 0.8fr 1.4fr;
    gap: 10px;
    align-items: start;
  }
  .ov-grid :global(.panel) {
    max-height: 320px;
  }

  .art {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .artrow {
    display: grid;
    grid-template-columns: 16px 1fr auto;
    align-items: center;
    gap: 6px;
    padding: 2px 0;
    font-size: 11px;
  }
  .aci {
    color: var(--green);
  }
  .artrow.miss .aci {
    color: var(--text-faint);
  }
  .acn {
    color: var(--text-dim);
  }
  .artrow.miss .acn {
    color: var(--text-faint);
  }
  .acs {
    font-size: 9px;
    color: var(--text-faint);
    letter-spacing: 0.05em;
  }
  .art-state {
    margin-top: 8px;
    padding-top: 6px;
    border-top: 1px solid var(--border);
    font-size: 9.5px;
    letter-spacing: 0.06em;
    color: var(--text-faint);
  }

  .op {
    display: block;
    color: var(--text-faint);
    font-size: 9px;
  }
  .dim {
    color: var(--text-dim);
  }
</style>
