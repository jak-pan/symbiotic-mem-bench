<script lang="ts">
  import { api } from "../lib/api";
  import type { RunDetail } from "../lib/types";
  import { pct, money, ms, tokens, shortHash } from "../lib/format";
  import { createAsyncData } from "../lib/async.svelte";
  import Panel from "../components/Panel.svelte";
  import RingGauge from "../components/RingGauge.svelte";

  let { id }: { id: string } = $props();
  const ad = createAsyncData<RunDetail>();

  $effect(() => {
    const runId = id;
    ad.reset();
    api.run(runId).then((d) => {
      if (runId !== id) return; // user switched runs mid-flight
      ad.set(d);
    });
  });

  const detail = $derived(ad.data);
  const loading = $derived(ad.loading);

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
        {#snippet actions()}
          {#if s.oracle_gold}
            <span class="gold-badge" title="Oracle-gold run: gold evidence fed straight to the answerer (reader-ceiling method) — not real recall">G</span>
          {/if}
        {/snippet}
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
          {#if s.oracle_gold}
            <dt>EVIDENCE</dt><dd class="gold">GOLD · oracle (recall bypassed)</dd>
          {/if}
          <dt>JUDGE</dt><dd class="amber">{c.judge_model ?? "—"}</dd>
          <dt>JUDGE·MODE</dt><dd class:amber={c.judge_prompt_mode === "official"}>{c.judge_prompt_mode ?? "legacy"}</dd>
          <dt>ANSWER</dt><dd>{c.models?.answer ?? "—"}</dd>
          <dt>DISTILL</dt><dd>{c.models?.distill ?? "—"}</dd>
          <dt>EMBED</dt><dd>{c.models?.embed ?? "—"}</dd>
          <dt>RERANK</dt><dd class:dim={!c.models?.rerank}>{c.models?.rerank ?? "none"}</dd>
          <dt>QSET·FP</dt><dd class="mono-num">{shortHash(c.dataset_fingerprint, 16)}</dd>
          <dt>CFG·SIG</dt><dd class="mono-num">{shortHash(c.config_signature, 16)}</dd>
          <!-- `cohort_id` is the spelled-out comparability identity, not a
               hash: show it whole so the judge and question set are readable. -->
          <dt>COHORT</dt><dd class="mono-num wrap" title={s.cohort_id}>{s.cohort_id}</dd>
        </dl>
      </Panel>
    </div>

    <div class="ov-grid" class:trial-layout={s.is_trial_run && s.trial_markers.length}>
      {#if s.is_trial_run && s.trial_markers.length}
        <div class="cell trial-cell">
          <Panel title="Trial Context" tag="diagnostic">
            <div class="trial">
              {#each s.trial_markers as marker (`${marker.stack_id}:${marker.change_id}`)}
                <div class="trial-card">
                  <div class="trial-head">
                    <span class="chip amber">{marker.focused ? "FOCUSED" : "TRIAL"}</span>
                    <b>{marker.change_title || marker.change_id}</b>
                  </div>
                  <dl class="kv trial-kv">
                    <dt>STACK</dt><dd class="mono-num">{marker.stack_id}</dd>
                    <dt>CHANGE</dt><dd class="mono-num">{marker.change_id}</dd>
                    <dt>SAMPLE</dt>
                    <dd class="mono-num">
                      {marker.question_count}Q
                      <span class={marker.focused ? "amber" : "up"}>{marker.sample_classification}</span>
                    </dd>
                    <dt>DECISION</dt><dd>{marker.decision || "—"}</dd>
                    <dt>ANALYSIS</dt><dd class="mono-num">{marker.analysis_path}</dd>
                    <dt>DELTA</dt>
                    <dd>
                      <span class="up">{marker.improvements} fixed</span>
                      <span class="sep">/</span>
                      <span class={marker.regressions ? "down" : "dim"}>{marker.regressions} regressed</span>
                    </dd>
                    <dt>SCORE</dt>
                    <dd class="mono-num">
                      {pct(marker.aggregate_accuracy)}%
                      {#if marker.aggregate_correct != null && marker.aggregate_total != null}
                        <span class="dim">({marker.aggregate_correct}/{marker.aggregate_total})</span>
                      {/if}
                    </dd>
                  </dl>
                  {#if marker.focused}
                    <div class="trial-note">Focused stack for one failure class. Use a stratified 25-50Q trial before broad conclusions.</div>
                  {/if}
                </div>
              {/each}
            </div>
          </Panel>
        </div>
      {/if}

      <div class="cell params-cell">
        <Panel title="Run Parameters" tag="{paramEntries.length} fields" scroll>
          <dl class="kv params">
            {#each paramEntries as [k, v] (k)}
              <dt>{k}</dt><dd class="mono-num">{typeof v === "object" ? JSON.stringify(v) : String(v)}</dd>
            {/each}
          </dl>
        </Panel>
      </div>

      <div class="cell artifacts-cell">
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
      </div>

      {#if detail.cost && detail.cost.models.length}
        <div class="cell models-cell">
          <Panel title="Model Calls" tag="{detail.cost.calls} calls" scroll>
            <table class="grid compact-models">
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
        </div>
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
  .kv dd.wrap {
    white-space: normal;
    overflow-wrap: anywhere;
    font-size: 10px;
  }

  .ov-grid {
    display: grid;
    grid-template-columns: repeat(12, minmax(0, 1fr));
    gap: 10px;
    align-items: start;
  }
  .cell {
    min-width: 0;
  }
  .cell :global(.panel) {
    height: 100%;
    max-height: 360px;
  }
  .trial-cell {
    grid-column: span 6;
  }
  .params-cell {
    grid-column: span 4;
  }
  .artifacts-cell {
    grid-column: span 4;
  }
  .models-cell {
    grid-column: span 4;
  }
  .trial-layout .params-cell,
  .trial-layout .artifacts-cell,
  .trial-layout .models-cell {
    grid-column: span 6;
  }
  .models-cell :global(.panel) {
    max-height: 250px;
  }
  .trial {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .trial-card {
    border-left: 2px solid var(--amber-dim);
    padding: 4px 4px 2px 10px;
  }
  .trial-head {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 8px;
  }
  .trial-head b {
    color: var(--text);
    font-size: 13px;
    line-height: 1.2;
  }
  .trial-kv {
    grid-template-columns: 72px minmax(0, 1fr);
    gap: 5px 14px;
  }
  .trial-kv dd {
    white-space: normal;
    overflow-wrap: anywhere;
    font-size: 11.5px;
    line-height: 1.35;
  }
  .sep {
    color: var(--text-faint);
    margin: 0 4px;
  }
  .trial-note {
    margin-top: 8px;
    padding-top: 7px;
    border-top: 1px solid var(--border);
    color: var(--amber);
    font-size: 10px;
    line-height: 1.4;
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
  .compact-models td:first-child {
    white-space: normal;
  }

  .op {
    display: block;
    color: var(--text-faint);
    font-size: 9px;
  }
  .dim {
    color: var(--text-dim);
  }
  .gold {
    color: var(--gold);
  }
  .gold-badge {
    border: 1px solid var(--gold);
    background: rgba(232, 195, 74, 0.12);
    color: var(--gold);
    font-family: var(--sans);
    font-size: 9px;
    font-weight: 700;
    letter-spacing: 0.1em;
    padding: 2px 7px;
  }
</style>
