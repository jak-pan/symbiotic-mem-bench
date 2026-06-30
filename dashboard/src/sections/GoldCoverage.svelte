<script lang="ts">
  import { api } from "../lib/api";
  import type {
    GoldClass,
    GoldEvalResponse,
    GoldEvalQuestion,
    GoldRankDistribution,
    RunSummary,
  } from "../lib/types";
  import { pct, qtypeAbbr } from "../lib/format";
  import { createAsyncData } from "../lib/async.svelte";
  import Panel from "../components/Panel.svelte";

  let { id }: { id: string } = $props();
  const ad = createAsyncData<GoldEvalResponse>();
  let error = $state<string | null>(null);

  $effect(() => {
    const runId = id;
    ad.reset();
    error = null;
    api
      .goldEval(runId)
      .then((d) => {
        if (runId !== id) return; // user switched runs mid-flight
        ad.set(d);
      })
      .catch((e) => {
        if (runId !== id) return;
        error = e instanceof Error ? e.message : String(e);
        ad.set(null as unknown as GoldEvalResponse);
      });
  });

  const data = $derived(ad.data);
  const loading = $derived(ad.loading);
  const summary = $derived(data?.summary);

  // ── Embedding-vs-rerank: deepest gold-turn retrieval rank ───────────────────
  // The bar buckets (top-N recall of the deepest gold turn) shared by the
  // current-run view and the cross-run comparison.
  const RANK_BUCKETS = [
    { key: "within_10", label: "≤10" },
    { key: "within_20", label: "≤20" },
    { key: "within_50", label: "≤50" },
    { key: "within_100", label: "≤100" },
  ] as const;

  const goldRank = $derived(summary?.gold_rank);

  function distVal(d: GoldRankDistribution | undefined, key: string): number | null {
    if (!d) return null;
    const v = (d as unknown as Record<string, number>)[key];
    return typeof v === "number" ? v : null;
  }

  // Cross-run comparison: the named reference runs to line up beside the
  // selected one. Resolved name → run_id against the registry, then each run's
  // gold-eval.json is fetched and cached by run_id.
  const COMPARE_RUN_NAMES = ["c500-coh-1", "nemo-rpmfix-500", "pplx-rpmfix-500"];

  // Compact label for a run in the heatmap / tail rows (the task's `qwen · embed`
  // shape). Explicit aliases for the named reference runs, else a trimmed tail.
  const RUN_SHORT: Record<string, string> = {
    "c500-coh-1": "qwen",
    "nemo-rpmfix-500": "nemo",
    "pplx-rpmfix-500": "pplx",
  };
  function shortRun(name: string): string {
    return RUN_SHORT[name] ?? name.replace(/-rpmfix-\d+$|-\d+$/, "");
  }

  type CompareRow = {
    name: string;
    runId: string | null;
    rank: GoldEvalResponse["summary"]["gold_rank"] | null;
    // Per-question deepest-gold-turn ranks (raw-only), nulls dropped. Feed the
    // heatmap + tail percentiles, which the summary doesn't carry.
    embedRanks: number[];
    rerankRanks: number[];
    error: string | null;
  };

  let runList = $state<RunSummary[]>([]);
  let compareRows = $state<CompareRow[]>([]);
  // Which runs are visible in the side-by-side (default: all that have data).
  let selectedCompare = $state<Set<string>>(new Set(COMPARE_RUN_NAMES));
  let compareLoading = $state(false);

  function resolveRunId(name: string): string | null {
    // Prefer an exact run_name match; fall back to a run_id whose last path
    // segment is the name (registry ids are repo-relative paths).
    const byName = runList.find((r) => r.run_name === name);
    if (byName) return byName.run_id;
    const byTail = runList.find((r) => r.run_id.split("/").pop() === name);
    return byTail ? byTail.run_id : null;
  }

  // Load the registry once, then fetch each comparison run's gold-eval.
  $effect(() => {
    let cancelled = false;
    compareLoading = true;
    api
      .runs()
      .then(async (runs) => {
        if (cancelled) return;
        runList = runs;
        const rows = await Promise.all(
          COMPARE_RUN_NAMES.map(async (name): Promise<CompareRow> => {
            const runId = resolveRunId(name);
            if (!runId)
              return {
                name,
                runId: null,
                rank: null,
                embedRanks: [],
                rerankRanks: [],
                error: "not in registry",
              };
            try {
              const d = await api.goldEval(runId);
              const qs = d.questions ?? [];
              return {
                name,
                runId,
                rank: d.summary.gold_rank ?? null,
                embedRanks: qs
                  .map((q) => q.gold_embed_rank)
                  .filter((v): v is number => typeof v === "number"),
                rerankRanks: qs
                  .map((q) => q.gold_rerank_rank)
                  .filter((v): v is number => typeof v === "number"),
                error: null,
              };
            } catch (e) {
              return {
                name,
                runId,
                rank: null,
                embedRanks: [],
                rerankRanks: [],
                error: e instanceof Error ? e.message : String(e),
              };
            }
          }),
        );
        if (!cancelled) compareRows = rows;
      })
      .catch(() => {
        if (!cancelled) compareRows = [];
      })
      .finally(() => {
        if (!cancelled) compareLoading = false;
      });
    return () => {
      cancelled = true;
    };
  });

  const visibleCompare = $derived(
    compareRows.filter((r) => selectedCompare.has(r.name)),
  );

  function toggleCompare(name: string) {
    const next = new Set(selectedCompare);
    if (next.has(name)) next.delete(name);
    else next.add(name);
    selectedCompare = next;
  }

  // ── Rank-distribution heatmap + tail percentiles ────────────────────────────
  // Both views are computed in the frontend from each run's per-question
  // gold_embed_rank / gold_rerank_rank arrays (the deepest gold turn's raw-only
  // rank), so the tail — not the mean — is what's on display.
  const RANK_BINS = [
    { label: "=1", lo: 1, hi: 1 },
    { label: "2–3", lo: 2, hi: 3 },
    { label: "4–5", lo: 4, hi: 5 },
    { label: "6–10", lo: 6, hi: 10 },
    { label: "11–20", lo: 11, hi: 20 },
    { label: "21–30", lo: 21, hi: 30 },
    { label: "31–50", lo: 31, hi: 50 },
    { label: "51–100", lo: 51, hi: 100 },
    { label: "101+", lo: 101, hi: Infinity },
  ] as const;

  type Stage = "embed" | "rerank";

  /** Nearest-rank percentile (p in 0–100) over an unsorted numeric array. */
  function percentile(arr: number[], p: number): number | null {
    if (!arr.length) return null;
    const s = [...arr].sort((a, b) => a - b);
    const idx = Math.min(s.length - 1, Math.max(0, Math.ceil((p / 100) * s.length) - 1));
    return s[idx];
  }

  /** % of `arr` whose value lands in each RANK_BINS bucket (rounded integer). */
  function binPcts(arr: number[]): number[] {
    const n = arr.length;
    return RANK_BINS.map((b) => {
      if (!n) return 0;
      const c = arr.filter((v) => v >= b.lo && v <= b.hi).length;
      return Math.round((c / n) * 100);
    });
  }

  type HeatRow = { run: string; stage: Stage; n: number; cells: number[] };

  const heatRows = $derived.by<HeatRow[]>(() => {
    const out: HeatRow[] = [];
    for (const cr of visibleCompare) {
      const run = shortRun(cr.name);
      out.push({ run, stage: "embed", n: cr.embedRanks.length, cells: binPcts(cr.embedRanks) });
      out.push({ run, stage: "rerank", n: cr.rerankRanks.length, cells: binPcts(cr.rerankRanks) });
    }
    return out;
  });

  // Median n across visible run/stages — the "n ≈ X / run" footnote.
  const heatN = $derived.by<number | null>(() => {
    const ns = heatRows.map((r) => r.n).filter((n) => n > 0);
    return ns.length ? percentile(ns, 50) : null;
  });

  type TailStat = {
    run: string;
    stage: Stage;
    n: number;
    p50: number | null;
    p95: number | null;
    p98: number | null;
    max: number | null;
  };

  function tailFor(run: string, stage: Stage, arr: number[]): TailStat {
    return {
      run,
      stage,
      n: arr.length,
      p50: percentile(arr, 50),
      p95: percentile(arr, 95),
      p98: percentile(arr, 98),
      max: arr.length ? Math.max(...arr) : null,
    };
  }

  const tailStats = $derived.by<TailStat[]>(() => {
    const out: TailStat[] = [];
    for (const cr of visibleCompare) {
      const run = shortRun(cr.name);
      out.push(tailFor(run, "embed", cr.embedRanks));
      out.push(tailFor(run, "rerank", cr.rerankRanks));
    }
    return out;
  });

  // Sequential blue cell, dark-mode aware: faint floor + intensity ramp to ~35%.
  // `pct` is the bin's rounded percent (0–100).
  function heatBg(p: number): string {
    const t = Math.min(p / 35, 1);
    return `rgba(55,138,221,${(0.05 + t * 0.9).toFixed(3)})`;
  }
  function heatNorm(p: number): number {
    return Math.min(p / 35, 1);
  }

  // Class filter for the per-question table — one click to the reader-fail
  // worklist ("gold present, reader still wrong").
  const CLASSES: GoldClass[] = ["correct", "reader_fail", "retrieval_gap"];
  let classFilter = $state<GoldClass | "all">("all");

  const rows = $derived.by<GoldEvalQuestion[]>(() => {
    const all = data?.questions ?? [];
    const filtered =
      classFilter === "all" ? all : all.filter((q) => q.class === classFilter);
    // Surface the actionable misses first: gaps, then reader-fails, then correct;
    // within a class, the deepest cut (hardest retrieval) on top.
    const order: Record<GoldClass, number> = { retrieval_gap: 0, reader_fail: 1, correct: 2 };
    return [...filtered].sort((a, b) => {
      const byClass = order[a.class] - order[b.class];
      if (byClass !== 0) return byClass;
      return (b.gold_deepest_rank ?? 0) - (a.gold_deepest_rank ?? 0);
    });
  });

  function answerText(value: unknown): string {
    if (value === null || value === undefined) return "—";
    if (typeof value === "string") return value;
    return JSON.stringify(value);
  }

  const classLabel: Record<GoldClass, string> = {
    correct: "correct",
    reader_fail: "reader fail",
    retrieval_gap: "retrieval gap",
  };
</script>

{#if loading}
  <div class="load">LOADING GOLD COVERAGE…</div>
{:else if error || !data || !summary}
  <div class="gc">
    <div class="none">
      NO GOLD-EVAL ARTIFACT — run <code>membench gold-eval --run {id}</code> to generate
      <code>artifacts/gold-eval.json</code>.{#if error}<span class="errmsg"> ({error})</span>{/if}
    </div>
  </div>
{:else}
  <div class="gc fade-in">
    <Panel title="Gold Coverage" tag="{summary.total} questions · {data.run_name}" flush>
      <div class="strip">
        <!-- Classification split: where the misses live. -->
        <div class="card">
          <div class="card-h">CLASSIFICATION</div>
          <div class="bars">
            {#each CLASSES as c (c)}
              <button
                class="cbar {c}"
                class:sel={classFilter === c}
                onclick={() => (classFilter = classFilter === c ? "all" : c)}
                title="Filter the table to {classLabel[c]}"
              >
                <span class="cnum">{summary.class_counts[c]}</span>
                <span class="clbl">{classLabel[c]}</span>
              </button>
            {/each}
          </div>
          <div class="hint">
            {summary.correct} correct · {summary.wrong} wrong · {summary.abstained} abstained
          </div>
        </div>

        <!-- Piece coverage (multi-piece scope; single-piece trivially covers). -->
        <div class="card">
          <div class="card-h">GOLD PIECE COVERAGE</div>
          <div class="big mono-num">{pct(summary.piece_coverage)}%</div>
          <div class="hint">
            {summary.gold_pieces_covered}/{summary.gold_pieces_needed} pieces (multi-piece) captured by distilled facts
          </div>
        </div>

        <!-- How each gold piece is covered: distill fact vs raw turn. -->
        <div class="card">
          <div class="card-h">COVERAGE BY SOURCE</div>
          <div class="src">
            <span class="schip both">both {summary.coverage_by_source.both}</span>
            <span class="schip fact">fact {summary.coverage_by_source.fact}</span>
            <span class="schip raw">raw {summary.coverage_by_source.raw}</span>
            <span class="schip none">none {summary.coverage_by_source.none}</span>
          </div>
          <div class="hint">
            per gold piece across all questions · raw-only = distill missed, masked by raw turns
          </div>
        </div>

        <!-- Question shape. -->
        <div class="card">
          <div class="card-h">QUESTION SHAPE</div>
          <div class="shape">
            <span class="schip">single {summary.single_piece}</span>
            <span class="schip">multi {summary.multi_piece}</span>
          </div>
          <div class="hint">multi-piece questions need every gold session retrieved</div>
        </div>
      </div>
    </Panel>

    <!-- Embedding vs rerank: where the deepest gold TURN lands after embedding
         vs after the reranker, ranked among raw-turn candidates only so runs
         compare fairly (merged and separate traces are normalized). -->
    <Panel
      title="Embedding vs Rerank"
      tag={goldRank
        ? `deepest gold turn · ${goldRank.embed.n} in-set Qs`
        : "no rank data"}
      flush
    >
      {#if !goldRank}
        <div class="hint pad">
          No <code>gold_rank</code> in this artifact — regenerate with
          <code>membench gold-eval --run {data.run_name}</code>.
        </div>
      {:else}
        <div class="evr">
          <div class="evr-head">
            <span class="evr-cap">DEEPEST GOLD-TURN RANK · TOP-N RECALL</span>
            <span class="evr-inset"
              >in candidate set {pct(goldRank.gold_turn_in_set_pct)}% ({goldRank.gold_turns_in_set}/{goldRank.gold_turns_total}
              turns)</span
            >
          </div>
          <table class="evrgrid">
            <thead>
              <tr>
                <th>BUCKET</th>
                <th class="num emb">EMBED</th>
                <th class="num rrk">RERANK</th>
                <th class="num">Δ</th>
                <th class="bars-h"></th>
              </tr>
            </thead>
            <tbody>
              {#each RANK_BUCKETS as b (b.key)}
                {@const e = distVal(goldRank.embed, b.key) ?? 0}
                {@const r = distVal(goldRank.rerank, b.key) ?? 0}
                <tr>
                  <td class="dim">top {b.label}</td>
                  <td class="num mono-num emb">{pct(e)}%</td>
                  <td class="num mono-num rrk">{pct(r)}%</td>
                  <td
                    class="num mono-num"
                    class:up={r > e}
                    class:down={r < e}>{r > e ? "+" : ""}{pct(r - e)}</td
                  >
                  <td class="bars-c">
                    <div class="mini">
                      <div class="mfill emb" style="width:{e * 100}%"></div>
                    </div>
                    <div class="mini">
                      <div class="mfill rrk" style="width:{r * 100}%"></div>
                    </div>
                  </td>
                </tr>
              {/each}
              <tr class="meanrow">
                <td class="dim">mean rank</td>
                <td class="num mono-num emb">{goldRank.embed.mean.toFixed(1)}</td>
                <td class="num mono-num rrk">{goldRank.rerank.mean.toFixed(1)}</td>
                <td
                  class="num mono-num"
                  class:up={goldRank.rerank.mean < goldRank.embed.mean}
                  >{(goldRank.rerank.mean - goldRank.embed.mean).toFixed(1)}</td
                >
                <td class="dim small">lower = gold ranked higher</td>
              </tr>
            </tbody>
          </table>
        </div>

        <!-- Cross-run comparison: line the named reference runs up side-by-side. -->
        <div class="cmp">
          <div class="cmp-head">
            <span class="evr-cap">RUN COMPARISON</span>
            <div class="filt">
              {#each compareRows as cr (cr.name)}
                <button
                  class="ftab"
                  class:on={selectedCompare.has(cr.name)}
                  class:dis={!cr.rank}
                  disabled={!cr.rank}
                  title={cr.error
                    ? `${cr.name}: ${cr.error}`
                    : `Toggle ${cr.name}`}
                  onclick={() => toggleCompare(cr.name)}>{cr.name}</button
                >
              {/each}
            </div>
          </div>
          {#if compareLoading && !compareRows.length}
            <div class="hint pad">loading runs…</div>
          {:else if !visibleCompare.length}
            <div class="hint pad">No runs selected.</div>
          {:else}
            <table class="cmpgrid">
              <thead>
                <tr>
                  <th>RUN</th>
                  <th class="num">METRIC</th>
                  {#each RANK_BUCKETS as b (b.key)}<th class="num"
                      >top {b.label}</th
                    >{/each}
                  <th class="num">mean</th>
                  <th class="num">in-set</th>
                </tr>
              </thead>
              <tbody>
                {#each visibleCompare as cr (cr.name)}
                  {#if cr.rank}
                    <tr class="cmp-run emb-row">
                      <td class="rn" rowspan="2">{cr.name}</td>
                      <td class="num lbl emb">embed</td>
                      {#each RANK_BUCKETS as b (b.key)}<td class="num mono-num"
                          >{pct(distVal(cr.rank.embed, b.key))}</td
                        >{/each}
                      <td class="num mono-num">{cr.rank.embed.mean.toFixed(1)}</td>
                      <td class="num mono-num dim" rowspan="2"
                        >{pct(cr.rank.gold_turn_in_set_pct)}%</td
                      >
                    </tr>
                    <tr class="cmp-run rrk-row">
                      <td class="num lbl rrk">rerank</td>
                      {#each RANK_BUCKETS as b (b.key)}<td class="num mono-num"
                          >{pct(distVal(cr.rank.rerank, b.key))}</td
                        >{/each}
                      <td class="num mono-num">{cr.rank.rerank.mean.toFixed(1)}</td>
                    </tr>
                  {/if}
                {/each}
              </tbody>
            </table>
          {/if}
          <div class="hint pad">
            % of questions whose deepest gold turn (worst of its gold turns)
            ranks within top-N · embed = by embedding score, rerank = by rerank
            score, raw-turn candidates only
          </div>
        </div>

        <!-- Tail percentiles: p50/p95/p98/max of the per-question deepest-gold-turn
             rank. The tail (not the mean) is what the reader pays for. -->
        <div class="cmp">
          <div class="cmp-head">
            <span class="evr-cap">RANK TAIL · p50 / p95 / p98 / MAX</span>
            <span class="evr-inset">lower = gold ranked higher · raw-only</span>
          </div>
          {#if !tailStats.length}
            <div class="hint pad">No runs selected.</div>
          {:else}
            <table class="evrgrid tail">
              <thead>
                <tr>
                  <th>RUN · STAGE</th>
                  <th class="num">n</th>
                  <th class="num">p50</th>
                  <th class="num">p95</th>
                  <th class="num">p98</th>
                  <th class="num">max</th>
                </tr>
              </thead>
              <tbody>
                {#each tailStats as t (t.run + t.stage)}
                  <tr class:rrk-sep={t.stage === "rerank"}>
                    <td class="tlbl">
                      <span class="rn">{t.run}</span>
                      <span class="stage {t.stage === 'embed' ? 'emb' : 'rrk'}"
                        >{t.stage}</span
                      >
                    </td>
                    <td class="num mono-num dim">{t.n}</td>
                    <td class="num mono-num">{t.p50 ?? "—"}</td>
                    <td class="num mono-num">{t.p95 ?? "—"}</td>
                    <td class="num mono-num">{t.p98 ?? "—"}</td>
                    <td class="num mono-num dim">{t.max ?? "—"}</td>
                  </tr>
                {/each}
              </tbody>
            </table>
          {/if}
        </div>

        <!-- Rank-distribution heatmap: each row is a run × stage, each column a
             rank bin, each cell the % of that row's questions in the bin. -->
        <div class="cmp">
          <div class="cmp-head">
            <span class="evr-cap">RANK DISTRIBUTION · % OF QUESTIONS PER BIN</span>
            <div class="hmlegend">
              <span class="hm-lo">low</span>
              <span class="hm-ramp"></span>
              <span class="hm-hi">high</span>
              {#if heatN !== null}<span class="hm-n">n ≈ {heatN} / run</span>{/if}
            </div>
          </div>
          {#if !heatRows.length}
            <div class="hint pad">No runs selected.</div>
          {:else}
            <div class="hmwrap">
              <table class="hmgrid">
                <thead>
                  <tr>
                    <th class="hm-rowh">RUN · STAGE</th>
                    {#each RANK_BINS as b (b.label)}<th class="num hm-colh"
                        >{b.label}</th
                      >{/each}
                  </tr>
                </thead>
                <tbody>
                  {#each heatRows as row (row.run + row.stage)}
                    <tr class:rrk-sep={row.stage === "rerank"}>
                      <td class="hm-rowl">
                        <span class="rn">{row.run}</span>
                        <span
                          class="stage {row.stage === 'embed' ? 'emb' : 'rrk'}"
                          >{row.stage}</span
                        >
                      </td>
                      {#each row.cells as p, i (i)}
                        <td
                          class="hm-cell mono-num"
                          class:intense={heatNorm(p) > 0.42}
                          style="background:{heatBg(p)}"
                          title="{row.run} · {row.stage} · {RANK_BINS[i].label}: {p}% ({row.n} Qs)"
                          >{p}</td
                        >
                      {/each}
                    </tr>
                  {/each}
                </tbody>
              </table>
            </div>
            <div class="hint pad">
              cell = count in bin / n, rounded · darker = more questions land at
              that rank · embed = by embedding score, rerank = by rerank score
            </div>
          {/if}
        </div>
      {/if}
    </Panel>

    <Panel title="Per-question" tag="{rows.length} shown" flush scroll>
      {#snippet actions()}
        <div class="filt">
          <button class="ftab" class:on={classFilter === "all"} onclick={() => (classFilter = "all")}>all</button>
          {#each CLASSES as c (c)}
            <button class="ftab {c}" class:on={classFilter === c} onclick={() => (classFilter = c)}>{classLabel[c]}</button>
          {/each}
        </div>
      {/snippet}
      <table class="grid">
        <thead>
          <tr>
            <th>QID</th>
            <th>Type</th>
            <th class="num">Gold</th>
            <th class="num">Cov</th>
            <th class="num">Top</th>
            <th class="num">Deep</th>
            <th class="num emb" title="Deepest gold turn's embed rank (raw-only)">E.rank</th>
            <th class="num rrk" title="Deepest gold turn's rerank rank (raw-only)">R.rank</th>
            <th>Correct</th>
            <th>Abst</th>
            <th>Class</th>
          </tr>
        </thead>
        <tbody>
          {#each rows as q (q.qid)}
            <tr class:miss={q.class !== "correct"}>
              <td class="mono-num dim" title={answerText(q.answer)}>{q.qid}</td>
              <td class="dim">{qtypeAbbr(q.type)}</td>
              <td class="num mono-num">{q.n_gold_pieces}</td>
              <td class="num mono-num" class:down={q.covered_pieces < q.n_gold_pieces}>{q.covered_pieces}</td>
              <td class="num mono-num dim">{q.gold_top_rank ?? "—"}</td>
              <td class="num mono-num dim">{q.gold_deepest_rank ?? "—"}</td>
              <td class="num mono-num emb">{q.gold_embed_rank ?? "—"}</td>
              <td class="num mono-num rrk">{q.gold_rerank_rank ?? "—"}</td>
              <td><span class:up={q.correct} class:down={!q.correct}>{q.correct ? "yes" : "no"}</span></td>
              <td class="dim">{q.abstained ? "yes" : "—"}</td>
              <td><span class="ctag {q.class}">{classLabel[q.class]}</span></td>
            </tr>
          {/each}
          {#if !rows.length}
            <tr><td colspan="11" class="empty">No questions match this filter.</td></tr>
          {/if}
        </tbody>
      </table>
    </Panel>
  </div>
{/if}

<style>
  .load {
    padding: 40px;
    color: var(--text-faint);
    letter-spacing: 0.2em;
  }
  .gc {
    padding: 10px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-width: 0;
  }
  .none {
    padding: 10px;
    background: var(--bg-panel);
    border: 1px solid var(--border-bright);
    color: var(--text-dim);
    font-size: 11px;
  }
  .none code {
    color: var(--amber);
    font-family: var(--mono);
  }
  .errmsg {
    color: var(--text-faint);
  }
  .strip {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
    gap: 8px;
    padding: 8px;
  }
  .card {
    border: 1px solid var(--border);
    background: var(--bg-elev);
    padding: 8px 10px;
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
  }
  .card-h {
    font-size: 9px;
    font-weight: 800;
    letter-spacing: 0.14em;
    color: var(--text-faint);
  }
  .bars {
    display: flex;
    gap: 4px;
  }
  .cbar {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
    padding: 6px 2px;
    border: 1px solid var(--border);
    background: var(--bg-panel);
    cursor: pointer;
    font: inherit;
  }
  .cbar:hover {
    border-color: var(--text-faint);
  }
  .cbar.sel {
    border-color: var(--amber);
    box-shadow: inset 0 -2px 0 var(--amber);
  }
  .cnum {
    font-family: var(--mono);
    font-size: 16px;
    font-weight: 700;
    color: var(--text);
  }
  .clbl {
    font-size: 8.5px;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text-dim);
  }
  .cbar.correct .cnum {
    color: var(--green);
  }
  .cbar.reader_fail .cnum {
    color: var(--amber);
  }
  .cbar.retrieval_gap .cnum {
    color: var(--red);
  }
  .big {
    font-size: 24px;
    font-weight: 700;
    color: var(--cyan);
  }
  .hint {
    font-size: 9px;
    color: var(--text-faint);
    line-height: 1.35;
  }
  .src,
  .shape {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }
  .schip {
    border: 1px solid var(--border);
    background: var(--bg-panel);
    padding: 2px 6px;
    font-size: 10px;
    color: var(--text-dim);
    font-family: var(--mono);
  }
  .schip.both {
    border-color: var(--green);
    color: var(--green);
  }
  .schip.fact {
    border-color: var(--cyan);
    color: var(--cyan);
  }
  .schip.raw {
    border-color: var(--amber);
    color: var(--amber);
  }
  .schip.none {
    border-color: var(--red);
    color: var(--red);
  }
  .filt {
    display: flex;
    gap: 4px;
  }
  .ftab {
    background: var(--bg-panel);
    border: 1px solid var(--border-bright);
    color: var(--text-dim);
    font: inherit;
    font-size: 9px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    padding: 2px 8px;
    cursor: pointer;
  }
  .ftab:hover {
    color: var(--text);
    border-color: var(--text-faint);
  }
  .ftab.on {
    color: var(--amber);
    border-color: var(--amber);
  }
  .miss td {
    background: color-mix(in srgb, var(--amber) 5%, transparent);
  }
  .ctag {
    font-size: 9px;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    padding: 1px 5px;
    border: 1px solid var(--border);
    color: var(--text-dim);
  }
  .ctag.correct {
    border-color: var(--green);
    color: var(--green);
  }
  .ctag.reader_fail {
    border-color: var(--amber);
    color: var(--amber);
  }
  .ctag.retrieval_gap {
    border-color: var(--red);
    color: var(--red);
  }
  .dim {
    color: var(--text-dim);
  }
  .empty {
    text-align: center;
    color: var(--text-faint);
    padding: 16px;
  }

  /* ── Embedding vs rerank ─────────────────────────────────────────────── */
  /* embed = cyan, rerank = amber (the "lifted" result color). */
  .emb {
    color: var(--cyan);
  }
  .rrk {
    color: var(--amber);
  }
  .pad {
    padding: 10px 12px;
  }
  .pad code {
    color: var(--amber);
    font-family: var(--mono);
  }
  .evr {
    padding: 8px 10px 4px;
  }
  .evr-head,
  .cmp-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    margin-bottom: 6px;
    flex-wrap: wrap;
  }
  .evr-cap {
    font-size: 9px;
    font-weight: 800;
    letter-spacing: 0.14em;
    color: var(--text-faint);
  }
  .evr-inset {
    font-size: 9.5px;
    color: var(--text-dim);
    font-family: var(--mono);
  }
  .evrgrid,
  .cmpgrid {
    width: 100%;
    border-collapse: collapse;
    font-size: 11px;
  }
  .evrgrid th,
  .cmpgrid th {
    text-align: left;
    font-size: 9px;
    font-weight: 800;
    letter-spacing: 0.1em;
    color: var(--text-faint);
    padding: 3px 8px;
    border-bottom: 1px solid var(--border-bright);
    white-space: nowrap;
  }
  .evrgrid td,
  .cmpgrid td {
    padding: 3px 8px;
    border-bottom: 1px solid var(--border);
    white-space: nowrap;
  }
  .evrgrid .num,
  .cmpgrid .num {
    text-align: right;
  }
  .evrgrid .bars-h {
    width: 38%;
  }
  .bars-c {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding-top: 5px;
    padding-bottom: 5px;
  }
  .mini {
    height: 5px;
    background: var(--bg-panel);
    border: 1px solid var(--border);
    overflow: hidden;
  }
  .mfill {
    height: 100%;
  }
  .mfill.emb {
    background: var(--cyan);
  }
  .mfill.rrk {
    background: var(--amber);
  }
  .meanrow td {
    border-top: 1px solid var(--border-bright);
    border-bottom: none;
  }
  .meanrow .small {
    font-size: 9px;
    color: var(--text-faint);
  }
  /* Cross-run comparison. */
  .cmp {
    padding: 6px 10px 4px;
    border-top: 1px solid var(--border-bright);
    margin-top: 4px;
  }
  .cmpgrid .rn {
    font-family: var(--mono);
    font-size: 10px;
    color: var(--text);
    vertical-align: middle;
    border-right: 1px solid var(--border);
  }
  .cmpgrid .lbl {
    text-align: left;
    font-size: 9px;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }
  .cmpgrid .emb-row td {
    border-bottom: none;
  }
  .cmpgrid .cmp-run:hover td {
    background: color-mix(in srgb, var(--amber) 4%, transparent);
  }
  .ftab.dis {
    opacity: 0.4;
    cursor: not-allowed;
    text-decoration: line-through;
  }

  /* ── Tail percentiles + heatmap ──────────────────────────────────────── */
  /* Shared run·stage label cell: run name + a small embed/rerank chip. */
  .tlbl,
  .hm-rowl {
    white-space: nowrap;
  }
  .tlbl .rn,
  .hm-rowl .rn {
    font-family: var(--mono);
    font-size: 10px;
    color: var(--text);
    border-right: none;
  }
  .stage {
    margin-left: 6px;
    font-size: 8.5px;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    padding: 0 4px;
    border: 1px solid var(--border-bright);
  }
  .stage.emb {
    color: var(--cyan);
    border-color: var(--cyan-dim);
  }
  .stage.rrk {
    color: var(--amber);
    border-color: var(--amber-dim);
  }
  /* A hairline above each rerank row visually pairs it under its embed row. */
  .evrgrid.tail tbody tr.rrk-sep td,
  .hmgrid tbody tr:not(.rrk-sep) td {
    border-bottom: none;
  }
  .evrgrid.tail tbody tr:not(.rrk-sep) + tr.rrk-sep td,
  .hmgrid tbody tr.rrk-sep td {
    border-bottom: 1px solid var(--border-bright);
  }

  /* Heatmap grid. */
  .hmwrap {
    overflow-x: auto;
  }
  .hmgrid {
    width: 100%;
    border-collapse: collapse;
    font-size: 11px;
  }
  .hmgrid th {
    text-align: right;
    font-size: 9px;
    font-weight: 800;
    letter-spacing: 0.08em;
    color: var(--text-faint);
    padding: 3px 6px;
    white-space: nowrap;
  }
  .hmgrid .hm-rowh {
    text-align: left;
  }
  .hm-rowl {
    padding: 3px 8px 3px 0;
  }
  .hm-cell {
    text-align: right;
    padding: 5px 6px;
    color: var(--text);
    border: 1px solid rgba(0, 0, 0, 0.35);
    transition: outline 0.1s ease;
  }
  .hm-cell.intense {
    color: #fff;
  }
  .hm-cell:hover {
    outline: 1px solid var(--text);
    position: relative;
    z-index: 1;
  }

  /* Heatmap legend: low → high blue ramp + n footnote. */
  .hmlegend {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 9px;
    color: var(--text-faint);
  }
  .hm-ramp {
    width: 56px;
    height: 8px;
    border: 1px solid var(--border-bright);
    background: linear-gradient(
      90deg,
      rgba(55, 138, 221, 0.07),
      rgba(55, 138, 221, 0.95)
    );
  }
  .hm-n {
    margin-left: 4px;
    font-family: var(--mono);
    color: var(--text-dim);
  }
</style>
