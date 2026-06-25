<script lang="ts">
  // Provider Queue Summary — aggregated wait/run/total percentiles per queue.
  // Pure render of rows derived by the parent (which also feeds bottlenecks).
  import { ms } from "../lib/format";
  import type { QueueSummaryRow } from "../lib/types";

  let { rows }: { rows: QueueSummaryRow[] } = $props();
</script>

<table class="grid">
  <thead>
    <tr>
      <th>Queue</th>
      <th class="num">Items</th>
      <th class="num">Fail</th>
      <th class="num">Wait p80</th>
      <th class="num">Wait p95</th>
      <th class="num">Run p80</th>
      <th class="num">Run p95</th>
      <th class="num">Total p80</th>
      <th class="num">Total p98</th>
    </tr>
  </thead>
  <tbody>
    {#each rows as q (q.name)}
      <tr>
        <td class="amber">{q.name}</td>
        <td class="num mono-num">{q.count}</td>
        <td class="num mono-num" class:down={q.failed > 0}>{q.failed || "—"}</td>
        <td class="num mono-num">{ms(q.wait_p80)}</td>
        <td class="num mono-num dim">{ms(q.wait_p95)}</td>
        <td class="num mono-num">{ms(q.run_p80)}</td>
        <td class="num mono-num dim">{ms(q.run_p95)}</td>
        <td class="num mono-num">{ms(q.total_p80)}</td>
        <td class="num mono-num">{ms(q.total_p98)}</td>
      </tr>
    {/each}
  </tbody>
</table>
