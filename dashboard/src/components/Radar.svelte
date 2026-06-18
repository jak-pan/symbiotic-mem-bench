<script lang="ts">
  interface Series {
    label: string;
    color: string;
    values: (number | null)[];
  }
  interface Props {
    axes: string[];
    series: Series[];
    size?: number;
    min?: number;
  }
  let { axes, series, size = 240, min = 0.5 }: Props = $props();

  const cx = $derived(size / 2);
  const cy = $derived(size / 2);
  const r = $derived(size / 2 - 34);

  function norm(v: number | null): number {
    if (v == null) return 0;
    return Math.max(0, (v - min) / (1 - min));
  }
  function point(i: number, value: number): [number, number] {
    const angle = (-90 + (360 / axes.length) * i) * (Math.PI / 180);
    return [cx + r * value * Math.cos(angle), cy + r * value * Math.sin(angle)];
  }
  function poly(values: (number | null)[]): string {
    return values
      .map((v, i) => point(i, norm(v)).join(","))
      .join(" ");
  }
  const rings = [0.25, 0.5, 0.75, 1];
</script>

<svg viewBox="0 0 {size} {size}" width={size} height={size}>
  {#each rings as ring (ring)}
    <polygon
      points={axes.map((_, i) => point(i, ring).join(",")).join(" ")}
      fill="none"
      stroke="var(--border-bright)"
      stroke-width="0.5"
    />
  {/each}
  {#each axes as ax, i (i)}
    {@const [ex, ey] = point(i, 1)}
    {@const [lx, ly] = point(i, 1.16)}
    <line x1={cx} y1={cy} x2={ex} y2={ey} stroke="var(--border-bright)" stroke-width="0.5" />
    <text
      x={lx}
      y={ly}
      text-anchor="middle"
      dominant-baseline="middle"
      class="ax"
    >{ax}</text>
  {/each}
  {#each series as s (s.label)}
    <polygon
      points={poly(s.values)}
      fill={s.color}
      fill-opacity="0.13"
      stroke={s.color}
      stroke-width="1.5"
    />
    {#each s.values as v, i (i)}
      {@const [px, py] = point(i, norm(v))}
      <circle cx={px} cy={py} r="2.2" fill={s.color} />
    {/each}
  {/each}
</svg>

<style>
  .ax {
    font-family: var(--sans);
    font-size: 8px;
    font-weight: 700;
    letter-spacing: 0.06em;
    fill: var(--text-faint);
  }
</style>
