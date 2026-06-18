<script lang="ts">
  interface Pt {
    x: number;
    y: number;
    label: string;
    color: string;
    highlight?: boolean;
  }
  interface Props {
    points: Pt[];
    xlabel: string;
    ylabel: string;
    width?: number;
    height?: number;
    xFmt?: (v: number) => string;
    yFmt?: (v: number) => string;
  }
  let {
    points,
    xlabel,
    ylabel,
    width = 460,
    height = 240,
    xFmt = (v) => v.toFixed(0),
    yFmt = (v) => v.toFixed(2),
  }: Props = $props();

  const padL = 44,
    padR = 16,
    padT = 14,
    padB = 30;
  const plotW = $derived(width - padL - padR);
  const plotH = $derived(height - padT - padB);

  const xs = $derived(points.map((p) => p.x));
  const ys = $derived(points.map((p) => p.y));
  const xmin = $derived(Math.min(...xs));
  const xmax = $derived(Math.max(...xs));
  const ymin = $derived(Math.min(...ys));
  const ymax = $derived(Math.max(...ys));

  function sx(x: number): number {
    const span = xmax - xmin || 1;
    return padL + ((x - xmin) / span) * plotW;
  }
  function sy(y: number): number {
    const span = ymax - ymin || 1;
    return padT + plotH - ((y - ymin) / span) * plotH;
  }
  const yticks = $derived([0, 0.25, 0.5, 0.75, 1].map((t) => ymin + (ymax - ymin) * t));
  const xticks = $derived([0, 0.5, 1].map((t) => xmin + (xmax - xmin) * t));
</script>

<svg viewBox="0 0 {width} {height}" {width} {height}>
  {#each yticks as t (t)}
    <line x1={padL} y1={sy(t)} x2={width - padR} y2={sy(t)} stroke="var(--border)" stroke-width="0.5" />
    <text x={padL - 6} y={sy(t)} text-anchor="end" dominant-baseline="middle" class="tk">{yFmt(t)}</text>
  {/each}
  {#each xticks as t (t)}
    <text x={sx(t)} y={height - 10} text-anchor="middle" class="tk">{xFmt(t)}</text>
  {/each}
  {#each points as p (p.label)}
    <circle
      cx={sx(p.x)}
      cy={sy(p.y)}
      r={p.highlight ? 5 : 3.5}
      fill={p.color}
      fill-opacity={p.highlight ? 1 : 0.8}
      stroke={p.highlight ? "#fff" : "none"}
      stroke-width="1"
    >
      <title>{p.label}</title>
    </circle>
  {/each}
  <text x={padL} y={11} class="axl">{ylabel}</text>
  <text x={width - padR} y={height - 10} text-anchor="end" class="axl">{xlabel}</text>
</svg>

<style>
  .tk {
    font-family: var(--mono);
    font-size: 8.5px;
    fill: var(--text-faint);
  }
  .axl {
    font-family: var(--sans);
    font-size: 8.5px;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    fill: var(--text-dim);
  }
</style>
