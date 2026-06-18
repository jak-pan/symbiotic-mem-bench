<script lang="ts">
  import { QTYPES, type QTypeScore } from "../lib/types";
  import { heatColor, pct, qtypeShort } from "../lib/format";

  interface Props {
    scores: Record<string, QTypeScore> | null | undefined;
    cell?: number;
    gap?: number;
  }
  let { scores, cell = 13, gap = 2 }: Props = $props();
</script>

<div class="heat" style="gap:{gap}px">
  {#each QTYPES as qt (qt)}
    {@const s = scores?.[qt]}
    <div
      class="cell"
      style="width:{cell}px;height:{cell}px;background:{s
        ? heatColor(s.accuracy)
        : 'var(--bg-elev)'}"
      title={s
        ? `${qtypeShort(qt)}  ${pct(s.accuracy)}%  (${s.correct}/${s.n})`
        : `${qtypeShort(qt)}  no data`}
    ></div>
  {/each}
</div>

<style>
  .heat {
    display: inline-flex;
  }
  .cell {
    border: 1px solid rgba(0, 0, 0, 0.35);
    transition: transform 0.1s ease;
  }
  .cell:hover {
    transform: scale(1.25);
    outline: 1px solid var(--text);
    z-index: 1;
  }
</style>
