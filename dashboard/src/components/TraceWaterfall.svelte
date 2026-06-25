<script lang="ts">
  // Horizontal waterfall of memory/provider work over a timeline. Renders both
  // the trace waterfall (lanes = operations, blocks = work/wait/run) and the
  // dependency waterfall (lanes = sources, blocks = setup/capture/parallel/...)
  // via the `variant` prop. Owns all `.wf-*` positioning + block-colour styles.
  import { ms } from "../lib/format";
  import { clampPct } from "../lib/format";

  export interface WfBlock {
    start_ms: number;
    end_ms: number;
    duration_ms: number;
    label: string;
    kind: string;
    status?: string;
    source?: string;
    item_count?: number;
    item_unit?: string;
  }
  export interface WfLane {
    id: string;
    label: string;
    /** Chip kind label rendered before the lane name (e.g. "memory"). */
    chipKind?: string;
    waitMs?: number | null;
    setupMs?: number | null;
    blocks: WfBlock[];
  }
  export interface WfTick {
    ratio: number;
    label: string;
  }

  let {
    variant,
    lanes,
    ticks,
    duration,
    blockTitle,
  }: {
    variant: "trace" | "dependency";
    lanes: WfLane[];
    ticks: WfTick[];
    duration: number;
    blockTitle: (block: WfBlock) => string;
  } = $props();

  function blockClass(block: WfBlock): string {
    return variant === "dependency"
      ? `dep-block ${block.kind}`
      : `${block.kind} ${block.status ?? ""}`;
  }
</script>

<div class="wf" class:dep={variant === "dependency"}>
  <div class="wf-axis">
    <div></div>
    <div class="wf-track axis">
      {#each ticks as tick (tick.ratio)}
        <span class="wf-tick" class:last={tick.ratio === 1} style:left={`${tick.ratio * 100}%`}><i></i><b>{tick.label}</b></span>
      {/each}
    </div>
  </div>
  {#each lanes as lane (lane.id)}
    <div class="wf-row" class:dep-row={variant === "dependency"}>
      <div class="wf-name">
        <span class="bkind {lane.chipKind ?? "memory"}">{lane.chipKind ?? "source"}</span>
        <span class="wf-name-text">{lane.label}</span>
        {#if lane.setupMs != null}<span class="dep-setup mono-num">setup {ms(lane.setupMs)}</span>{/if}
        {#if lane.waitMs != null}<span class="dep-wait mono-num">wait {ms(lane.waitMs)}</span>{/if}
      </div>
      <div class="wf-track">
        {#each lane.blocks as block, i (`${lane.id}-${block.kind}-${block.start_ms}-${i}`)}
          <span
            class="wf-block {blockClass(block)}"
            style:left={`${clampPct(block.start_ms, duration)}%`}
            style:width={`${Math.max(0.18, clampPct(Math.max(block.duration_ms, block.end_ms - block.start_ms, 1), duration))}%`}
            title={blockTitle(block)}
          ></span>
        {/each}
      </div>
    </div>
  {/each}
</div>

<style>
  .wf {
    max-height: 360px;
    overflow-y: auto;
    overflow-x: hidden;
    padding: 8px;
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
  }
  .wf.dep {
    max-height: 300px;
  }
  .wf-axis,
  .wf-row {
    display: grid;
    grid-template-columns: minmax(170px, 280px) minmax(0, 1fr);
    gap: 8px;
    align-items: center;
    min-width: 0;
  }
  .dep-row .wf-track {
    height: 18px;
  }
  .dep-setup,
  .dep-wait {
    margin-left: auto;
    font-size: 9px;
  }
  .dep-setup {
    color: var(--cyan);
  }
  .dep-wait {
    margin-left: 0;
    color: var(--amber);
  }
  .wf-axis {
    position: sticky;
    top: 0;
    z-index: 2;
    background: var(--bg);
    padding-bottom: 2px;
  }
  .wf-track {
    position: relative;
    height: 16px;
    background: var(--bg-elev);
    border: 1px solid var(--border);
    overflow: hidden;
    min-width: 0;
  }
  .wf-track.axis {
    height: 18px;
    overflow: hidden;
    background:
      linear-gradient(to right, rgba(255, 255, 255, 0.04) 1px, transparent 1px) 0 0 / 25% 100%,
      var(--bg-elev);
  }
  .wf-tick {
    position: absolute;
    top: 0;
    height: 100%;
    color: var(--text-faint);
    font-size: 8px;
    transform: translateX(-1px);
  }
  .wf-tick i {
    display: block;
    width: 1px;
    height: 100%;
    background: var(--border-bright);
  }
  .wf-tick b {
    position: absolute;
    top: 2px;
    left: 4px;
    font-weight: 500;
    white-space: nowrap;
  }
  .wf-tick.last b {
    left: auto;
    right: 4px;
  }
  .wf-name {
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 10px;
  }
  .wf-name-text {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text);
  }
  .wf-block {
    position: absolute;
    top: 3px;
    height: 8px;
    min-width: 2px;
    opacity: 0.92;
    border: 1px solid rgba(255, 255, 255, 0.12);
  }
  .wf-block.memory_work {
    background: var(--cyan);
  }
  .wf-block.dep-block {
    top: 4px;
    height: 8px;
  }
  .wf-block.dep-block.setup {
    background: var(--blue);
  }
  .wf-block.dep-block.capture {
    background: var(--cyan);
  }
  .wf-block.dep-block.parallel {
    background: var(--cyan);
  }
  .wf-block.dep-block.blocked_distill,
  .wf-block.dep-block.blocked_raw {
    background: var(--amber);
    border-color: rgba(255, 174, 31, 0.7);
  }
  .wf-block.dep-block.archive,
  .wf-block.dep-block.index,
  .wf-block.dep-block.consolidate {
    background: var(--green);
  }
  .wf-block.dep-block.answer {
    background: var(--violet);
  }
  .wf-block.provider_wait {
    background: var(--amber);
  }
  .wf-block.provider_run {
    background: var(--green);
  }
  .wf-block.memory_failed,
  .wf-block.provider_failed,
  .wf-block.failed,
  .wf-block.dead {
    background: var(--red);
  }
</style>
