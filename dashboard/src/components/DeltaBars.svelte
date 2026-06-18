<script lang="ts">
  import { pctSign } from "../lib/format";

  interface Item {
    label: string;
    delta: number;
    n?: number;
  }
  interface Props {
    items: Item[];
    height?: number;
  }
  let { items, height = 18 }: Props = $props();

  const maxAbs = $derived(
    Math.max(0.02, ...items.map((i) => Math.abs(i.delta))),
  );
</script>

<div class="delta-bars">
  {#each items as item (item.label)}
    {@const w = (Math.abs(item.delta) / maxAbs) * 50}
    <div class="row" style="height:{height}px">
      <div class="lbl">{item.label}</div>
      <div class="track">
        <div class="center"></div>
        {#if item.delta >= 0}
          <div class="fill pos" style="left:50%;width:{w}%"></div>
        {:else}
          <div class="fill neg" style="right:50%;width:{w}%"></div>
        {/if}
      </div>
      <div class="val mono-num" class:up={item.delta > 0} class:down={item.delta < 0}>
        {pctSign(item.delta)}
      </div>
    </div>
  {/each}
</div>

<style>
  .delta-bars {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .row {
    display: grid;
    grid-template-columns: 96px 1fr 54px;
    align-items: center;
    gap: 8px;
  }
  .lbl {
    font-family: var(--sans);
    font-size: 9px;
    font-weight: 700;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    color: var(--text-dim);
    text-align: right;
  }
  .track {
    position: relative;
    height: 60%;
    background: var(--bg-elev);
    border: 1px solid var(--border);
  }
  .center {
    position: absolute;
    left: 50%;
    top: -1px;
    bottom: -1px;
    width: 1px;
    background: var(--border-bright);
  }
  .fill {
    position: absolute;
    top: 0;
    bottom: 0;
  }
  .fill.pos {
    background: var(--green);
  }
  .fill.neg {
    background: var(--red);
  }
  .val {
    font-size: 11px;
    font-weight: 600;
    text-align: right;
  }
</style>
