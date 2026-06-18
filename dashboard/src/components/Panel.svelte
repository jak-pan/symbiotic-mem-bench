<script lang="ts">
  import type { Snippet } from "svelte";
  interface Props {
    title?: string;
    tag?: string;
    flush?: boolean;
    scroll?: boolean;
    children?: Snippet;
    actions?: Snippet;
  }
  let { title, tag, flush = false, scroll = false, children, actions }: Props =
    $props();
</script>

<section class="panel">
  {#if title || actions}
    <header class="panel-h">
      <span class="panel-t">{title}{#if tag}<span class="tag">{tag}</span>{/if}</span>
      {#if actions}<div class="panel-a">{@render actions()}</div>{/if}
    </header>
  {/if}
  <div class="panel-b" class:flush class:scroll>
    {@render children?.()}
  </div>
</section>

<style>
  .panel {
    display: flex;
    flex-direction: column;
    background: var(--bg-panel);
    border: 1px solid var(--border);
    min-height: 0;
    min-width: 0;
  }
  .panel-h {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 5px 10px;
    border-bottom: 1px solid var(--border-bright);
    background: linear-gradient(var(--bg-elev), var(--bg-panel));
    flex: none;
  }
  .panel-t {
    font-family: var(--sans);
    font-size: 10px;
    font-weight: 800;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: var(--text-dim);
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .tag {
    color: var(--amber);
    font-weight: 600;
    letter-spacing: 0.05em;
  }
  .panel-a {
    display: flex;
    gap: 6px;
    align-items: center;
  }
  .panel-b {
    min-height: 0;
    min-width: 0;
    flex: 1;
  }
  .panel-b:not(.flush) {
    padding: 10px;
  }
  .panel-b.scroll {
    overflow: auto;
  }
</style>
