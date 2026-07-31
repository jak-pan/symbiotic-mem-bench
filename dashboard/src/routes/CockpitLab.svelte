<script lang="ts">
  import { store } from "../lib/store.svelte";

  let system = $state("");
  let benchmark = $state("");
  let limit = $state("500");

  const selectedSystem = $derived(system || store.systems[0] || "no system registered");
  const selectedBenchmark = $derived(benchmark || store.benchmarks[0] || "no benchmark registered");
  const canLaunch = $derived(store.online && false);
</script>

<aside class="rail">
  <div class="rail-head"><span>LAB WORKSPACE</span><b>F3</b></div>
  <div class="rail-section">
    <span class="label">RUN DESIGN</span>
    <button class="active">New benchmark run</button>
    <button disabled>Resume interrupted run</button>
    <button disabled>Answer-only trial</button>
  </div>
  <div class="rail-section">
    <span class="label">EVIDENCE POLICY</span>
    <p>Every launch must emit portable params, report and public artifacts before it can be reviewed.</p>
  </div>
</aside>

<section class="work">
  <div class="subtabs"><span class="active">CONFIGURE</span><span>FLEET</span><span>ABLATIONS</span><span>STATE</span></div>
  <div class="scroll">
    <div class="hero">
      <div><span class="eyebrow">EXPERIMENT WORKBENCH</span><h1>Design an evidence-producing run</h1><p>The static public canary exposes the contract, never a fake launch.</p></div>
      <span class="backend" class:live={store.online}>{store.online ? "LIVE REGISTRY · LAUNCH API NOT WIRED" : "STATIC CANARY · READ ONLY"}</span>
    </div>

    <div class="layout">
      <section class="panel config">
        <header><span>RUN CONFIGURATION</span><b>adapter × benchmark</b></header>
        <div class="form">
          <label><span>SYSTEM / ADAPTER</span>
            <select bind:value={system}>
              {#each store.systems as value}<option value={value}>{value}</option>{/each}
            </select>
          </label>
          <label><span>BENCHMARK</span>
            <select bind:value={benchmark}>
              {#each store.benchmarks as value}<option value={value}>{value}</option>{/each}
            </select>
          </label>
          <label><span>QUESTION LIMIT</span><input bind:value={limit} inputmode="numeric" /></label>
          <label><span>RUN MODE</span><select disabled><option>fresh · full pipeline</option></select></label>
        </div>
        <div class="resolver">
          <span>RESOLUTION</span><b>{selectedSystem} × {selectedBenchmark}</b><em>Requires a live execution backend and explicit provider configuration.</em>
        </div>
        <button class="launch" disabled={!canLaunch}>LAUNCH RUN</button>
        <p class="why">Launch is intentionally disabled: neither the static Netlify deployment nor the current public API exposes a safe run-creation endpoint. This screen will become active only when that backend contract exists.</p>
      </section>

      <section class="panel">
        <header><span>OUTPUT CONTRACT</span><b>required for review</b></header>
        <div class="contract">
          <div><span>01</span><b>run-params.json</b><p>System, benchmark, scale, model and cohort identity.</p></div>
          <div><span>02</span><b>benchmark-report.json</b><p>Aggregate measurement and executor status.</p></div>
          <div><span>03</span><b>artifacts/</b><p>Question-level hypotheses, verdicts, traces and score evidence.</p></div>
          <div><span>04</span><b>independent review</b><p>Agent or human attestation required before ranking.</p></div>
        </div>
      </section>
    </div>

    <section class="panel pipeline">
      <header><span>APPEND-ONLY STEP ENVELOPE</span><b>planned execution flow</b></header>
      <div class="stages">
        <div><small>1</small><b>INGEST</b><span>source → adapter</span></div><i>›</i>
        <div><small>2</small><b>CAPTURE</b><span>native memory writes</span></div><i>›</i>
        <div><small>3</small><b>RECALL</b><span>query → evidence</span></div><i>›</i>
        <div><small>4</small><b>ANSWER</b><span>hypothesis + trace</span></div><i>›</i>
        <div><small>5</small><b>JUDGE</b><span>verdict + cohort</span></div><i>›</i>
        <div><small>6</small><b>REVIEW</b><span>rank or hold back</span></div>
      </div>
    </section>
  </div>
</section>

<style>
  .rail { width: 252px; flex: none; border-right: 1px solid var(--border); background: var(--bg-panel); }
  .rail-head { display: flex; justify-content: space-between; padding: 9px 12px; border-bottom: 1px solid var(--border); color: var(--text-faint); font-family: var(--sans); font-size: 9.5px; font-weight: 700; letter-spacing: .13em; }
  .rail-head b { color: var(--amber); }
  .rail-section { padding: 12px; border-bottom: 1px solid var(--border); }
  .rail-section button { width: 100%; display: block; padding: 7px 8px; cursor: pointer; text-align: left; background: none; border: 0; color: var(--text-faint); font-size: 10px; }
  .rail-section button.active { margin-top: 7px; background: var(--bg-sel); box-shadow: inset 2px 0 0 var(--amber); color: var(--text); }
  .rail-section button:disabled { opacity: .45; cursor: not-allowed; }
  .rail-section p { margin-top: 7px; color: var(--text-dim); font-size: 10px; line-height: 1.5; }
  .work { min-width: 0; flex: 1; display: flex; flex-direction: column; overflow: hidden; }
  .subtabs { height: 34px; flex: none; display: flex; gap: 3px; padding: 0 7px; border-bottom: 1px solid var(--border); background: var(--bg-panel); }
  .subtabs span { display: flex; align-items: center; padding: 0 11px; border-bottom: 2px solid transparent; color: var(--text-faint); font-family: var(--sans); font-size: 9px; font-weight: 700; letter-spacing: .1em; }
  .subtabs .active { color: var(--amber); border-bottom-color: var(--amber); }
  .scroll { flex: 1; overflow-y: auto; padding: 12px; }
  .hero { display: flex; justify-content: space-between; align-items: center; padding: 13px 14px; border: 1px solid var(--border-bright); background: linear-gradient(100deg, rgba(178,133,255,.08), transparent 46%), var(--bg-panel); }
  .eyebrow { color: var(--violet); font-family: var(--sans); font-size: 8px; font-weight: 800; letter-spacing: .17em; }
  h1 { margin-top: 2px; font-size: 20px; }.hero p { margin-top: 4px; color: var(--text-dim); font-size: 10px; }
  .backend { padding: 4px 8px; border: 1px solid var(--amber-dim); color: var(--amber); font-size: 8px; font-weight: 700; letter-spacing: .1em; }
  .backend.live { border-color: var(--green-dim); color: var(--green); }
  .layout { display: grid; grid-template-columns: 1.1fr .9fr; gap: 10px; margin-top: 10px; }
  .panel { border: 1px solid var(--border); background: var(--bg-panel); }
  .panel header { display: flex; justify-content: space-between; padding: 8px 10px; border-bottom: 1px solid var(--border); }
  .panel header span { font-family: var(--sans); font-size: 9px; font-weight: 700; letter-spacing: .11em; }
  .panel header b { color: var(--amber); font-size: 8px; font-weight: 500; text-transform: uppercase; }
  .form { display: grid; grid-template-columns: 1fr 1fr; gap: 10px; padding: 12px; }
  label span { display: block; margin-bottom: 4px; color: var(--text-faint); font-family: var(--sans); font-size: 8px; font-weight: 700; letter-spacing: .1em; }
  select, input { width: 100%; padding: 6px 8px; outline: none; border: 1px solid var(--border-bright); background: var(--bg); color: var(--text); }
  select:focus, input:focus { border-color: var(--amber-dim); }
  .resolver { margin: 0 12px 10px; padding: 9px; border: 1px solid var(--border); background: var(--bg-elev); }
  .resolver span, .resolver b, .resolver em { display: block; }
  .resolver span { color: var(--text-faint); font-family: var(--sans); font-size: 8px; font-weight: 700; letter-spacing: .1em; }
  .resolver b { margin-top: 3px; color: var(--cyan); }.resolver em { margin-top: 3px; color: var(--text-dim); font-size: 9px; font-style: normal; }
  .launch { margin: 0 12px; padding: 7px 12px; border: 1px solid var(--amber-dim); background: rgba(255,165,36,.1); color: var(--amber); font-size: 10px; font-weight: 700; }
  .launch:disabled { opacity: .4; cursor: not-allowed; }
  .why { padding: 8px 12px 12px; color: var(--text-faint); font-size: 9px; line-height: 1.45; }
  .contract { padding: 4px 12px; }
  .contract div { display: grid; grid-template-columns: 28px 130px 1fr; gap: 8px; align-items: center; padding: 10px 0; border-bottom: 1px solid var(--border); }
  .contract span { color: var(--amber); }.contract b { color: var(--text); font-size: 10px; }.contract p { color: var(--text-dim); font-size: 9px; }
  .pipeline { margin-top: 10px; }
  .stages { display: flex; align-items: stretch; padding: 12px; }
  .stages div { flex: 1; min-width: 0; padding: 10px; border: 1px solid var(--border); background: var(--bg-elev); }
  .stages small, .stages b, .stages span { display: block; }.stages small { color: var(--text-faint); }.stages b { margin-top: 2px; color: var(--amber); }.stages span { margin-top: 3px; color: var(--text-dim); font-size: 8px; }
  .stages i { display: flex; align-items: center; color: var(--text-faint); font-size: 20px; font-style: normal; }
</style>
