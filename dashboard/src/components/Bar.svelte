<script lang="ts">
  interface Props {
    value: number | null;
    max?: number;
    color?: string;
    track?: string;
    height?: number;
    marker?: number | null;
  }
  let {
    value,
    max = 1,
    color = "var(--amber)",
    track = "var(--bg-elev)",
    height = 7,
    marker = null,
  }: Props = $props();

  const fillW = $derived(
    value == null ? 0 : Math.max(0, Math.min(1, value / max)) * 100,
  );
  const markW = $derived(
    marker == null ? null : Math.max(0, Math.min(1, marker / max)) * 100,
  );
</script>

<div class="bar" style="height:{height}px;background:{track}">
  <div class="fill" style="width:{fillW}%;background:{color}"></div>
  {#if markW != null}
    <div class="mark" style="left:{markW}%"></div>
  {/if}
</div>

<style>
  .bar {
    position: relative;
    width: 100%;
    min-width: 54px;
    overflow: hidden;
    border: 1px solid var(--border);
  }
  .fill {
    height: 100%;
    transition: width 0.4s cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  .mark {
    position: absolute;
    top: -1px;
    bottom: -1px;
    width: 2px;
    background: var(--text);
    box-shadow: 0 0 2px #000;
  }
</style>
