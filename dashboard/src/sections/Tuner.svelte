<script lang="ts">
  import { api } from "../lib/api";
  import type { ParamField, RunnerPreview, RunnerSchema, RunSummary } from "../lib/types";
  import Panel from "../components/Panel.svelte";

  let { selected }: { selected: RunSummary } = $props();
  let schema = $state<RunnerSchema | null>(null);
  let values = $state<Record<string, any>>({});
  let preview = $state<RunnerPreview | null>(null);
  let copied = $state<string | null>(null);
  let planning = $state(false);

  // Load schema once; prefill from the focused run's params each time it changes.
  $effect(() => {
    api.runnerSchema().then(async (s) => {
      schema = s;
      const detail = await api.run(selected.run_id).catch(() => null);
      const params = detail?.params ?? {};
      const v: Record<string, any> = {};
      for (const f of s.fields) {
        v[f.name] = params[f.name] ?? f.default ?? (f.kind === "bool" ? false : "");
      }
      values = v;
    });
  });

  // Re-plan whenever values change (debounced).
  let timer: ReturnType<typeof setTimeout>;
  $effect(() => {
    const snapshot = JSON.stringify(values);
    if (!schema) return;
    clearTimeout(timer);
    planning = true;
    timer = setTimeout(async () => {
      preview = await api.runnerPlan(JSON.parse(snapshot)).catch(() => null);
      planning = false;
    }, 220);
  });

  const groups = $derived.by(() => {
    const m = new Map<string, ParamField[]>();
    for (const f of schema?.fields ?? []) {
      if (!m.has(f.group)) m.set(f.group, []);
      m.get(f.group)!.push(f);
    }
    return [...m.entries()];
  });

  async function copy(text: string, tag: string) {
    await navigator.clipboard.writeText(text);
    copied = tag;
    setTimeout(() => (copied = null), 1200);
  }

  function fullScript(p: RunnerPreview): string {
    const env = p.env_defaults.map(([k, v]) => `${k}=${v}`).join(" \\\n  ");
    let out = p.env_defaults.length
      ? `# env defaults (secrets loaded from repo-local .env.test.local if present)\n${env} \\\n  ${p.run_shell}`
      : p.run_shell;
    if (p.score_shell) out += `\n\n${p.score_shell}`;
    return out;
  }
</script>

<div class="tn">
  <div class="form">
    <div class="form-h">
      <span class="label">RUN CONFIGURATION</span>
      <span class="seed">seeded from <b>{selected.run_name}</b></span>
    </div>
    <div class="form-body">
      {#each groups as [group, fields] (group)}
        <div class="grp">
          <div class="grp-t label">{group}</div>
          {#each fields as f (f.name)}
            <label class="row">
              <span class="fn" title={f.help}>{f.label}{#if f.required}<i class="req">*</i>{/if}</span>
              {#if f.kind === "bool"}
                <button class="toggle" class:on={values[f.name]} onclick={() => (values[f.name] = !values[f.name])} type="button">
                  <span class="kn"></span><em>{values[f.name] ? "ON" : "OFF"}</em>
                </button>
              {:else if f.kind === "enum"}
                <select class="field" bind:value={values[f.name]}>
                  <option value="">—</option>
                  {#each [...new Set([...f.options, ...(f.observed ?? [])])] as opt (opt)}<option value={opt}>{opt}</option>{/each}
                </select>
              {:else if f.kind === "int"}
                <input class="field" type="number" bind:value={values[f.name]} />
              {:else}
                <input class="field" bind:value={values[f.name]} placeholder={f.kind === "path" ? "path…" : ""} spellcheck="false"
                  list={f.observed?.length ? `dl-${f.name}` : undefined} />
                {#if f.observed?.length}
                  <datalist id="dl-{f.name}">{#each f.observed as o (o)}<option value={o}></option>{/each}</datalist>
                {/if}
              {/if}
            </label>
          {/each}
        </div>
      {/each}
    </div>
  </div>

  <div class="out">
    <Panel title="Command Preview" tag={planning ? "planning…" : "membench command"}>
      {#if preview}
        {#if preview.warnings.length}
          <div class="warns">
            {#each preview.warnings as w (w)}<div class="warn">⚠ {w}</div>{/each}
          </div>
        {:else}
          <div class="ok">✓ inputs present — ready to run</div>
        {/if}

        <div class="cmd-block">
          <div class="cb-h"><span class="label">RUN</span><button class="cp" onclick={() => copy(preview!.run_shell, "run")}>{copied === "run" ? "✓ COPIED" : "COPY"}</button></div>
          <pre>{preview.run_shell}</pre>
        </div>
        {#if preview.score_shell}
          <div class="cmd-block">
            <div class="cb-h"><span class="label">SCORE</span><button class="cp" onclick={() => copy(preview!.score_shell!, "score")}>{copied === "score" ? "✓ COPIED" : "COPY"}</button></div>
            <pre>{preview.score_shell}</pre>
          </div>
        {/if}

        {#if preview.env_defaults.length}
          <div class="cmd-block">
            <div class="cb-h"><span class="label">ENV DEFAULTS</span><span class="faint">{preview.env_defaults.length} vars · repo-local .env.test.local</span></div>
            <pre class="env">{preview.env_defaults.map(([k, v]) => `${k}=${v}`).join("\n")}</pre>
          </div>
        {/if}

        <div class="actions">
          <button class="btn full" onclick={() => copy(fullScript(preview!), "all")}>
            {copied === "all" ? "✓ COPIED FULL SCRIPT" : "⧉ COPY FULL SCRIPT"}
          </button>
          <button class="btn spawn" disabled title="Live execution arrives in the next phase">▶ SPAWN RUN (preview mode)</button>
        </div>
        <div class="note">Preview mode — the dashboard builds the exact command; run it in your terminal. Live spawn + log streaming is the next milestone.</div>
      {:else}
        <div class="faint">configure parameters to generate a command…</div>
      {/if}
    </Panel>

    <Panel title="Runs / Status" tag="0 active">
      <div class="jobs-empty">
        <p>No active runs.</p>
        <p class="faint">When live execution lands, spawned jobs appear here with status, progress, and streaming logs.</p>
      </div>
    </Panel>
  </div>
</div>

<style>
  .tn {
    display: grid;
    grid-template-columns: 380px 1fr;
    gap: 1px;
    background: var(--border);
    height: 100%;
    min-height: 0;
  }
  .form {
    background: var(--bg-panel);
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  .form-h {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 8px 10px;
    border-bottom: 1px solid var(--border-bright);
  }
  .seed {
    font-size: 9.5px;
    color: var(--text-faint);
  }
  .seed b {
    color: var(--text-dim);
  }
  .form-body {
    overflow: auto;
    padding: 8px 10px;
  }
  .grp {
    margin-bottom: 12px;
  }
  .grp-t {
    padding: 3px 0;
    border-bottom: 1px dotted var(--border);
    margin-bottom: 6px;
    color: var(--amber);
  }
  .row {
    display: grid;
    grid-template-columns: 116px 1fr;
    align-items: center;
    gap: 8px;
    margin-bottom: 5px;
  }
  .fn {
    font-size: 10.5px;
    color: var(--text-dim);
  }
  .req {
    color: var(--red);
    font-style: normal;
  }
  .toggle {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    background: var(--bg);
    border: 1px solid var(--border-bright);
    padding: 3px 5px;
    cursor: pointer;
    width: 100%;
  }
  .toggle .kn {
    width: 22px;
    height: 12px;
    background: var(--bg-elev);
    border: 1px solid var(--border-bright);
    position: relative;
    transition: all 0.15s;
  }
  .toggle .kn::after {
    content: "";
    position: absolute;
    top: 1px;
    left: 1px;
    width: 8px;
    height: 8px;
    background: var(--text-faint);
    transition: all 0.15s;
  }
  .toggle.on .kn {
    background: rgba(255, 165, 36, 0.2);
    border-color: var(--amber-dim);
  }
  .toggle.on .kn::after {
    left: 11px;
    background: var(--amber);
  }
  .toggle em {
    font-style: normal;
    font-size: 10px;
    color: var(--text-faint);
  }
  .toggle.on em {
    color: var(--amber);
  }

  .out {
    background: var(--bg);
    display: grid;
    grid-template-rows: 1fr auto;
    gap: 1px;
    min-height: 0;
    overflow: auto;
  }
  .out :global(.panel) {
    min-height: 0;
  }
  .warns {
    display: flex;
    flex-direction: column;
    gap: 3px;
    margin-bottom: 10px;
  }
  .warn {
    background: rgba(232, 195, 74, 0.07);
    border: 1px solid var(--gold);
    color: var(--gold);
    padding: 4px 8px;
    font-size: 10.5px;
  }
  .ok {
    color: var(--green);
    font-size: 11px;
    margin-bottom: 10px;
  }
  .cmd-block {
    margin-bottom: 10px;
  }
  .cb-h {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 3px;
  }
  .cp {
    background: var(--bg-elev);
    border: 1px solid var(--border-bright);
    color: var(--text-dim);
    font-size: 9px;
    letter-spacing: 0.08em;
    padding: 2px 7px;
    cursor: pointer;
  }
  .cp:hover {
    color: var(--amber);
    border-color: var(--amber-dim);
  }
  pre {
    background: #050608;
    border: 1px solid var(--border);
    border-left: 2px solid var(--amber-dim);
    padding: 8px 10px;
    font-size: 11px;
    line-height: 1.5;
    color: var(--amber-soft);
    white-space: pre-wrap;
    word-break: break-all;
    overflow-x: auto;
  }
  pre.env {
    color: var(--text-dim);
    border-left-color: var(--border-bright);
    max-height: 150px;
    overflow: auto;
  }
  .actions {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
    margin-top: 8px;
  }
  .btn.full {
    justify-content: center;
  }
  .btn.spawn {
    justify-content: center;
    border-color: var(--green-dim);
    color: var(--green);
  }
  .note {
    margin-top: 8px;
    font-size: 9.5px;
    color: var(--text-faint);
    line-height: 1.5;
  }
  .jobs-empty {
    font-size: 11px;
    color: var(--text-dim);
    line-height: 1.6;
  }
  .faint {
    color: var(--text-faint);
  }
</style>
