<script lang="ts">
  import { api } from "../lib/api";
  import { store } from "../lib/store.svelte";
  import type {
    AnswererCallDebug,
    MemoryEvidenceDebug,
    QueryPlannerCallDebug,
    QuestionDebug,
    QuestionRow,
    RerankProfile,
    RetrievalProfileDebug,
  } from "../lib/types";
  import { qtypeShort } from "../lib/format";

  let { id }: { id: string } = $props();
  // Run-level oracle-gold flag (gold evidence fed straight to the answerer, recall bypassed). Read
  // from the shared registry so the per-question drawer can flag it without an extra fetch.
  const oracleGold = $derived(store.byId(id)?.oracle_gold ?? false);
  let rows = $state<QuestionRow[]>([]);
  let loading = $state(true);
  let verdict = $state<"all" | "correct" | "wrong" | "abstain" | "error">("all");
  let qtype = $state<string>("all");
  let search = $state("");
  // Debounced search term + a render cap so a 500-question run doesn't render
  // every matching row on each keystroke. The cap grows via "show more".
  let debouncedSearch = $state("");
  let renderCap = $state(200);
  let searchTimer: ReturnType<typeof setTimeout> | undefined;
  let active = $state<QuestionRow | null>(null);
  let drawerEl: HTMLElement | null = $state(null);
  let debugByPath = $state<Record<string, QuestionDebug | undefined>>({});
  let debugLoading = $state<Record<string, boolean | undefined>>({});
  let debugError = $state<Record<string, string | undefined>>({});

  $effect(() => {
    const q = search;
    clearTimeout(searchTimer);
    searchTimer = setTimeout(() => {
      debouncedSearch = q;
      renderCap = 200;
    }, 120);
    return () => clearTimeout(searchTimer);
  });

  $effect(() => {
    const runId = id;
    loading = true;
    active = null;
    rows = [];
    debugByPath = {};
    debugLoading = {};
    debugError = {};
    api.questions(runId)
      .then((q) => {
        if (id === runId) rows = q;
      })
      .finally(() => {
        if (id === runId) loading = false;
      });
  });

  $effect(() => {
    const path = active?.debug_artifact;
    if (!path || debugByPath[path] || debugLoading[path]) return;
    debugLoading = { ...debugLoading, [path]: true };
    debugError = { ...debugError, [path]: undefined };
    api.questionDebug(id, path)
      .then((debug) => {
        debugByPath = { ...debugByPath, [path]: debug };
      })
      .catch((error) => {
        debugError = { ...debugError, [path]: error.message ?? String(error) };
      })
      .finally(() => {
        debugLoading = { ...debugLoading, [path]: false };
      });
  });

  const types = $derived(["all", ...new Set(rows.map((r) => r.question_type).filter(Boolean) as string[])]);

  const filtered = $derived.by(() => {
    const q = debouncedSearch.trim().toLowerCase();
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

  // Cap the rendered rows so huge tables stay responsive (see renderCap).
  const visible = $derived(filtered.slice(0, renderCap));

  // Move focus into the drawer when it opens so Escape/keyboard users land inside.
  $effect(() => {
    if (active && drawerEl) drawerEl.focus();
  });

  const stats = $derived.by(() => {
    const correct = rows.filter((r) => r.label === true).length;
    const wrong = rows.filter((r) => r.label === false).length;
    return { correct, wrong, total: rows.length };
  });

  function plannerCall(row: QuestionRow): QueryPlannerCallDebug | null {
    const path = row.debug_artifact;
    if (!path) return null;
    const planner = debugByPath[path]?.recall?.query_planner_call
      ?? debugByPath[path]?.recall?.planner
      ?? null;
    return planner && !planner.parsed_plan && planner.plan
      ? { ...planner, parsed_plan: planner.plan }
      : planner;
  }

  function recallDebug(row: QuestionRow): QuestionDebug["recall"] | null {
    const path = row.debug_artifact;
    return path ? debugByPath[path]?.recall ?? null : null;
  }

  function promptList(values: string[] | null | undefined): string {
    return values?.length ? values.join("\n") : "—";
  }

  function usageText(usage: Record<string, unknown> | null | undefined): string {
    return usage ? JSON.stringify(usage) : "—";
  }

  function scoreText(score: number | null | undefined): string {
    return typeof score === "number" ? score.toFixed(4) : "—";
  }

  function sourceText(source: Record<string, unknown> | null | undefined): string {
    if (!source) return "—";
    const turn = typeof source.turn_id === "string" ? source.turn_id : null;
    const captured = typeof source.captured_at === "string" ? source.captured_at : null;
    const page = typeof source.page === "number" || typeof source.page === "string" ? `p${source.page}` : null;
    return [turn, captured, page].filter(Boolean).join(" · ") || JSON.stringify(source);
  }

  function sourceRefsText(refs: Array<Record<string, unknown>> | null | undefined): string {
    if (!refs?.length) return "—";
    return refs.slice(0, 4).map(sourceText).join("\n");
  }

  function tagsText(tags: string[] | null | undefined): string {
    return tags?.length ? tags.slice(0, 24).join(", ") : "—";
  }

  function profileCount(profile: RetrievalProfileDebug | null | undefined): string {
    const facts = profile?.facts?.length ?? 0;
    const raw = profile?.raw_turns?.length ?? 0;
    return `${facts} facts · ${raw} raw`;
  }

  function answererCalls(row: QuestionRow): AnswererCallDebug[] {
    const recall = recallDebug(row);
    const calls = recall?.answerer_calls ?? recall?.answer_calls;
    return Array.isArray(calls) ? (calls as AnswererCallDebug[]) : [];
  }

  function evidenceProfile(items: MemoryEvidenceDebug[] | null | undefined): RetrievalProfileDebug | null {
    if (!Array.isArray(items)) return null;
    return {
      facts: items.filter((item) => item.kind === "fact").map((item) => ({
        score: item.score,
        fact: {
          memory_id: item.evidence_id,
          content: item.content,
          event_time: item.captured_at,
          tags: item.tags,
          source_refs: item.source_refs,
        },
      })),
      raw_turns: items.filter((item) => item.kind === "raw_turn").map((item) => ({
        score: item.score,
        speaker: item.speaker,
        text: item.content,
        event_time: item.captured_at,
        source_ref: item.source_refs?.[0],
      })),
    };
  }

  function initialProfile(recall: QuestionDebug["recall"]): RetrievalProfileDebug | null {
    return recall?.initial_profile ?? evidenceProfile(recall?.evidence);
  }

  function fallbackProfile(recall: QuestionDebug["recall"]): RetrievalProfileDebug | null {
    return recall?.fallback_profile ?? evidenceProfile(recall?.fallback_evidence);
  }

  function rerankProfiles(recall: QuestionDebug["recall"]): RerankProfile[] {
    if (Array.isArray(recall?.rerank_trace)) return recall.rerank_trace;
    return (recall?.rerank ?? []).map((pass) => ({
      candidate_type: pass.candidate_set,
      candidates: (pass.candidates ?? []).map((candidate) => ({
        candidate_id: candidate.evidence_id ?? "",
        embedding_rank: candidate.embedding_rank,
        embedding_score: candidate.embedding_score,
        rerank_score: candidate.rerank_score,
        final_rank: candidate.final_rank,
        text: candidate.content,
      })),
    }));
  }

  // The EXACT context handed to the answerer (first call). This is the ground truth of what the
  // model saw — distinct from the recall "Search Results", which can differ (oracle mode bypasses
  // recall entirely; cuts/dedup also reshape it). Each item is "[type: ... ] <content>".
  function answererContextItems(calls: AnswererCallDebug[]): string[] {
    const ctx = calls[0]?.context;
    return Array.isArray(ctx) ? ctx.map((x) => String(x)) : [];
  }
  function parseContextItem(raw: string): { type: string; content: string } {
    const m = raw.match(/^\[type:\s*([a-z_]+)[^\]]*\]\s*([\s\S]*)$/);
    return m ? { type: m[1], content: m[2] } : { type: "?", content: raw };
  }

  function unknownText(value: unknown): string {
    if (value == null) return "—";
    if (typeof value === "string") return value;
    return JSON.stringify(value, null, 2);
  }
</script>

<svelte:window onkeydown={(e) => { if (e.key === "Escape" && active) active = null; }} />

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
      <span class="faint">
        / {filtered.length} match{filtered.length === 1 ? "" : "es"}
        {#if renderCap < filtered.length}<span class="dim"> · {visible.length} rendered</span>{/if}
      </span>
    </div>
  </div>

  <div class="qmain">
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
            {#each visible as r (r.question_id)}
              <!-- svelte-ignore a11y_no_noninteractive_element_to_interactive_role a11y_click_events_have_key_events -->
              <tr
                class:selected={active?.question_id === r.question_id}
                tabindex="0"
                role="button"
                aria-current={active?.question_id === r.question_id ? "true" : undefined}
                onclick={() => (active = r)}
                onkeydown={(e) => {
                  if (e.key === "Enter" || e.key === " ") {
                    e.preventDefault();
                    active = r;
                  }
                }}
              >
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
        {#if renderCap < filtered.length}
          <div class="capmore">
            <span class="faint">rendered {visible.length} of {filtered.length}</span>
            <button class="btn" onclick={() => (renderCap = Math.min(filtered.length, renderCap + 200))}>SHOW 200 MORE</button>
          </div>
        {/if}
      {/if}
  </div>

  {#if active}
      {@const planner = plannerCall(active)}
      {@const recall = recallDebug(active)}
      {@const calls = answererCalls(active)}
      {@const initial = initialProfile(recall)}
      {@const fallback = fallbackProfile(recall)}
      {@const rerank = rerankProfiles(recall)}
      <div
        class="drawer fade-in"
        role="dialog"
        aria-modal="true"
        aria-label="Question detail"
        tabindex="-1"
        bind:this={drawerEl}
      >
        <div class="dh">
          <span class="chip {active.label === true ? 'green' : active.label === false ? 'red' : ''}">
            {active.label === true ? "CORRECT" : active.label === false ? "WRONG" : "UNSCORED"}
          </span>
          {#if oracleGold}
            <span class="chip gold" title="Oracle-gold run: the answerer was fed gold-session evidence directly (reader-ceiling method), bypassing recall">G</span>
          {/if}
          <span class="did mono-num">{active.question_id}</span>
          <button class="x" onclick={() => (active = null)}>✕</button>
        </div>
        <div class="db">
          <section class="debug-section">
            <h3>Question</h3>
            <div class="kv">
              <span class="label">TYPE</span>
              <div>{active.question_type ?? "—"} {#if active.is_abstention}<span class="chip amber">abstention</span>{/if}</div>
              <span class="label">JUDGE</span>
              <div>{active.judge_model ?? "—"} → <b class:up={active.label} class:down={active.label === false}>{active.judge_raw ?? "—"}</b></div>
              <span class="label">DEBUG BUNDLE</span>
              <pre>{active.debug_artifact ?? "not recorded for this row"}</pre>
            </div>
            <div class="debug-grid">
              <div class="mini full"><span class="label">QUESTION</span><pre>{active.question ?? "—"}</pre></div>
              <div class="mini"><span class="label">GOLD ANSWER</span><pre class="gold">{active.gold_answer ?? "—"}</pre></div>
              <div class="mini"><span class="label">HYPOTHESIS</span><pre class:wrongtext={active.label === false} class:righttext={active.label === true}>{active.hypothesis ?? "—"}</pre></div>
            </div>
          </section>

          {#if active.judge_system_prompt || active.judge_user_prompt}
            <section class="debug-section">
              <h3>Judge Input — the exact prompt sent to the grader</h3>
              <div class="kv">
                <span class="label">GRADER</span>
                <div>{active.judge_model ?? "—"} · {active.question_type ?? "—"} prompt → <b class:up={active.label} class:down={active.label === false}>{active.judge_raw ?? "—"}</b></div>
              </div>
              <div class="debug-grid">
                <div class="mini full"><span class="label">JUDGE SYSTEM PROMPT</span><pre>{active.judge_system_prompt ?? "—"}</pre></div>
                <div class="mini full"><span class="label">JUDGE USER MESSAGE</span><pre>{active.judge_user_prompt ?? "—"}</pre></div>
              </div>
            </section>
          {/if}

          {#if active.debug_artifact}
            {#if debugLoading[active.debug_artifact]}
              <section class="debug-section"><h3>Debug</h3><div class="txt faint">LOADING QUESTION DEBUG…</div></section>
            {:else if debugError[active.debug_artifact]}
              <section class="debug-section"><h3>Debug</h3><div class="txt down">{debugError[active.debug_artifact]}</div></section>
            {:else}
              {@const ctxItems = answererContextItems(calls)}
              <section class="debug-section">
                <h3>Answerer Context — what the model actually received</h3>
                <div class="pmeta">
                  <span><b>{ctxItems.length}</b> items fed to the answerer</span>
                  {#if oracleGold}<span class="gold-meta">GOLD · ORACLE — this context is gold-session evidence (score 1.000), fed directly; recall below is diagnostic only</span>{/if}
                  <span>Search Results / Rerank below are retrieval candidates — NOT necessarily fed (oracle mode bypasses recall; cuts reshape it)</span>
                </div>
                <div class="search-profile">
                  {#each ctxItems as item, i (`ctx-${i}`)}
                    {@const p = parseContextItem(item)}
                    <div class="result">
                      <div class="rmeta"><span>#{i + 1}</span><span>{p.type}</span></div>
                      <pre>{p.content}</pre>
                    </div>
                  {/each}
                  {#if !ctxItems.length}<pre>— no answerer context recorded —</pre>{/if}
                </div>
              </section>

              <section class="debug-section">
                <h3>Base Search Prompt</h3>
                <div class="pmeta">
                  <span>mode <b>{planner?.mode ?? "—"}</b></span>
                  <span>finish <b>{planner?.finish_reason ?? "—"}</b></span>
                  <span>usage <b>{usageText(planner?.usage)}</b></span>
                </div>
                {#if planner?.error}<div class="txt down">{planner.error}</div>{/if}
                <div class="mini full">
                  <span class="label">SYSTEM PROMPT</span>
                  <pre>{planner?.system_prompt ?? "—"}</pre>
                </div>
              </section>

              <section class="debug-section">
                <h3>Generated Search Prompt And Response</h3>
                <div class="debug-grid">
                  <div class="mini"><span class="label">USER PROMPT</span><pre>{planner?.user_prompt ?? "—"}</pre></div>
                  <div class="mini"><span class="label">QUERY RESPONSE</span><pre>{planner?.response_text ?? "—"}</pre></div>
                </div>
              </section>

              <section class="debug-section">
                <h3>Query Plan</h3>
                <div class="debug-grid">
                  <div class="mini"><span class="label">CANONICAL QUERY</span><pre>{planner?.parsed_plan?.canonical_query ?? "—"}</pre></div>
                  <div class="mini"><span class="label">DENSE QUERIES</span><pre>{promptList(planner?.parsed_plan?.dense_queries)}</pre></div>
                  <div class="mini"><span class="label">SPARSE TERMS</span><pre>{promptList(planner?.parsed_plan?.sparse_terms)}</pre></div>
                  <div class="mini"><span class="label">RETRIEVAL QUERIES</span><pre>{promptList(recall?.retrieval_queries)}</pre></div>
                  <div class="mini full"><span class="label">QUERY PLAN USED BY SEARCH</span><pre>{JSON.stringify(recall?.query_plan ?? {}, null, 2)}</pre></div>
                </div>
              </section>

              <section class="debug-section">
                <h3>Search Results — retrieval candidates (not necessarily fed to the answerer)</h3>
                <details class="prompt" open>
                  <summary>INITIAL SEARCH RESPONSE · {profileCount(initial)}</summary>
                  {@render searchProfile(initial)}
                </details>
                {#if fallback}
                  <details class="prompt" open>
                    <summary>FALLBACK SEARCH RESPONSE · {profileCount(fallback)}</summary>
                    {@render searchProfile(fallback)}
                  </details>
                {/if}
              </section>

              {#if rerank.length}
                <section class="debug-section">
                  <h3>Rerank</h3>
                  {#each rerank as rt, i (`rerank-trace-${i}`)}
                    <details class="prompt" open>
                      <summary>{i === 0 ? "INITIAL" : "FALLBACK"} RERANK · {rt.candidates.length} candidates</summary>
                      {@render rerankProfile(rt)}
                    </details>
                  {/each}
                </section>
              {/if}

              <section class="debug-section">
                <h3>Answer</h3>
                <div class="debug-grid">
                  <div class="mini"><span class="label">ROUTER PICK</span><pre>{active.router_pick ?? "—"}</pre></div>
                  <div class="mini"><span class="label">INITIAL → FINAL</span><pre>{active.initial_pick ?? "—"} → {active.final_pick ?? "—"}</pre></div>
                </div>
                {#if calls.length}
                  {#each calls as call, i (`answer-${i}`)}
                    <details class="prompt" open>
                      <summary>ANSWERER CALL {i + 1} · {unknownText(call.phase)} · {unknownText(call.finish_reason)}</summary>
                      <div class="answer-call">
                        <div class="mini"><span class="label">USAGE</span><pre>{unknownText(call.usage)}</pre></div>
                        <div class="mini"><span class="label">PROCESSED ANSWER</span><pre>{unknownText(call.processed_text)}</pre></div>
                        <div class="mini full"><span class="label">ANSWER SYSTEM PROMPT</span><pre>{unknownText(call.system_prompt)}</pre></div>
                        <div class="mini full"><span class="label">ANSWER USER PROMPT</span><pre>{unknownText(call.prompt)}</pre></div>
                        <div class="mini full"><span class="label">RESPONSE TEXT</span><pre>{unknownText(call.response_text)}</pre></div>
                        <div class="mini full"><span class="label">EVIDENCE CONTEXT</span><pre>{unknownText(call.context)}</pre></div>
                      </div>
                    </details>
                  {/each}
                {:else if recall}
                  <div class="txt faint">No answerer call recorded for this row.</div>
                {:else}
                  <div class="txt faint">No query planner, retrieval, or answer debug in question debug.</div>
                {/if}
              </section>
            {/if}
          {:else}
            <section class="debug-section"><h3>Debug</h3><div class="txt faint">No question debug artifact recorded.</div></section>
          {/if}
          {#if active.error}<section class="debug-section"><h3>Error</h3><div class="txt down">{active.error}</div></section>{/if}
        </div>
      </div>
  {/if}
  </div>
</div>

{#snippet searchProfile(profile: RetrievalProfileDebug | null | undefined)}
  {#if profile}
    <div class="search-profile">
      {#if profile.facts?.length}
        <div class="search-title">FACT RESULTS</div>
        {#each profile.facts as item, i (`f-${i}-${item.fact?.memory_id ?? ""}`)}
          <div class="result">
            <div class="rmeta">
              <span>#{i + 1}</span>
              <span>score {scoreText(item.score)}</span>
              <span>{item.fact?.status ?? "status?"}</span>
              <span>{item.fact?.event_time ?? item.fact?.valid_from ?? "time?"}</span>
            </div>
            <pre>{item.fact?.content ?? "—"}</pre>
            <div class="rsub">sources</div>
            <pre>{sourceRefsText(item.fact?.source_refs)}</pre>
            <div class="rsub">tags</div>
            <pre>{tagsText(item.fact?.tags)}</pre>
          </div>
        {/each}
      {/if}
      {#if profile.raw_turns?.length}
        <div class="search-title">RAW TURN RESULTS</div>
        {#each profile.raw_turns as item, i (`r-${i}-${item.ordinal ?? ""}`)}
          <div class="result">
            <div class="rmeta">
              <span>#{i + 1}</span>
              <span>score {scoreText(item.score)}</span>
              <span>{item.speaker ?? "speaker?"}</span>
              <span>{item.event_time ?? "time?"}</span>
              {#if item.ordinal != null}<span>ord {item.ordinal}</span>{/if}
            </div>
            <pre>{item.text ?? "—"}</pre>
            <div class="rsub">source</div>
            <pre>{sourceText(item.source_ref)}</pre>
          </div>
        {/each}
      {/if}
      {#if !profile.facts?.length && !profile.raw_turns?.length}
        <pre>—</pre>
      {/if}
    </div>
  {:else}
    <pre>—</pre>
  {/if}
{/snippet}

{#snippet rerankProfile(rt: RerankProfile | null | undefined)}
  {#if rt?.candidates?.length}
    {@const top = [...rt.candidates]
      .sort((a, b) => (a.final_rank ?? 9999) - (b.final_rank ?? 9999))
      .slice(0, 12)}
    <div class="search-profile">
      <div class="search-title">
        RERANKED {(rt.candidate_type ?? "FACT").toUpperCase()} · top {top.length} of {rt.candidates.length}
      </div>
      {#each top as c, i (`rr-${i}-${c.candidate_id ?? i}`)}
        <div class="result">
          <div class="rmeta">
            <span>final #{c.final_rank ?? "?"}</span>
            <span>rerank {scoreText(c.rerank_score)}</span>
            <span class:amber={(c.embedding_rank ?? null) !== (c.final_rank ?? null)}>
              embed #{c.embedding_rank ?? "?"} ({scoreText(c.embedding_score)})
            </span>
          </div>
          <pre>{c.text ?? "—"}</pre>
        </div>
      {/each}
    </div>
  {:else}
    <pre>—</pre>
  {/if}
{/snippet}

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
  .qstat .dim {
    color: var(--text-faint);
  }
  .faint {
    color: var(--text-faint);
  }
  .capmore {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 12px;
    padding: 10px;
    border-top: 1px solid var(--border);
    background: var(--bg-panel);
  }

  .qmain {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: 1fr;
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
    position: fixed;
    inset: calc(var(--h-topbar) + 4px) 16px calc(var(--h-statusbar) + 2px);
    z-index: 30;
    border: 1px solid var(--border-bright);
    background: var(--bg-panel);
    box-shadow: 0 20px 80px rgba(0, 0, 0, 0.72);
    overflow: auto;
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
    padding: 14px;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .debug-section {
    border: 1px solid var(--border);
    background: rgba(255, 255, 255, 0.012);
    padding: 12px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .debug-section h3 {
    margin: 0;
    color: var(--amber);
    font-size: 12px;
    line-height: 1.3;
    font-weight: 800;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }
  .debug-grid,
  .answer-call {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 10px;
  }
  .kv {
    display: grid;
    grid-template-columns: max-content minmax(0, 1fr);
    gap: 6px 12px;
    align-items: start;
  }
  .kv pre {
    margin: 0;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .label {
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
  .wrongtext {
    color: var(--red) !important;
  }
  .righttext {
    color: var(--green) !important;
  }
  .load {
    padding: 30px;
    color: var(--text-faint);
    letter-spacing: 0.2em;
  }
  .pmeta {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    color: var(--text-dim);
    font-size: 10px;
  }
  .pmeta span {
    border: 1px solid var(--border);
    background: var(--bg);
    padding: 3px 6px;
  }
  .pmeta b {
    color: var(--text);
    font-weight: 600;
  }
  .pmeta .gold-meta {
    color: var(--gold);
    border-color: var(--gold-dim);
    background: rgba(232, 195, 74, 0.08);
    font-weight: 600;
  }
  .mini {
    border: 1px solid var(--border);
    background: var(--bg);
    padding: 6px 8px;
  }
  .mini.full {
    grid-column: 1 / -1;
  }
  .mini pre,
  .prompt pre {
    margin: 0;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    color: var(--text);
    font-size: 11px;
    line-height: 1.45;
  }
  .prompt {
    border: 1px solid var(--border);
    background: var(--bg);
  }
  .prompt summary {
    cursor: pointer;
    color: var(--text-dim);
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.08em;
    padding: 6px 8px;
    border-bottom: 1px solid var(--border);
  }
  .prompt pre {
    padding: 8px;
  }
  .search-profile {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 8px;
  }
  .search-title {
    color: var(--amber);
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.08em;
    margin-top: 4px;
  }
  .result {
    border: 1px solid var(--border);
    background: rgba(255, 255, 255, 0.015);
    padding: 6px;
  }
  .result pre {
    padding: 0;
  }
  .rmeta {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
    margin-bottom: 5px;
    color: var(--text-dim);
    font-size: 10px;
  }
  .rmeta span {
    border: 1px solid var(--border);
    padding: 2px 5px;
  }
  .rsub {
    margin: 6px 0 2px;
    color: var(--text-faint);
    font-size: 9px;
    font-weight: 700;
    letter-spacing: 0.08em;
  }
  @media (max-width: 900px) {
    .drawer {
      inset: 38px 8px 22px;
    }
    .debug-grid,
    .answer-call {
      grid-template-columns: 1fr;
    }
  }
</style>
