<script lang="ts">
  interface Props {
    value: number | null;
    size?: number;
    color?: string;
    label?: string;
    sub?: string;
  }
  let { value, size = 132, color = "var(--amber)", label = "", sub = "" }: Props =
    $props();

  const stroke = 9;
  const r = $derived((size - stroke) / 2);
  const cx = $derived(size / 2);
  const cy = $derived(size / 2);
  const sweep = 270; // degrees
  const circ = $derived(2 * Math.PI * r);
  const arcLen = $derived((sweep / 360) * circ);
  const frac = $derived(value == null ? 0 : Math.max(0, Math.min(1, value)));
  const display = $derived(value == null ? "—" : (value * 100).toFixed(1));
</script>

<div class="gauge" style="width:{size}px;height:{size}px">
  <svg viewBox="0 0 {size} {size}" width={size} height={size}>
    <g transform="rotate(135 {cx} {cy})">
      <circle
        {cx}
        {cy}
        {r}
        fill="none"
        stroke="var(--bg-elev)"
        stroke-width={stroke}
        stroke-dasharray="{arcLen} {circ}"
        stroke-linecap="round"
      />
      <circle
        {cx}
        {cy}
        {r}
        fill="none"
        stroke={color}
        stroke-width={stroke}
        stroke-dasharray="{arcLen * frac} {circ}"
        stroke-linecap="round"
        style="transition:stroke-dasharray 0.6s cubic-bezier(0.2,0.8,0.2,1)"
      />
    </g>
  </svg>
  <div class="center">
    <div class="val mono-num" style="color:{color}">{display}<span class="pct">%</span></div>
    {#if label}<div class="lbl">{label}</div>{/if}
    {#if sub}<div class="sub">{sub}</div>{/if}
  </div>
</div>

<style>
  .gauge {
    position: relative;
    display: inline-block;
  }
  .center {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 1px;
  }
  .val {
    font-size: 26px;
    font-weight: 700;
    line-height: 1;
  }
  .pct {
    font-size: 13px;
    color: var(--text-faint);
    margin-left: 1px;
  }
  .lbl {
    font-family: var(--sans);
    font-size: 8.5px;
    font-weight: 700;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--text-faint);
    margin-top: 3px;
  }
  .sub {
    font-size: 9.5px;
    color: var(--text-dim);
  }
</style>
