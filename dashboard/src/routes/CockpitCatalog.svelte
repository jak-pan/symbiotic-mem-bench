<script lang="ts">
  import { store } from "../lib/store.svelte";

  const ranked = $derived(store.snapshot?.cohorts.flatMap((cohort) => cohort.rows) ?? store.runs.filter((run) => run.eligibility?.eligible));
  const artifacts = $derived([...new Set(ranked.flatMap((run) => run.artifacts_available))].sort());

  function runCountForSystem(system: string) { return ranked.filter((run) => run.system === system).length; }
  function runCountForBenchmark(benchmark: string) { return ranked.filter((run) => run.benchmark === benchmark).length; }
</script>

<aside class="rail">
  <div class="rail-head"><span>CATALOG</span><b>F4</b></div>
  <div class="rail-section"><span class="label">REGISTERED SYSTEMS</span><b>{store.systems.length}</b></div>
  <div class="rail-section"><span class="label">REGISTERED BENCHMARKS</span><b>{store.benchmarks.length}</b></div>
  <div class="rail-section"><span class="label">PUBLIC ARTIFACT CLASSES</span><b>{artifacts.length}</b></div>
  <p class="rail-note">The static canary lists only identities observed in its published export. Planned adapters and benchmarks are not fabricated into the registry.</p>
</aside>

<section class="work">
  <div class="subtabs" role="tablist" aria-label="Catalog feature availability">
    <button role="tab" aria-selected="true">REGISTRY</button>
    <button role="tab" disabled title="Dedicated capability view is not yet connected">CAPABILITIES</button>
    <button role="tab" disabled title="Dedicated dataset view is not yet connected">DATASETS</button>
    <button role="tab" disabled title="Dedicated contract view is not yet connected">CONTRACTS</button>
  </div>
  <div class="scroll">
    <div class="hero">
      <div><span class="eyebrow">AGNOSTIC REGISTRY</span><h1>Systems × benchmarks × evidence</h1><p>The catalog reflects the loaded publication snapshot, not prototype inventory.</p></div>
      <span class="truth">OBSERVED IDENTITIES ONLY</span>
    </div>
    <div class="cols">
      <section class="panel">
        <header><span>SYSTEMS / ADAPTERS</span><b>{store.systems.length} observed</b></header>
        <table><thead><tr><th>NAME</th><th>STATUS</th><th class="num">RANKED RUNS</th></tr></thead><tbody>
          {#each store.systems as system}<tr><td><strong>{system}</strong></td><td><span class="live">● OBSERVED</span></td><td class="num">{runCountForSystem(system)}</td></tr>{/each}
        </tbody></table>
      </section>
      <section class="panel">
        <header><span>BENCHMARKS</span><b>{store.benchmarks.length} observed</b></header>
        <table><thead><tr><th>NAME</th><th>STATUS</th><th class="num">RANKED RUNS</th></tr></thead><tbody>
          {#each store.benchmarks as benchmark}<tr><td><strong>{benchmark}</strong></td><td><span class="live">● OBSERVED</span></td><td class="num">{runCountForBenchmark(benchmark)}</td></tr>{/each}
        </tbody></table>
      </section>
    </div>
    <section class="panel coverage">
      <header><span>PORTABLE EVIDENCE COVERAGE</span><b>across verified rows</b></header>
      <div class="artifact-grid">
        {#each artifacts as artifact}<div><span>✓</span><b>{artifact.replaceAll("_", " ")}</b><small>{ranked.filter((run) => run.artifacts_available.includes(artifact)).length}/{ranked.length} verified records</small></div>{/each}
        {#if !artifacts.length}<p>No verified artifact manifest loaded.</p>{/if}
      </div>
    </section>
    <section class="panel rules">
      <header><span>CATALOG INVARIANTS</span><b>v2 integration boundary</b></header>
      <div class="rule-grid">
        <div><span>01</span><b>Adapters preserve native pipelines</b><p>No benchmark-owned duplicate memory implementation.</p></div>
        <div><span>02</span><b>Benchmarks declare cohort identity</b><p>Dataset fingerprint, scale, judge and prompt mode lock comparison.</p></div>
        <div><span>03</span><b>Artifacts are portable</b><p>Repo-relative evidence, no local paths, secrets or raw private prompts.</p></div>
        <div><span>04</span><b>Unknown means unavailable</b><p>The UI never fills a missing capability with prototype data.</p></div>
      </div>
    </section>
  </div>
</section>

<style>
  .rail { width: 252px; flex: none; border-right: 1px solid var(--border); background: var(--bg-panel); }
  .rail-head { display: flex; justify-content: space-between; padding: 9px 12px; border-bottom: 1px solid var(--border); color: var(--text-faint); font-family: var(--sans); font-size: 9.5px; font-weight: 700; letter-spacing: .13em; }
  .rail-head b { color: var(--amber); }
  .rail-section { display: flex; justify-content: space-between; align-items: center; padding: 12px; border-bottom: 1px solid var(--border); }
  .rail-section b { color: var(--amber); font-size: 18px; }.rail-note { padding: 12px; color: var(--text-dim); font-size: 10px; line-height: 1.5; }
  .work { min-width: 0; flex: 1; display: flex; flex-direction: column; overflow: hidden; }
  .subtabs { height: 34px; flex: none; display: flex; gap: 3px; padding: 0 7px; border-bottom: 1px solid var(--border); background: var(--bg-panel); }
  .subtabs button { display: flex; align-items: center; padding: 0 11px; background: none; border: 0; border-bottom: 2px solid transparent; color: var(--text-dim); font-family: var(--sans); font-size: 9px; font-weight: 700; letter-spacing: .1em; }
  .subtabs button[aria-selected="true"] { color: var(--amber); border-bottom-color: var(--amber); }
  .subtabs button:disabled { color: #747d89; cursor: not-allowed; }
  .scroll { flex: 1; overflow-y: auto; padding: 12px; }
  .hero { display: flex; justify-content: space-between; align-items: center; padding: 13px 14px; border: 1px solid var(--border-bright); background: linear-gradient(100deg, rgba(47,207,122,.07), transparent 46%), var(--bg-panel); }
  .eyebrow { color: var(--green); font-family: var(--sans); font-size: 8px; font-weight: 800; letter-spacing: .17em; }
  h1 { margin-top: 2px; font-size: 20px; }.hero p { margin-top: 4px; color: var(--text-dim); font-size: 10px; }
  .truth { padding: 4px 8px; border: 1px solid var(--green-dim); color: var(--green); font-size: 8px; font-weight: 700; letter-spacing: .1em; }
  .cols { display: grid; grid-template-columns: 1fr 1fr; gap: 10px; margin-top: 10px; }
  .panel { border: 1px solid var(--border); background: var(--bg-panel); }
  .panel header { display: flex; justify-content: space-between; padding: 8px 10px; border-bottom: 1px solid var(--border); }
  .panel header span { font-family: var(--sans); font-size: 9px; font-weight: 700; letter-spacing: .11em; }.panel header b { color: var(--amber); font-size: 8px; font-weight: 500; text-transform: uppercase; }
  table { width: 100%; border-collapse: collapse; }th, td { padding: 8px 10px; border-bottom: 1px solid var(--border); text-align: left; }th { color: var(--text-faint); font-family: var(--sans); font-size: 8px; letter-spacing: .08em; }td strong { color: var(--text); }.num { text-align: right; }.live { color: var(--green); font-size: 8px; }
  .coverage, .rules { margin-top: 10px; }
  .artifact-grid { display: grid; grid-template-columns: repeat(4, 1fr); gap: 1px; padding: 1px; background: var(--border); }
  .artifact-grid div { padding: 10px; background: var(--bg-elev); }.artifact-grid span { color: var(--green); }.artifact-grid b, .artifact-grid small { display: block; }.artifact-grid b { margin-top: 3px; color: var(--text); font-size: 9px; text-transform: uppercase; }.artifact-grid small { margin-top: 2px; color: var(--text-faint); font-size: 8px; }.artifact-grid p { padding: 12px; color: var(--text-faint); }
  .rule-grid { display: grid; grid-template-columns: repeat(4, 1fr); gap: 1px; background: var(--border); }.rule-grid div { padding: 12px; background: var(--bg-panel); }.rule-grid span { color: var(--amber); }.rule-grid b { display: block; margin-top: 4px; color: var(--text); font-size: 10px; }.rule-grid p { margin-top: 5px; color: var(--text-dim); font-size: 9px; line-height: 1.45; }
</style>
