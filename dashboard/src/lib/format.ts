// Display formatting helpers — terminal-style: tabular, signed, terse.

export function pct(value: number | null | undefined, digits = 1): string {
  if (value === null || value === undefined) return "—";
  return (value * 100).toFixed(digits);
}

export function pctSign(value: number | null | undefined, digits = 1): string {
  if (value === null || value === undefined) return "—";
  const v = value * 100;
  const s = v >= 0 ? "+" : "";
  return s + v.toFixed(digits);
}

export function signed(value: number | null | undefined, digits = 0): string {
  if (value === null || value === undefined) return "—";
  const s = value >= 0 ? "+" : "";
  return s + value.toFixed(digits);
}

export function ms(value: number | null | undefined): string {
  if (value === null || value === undefined) return "—";
  if (value >= 1000) return (value / 1000).toFixed(2) + "s";
  return Math.round(value) + "ms";
}

export function money(microUsd: number | null | undefined): string {
  if (microUsd === null || microUsd === undefined) return "—";
  const usd = microUsd / 1_000_000;
  if (usd === 0) return "$0";
  if (usd < 0.01) return "$" + usd.toFixed(4);
  return "$" + usd.toFixed(2);
}

export function tokens(value: number | null | undefined): string {
  if (value === null || value === undefined) return "—";
  if (value >= 1_000_000) return (value / 1_000_000).toFixed(1) + "M";
  if (value >= 1_000) return (value / 1_000).toFixed(1) + "k";
  return String(value);
}

export function shortHash(value: string | null | undefined, len = 8): string {
  if (!value) return "—";
  return value.slice(0, len);
}

export function ago(modifiedMs: number | null | undefined): string {
  if (!modifiedMs) return "—";
  const delta = Date.now() - modifiedMs;
  const m = Math.floor(delta / 60000);
  if (m < 1) return "just now";
  if (m < 60) return `${m}m ago`;
  const h = Math.floor(m / 60);
  if (h < 24) return `${h}h ago`;
  const d = Math.floor(h / 24);
  return `${d}d ago`;
}

export function deltaClass(value: number | null | undefined): string {
  if (value === null || value === undefined || Math.abs(value) < 1e-9) return "flat";
  return value > 0 ? "up" : "down";
}

// Color ramp for accuracy heat cells (red → amber → green).
export function heatColor(value: number | null | undefined): string {
  if (value === null || value === undefined) return "transparent";
  const v = Math.max(0, Math.min(1, value));
  // 0 → red, 0.5 → amber, 1 → green
  if (v < 0.5) {
    const t = v / 0.5;
    return mix([255, 83, 71], [255, 165, 36], t);
  }
  const t = (v - 0.5) / 0.5;
  return mix([255, 165, 36], [47, 207, 122], t);
}

function mix(a: number[], b: number[], t: number): string {
  const r = Math.round(a[0] + (b[0] - a[0]) * t);
  const g = Math.round(a[1] + (b[1] - a[1]) * t);
  const bl = Math.round(a[2] + (b[2] - a[2]) * t);
  return `rgb(${r} ${g} ${bl})`;
}

// Compact, distinct axis/column labels for the six LongMemEval types.
export function qtypeAbbr(t: string | null | undefined): string {
  return (
    {
      "single-session-user": "SS·U",
      "single-session-assistant": "SS·A",
      "single-session-preference": "SS·P",
      "multi-session": "MULTI",
      "temporal-reasoning": "TEMP",
      "knowledge-update": "K·UPD",
    }[t ?? ""] ?? (t ?? "?").toUpperCase()
  );
}

export function qtypeShort(t: string | null | undefined): string {
  if (!t) return "?";
  return (
    {
      "single-session-user": "SS·USER",
      "single-session-assistant": "SS·ASST",
      "single-session-preference": "SS·PREF",
      "multi-session": "MULTI",
      "temporal-reasoning": "TEMPORAL",
      "knowledge-update": "KNOW·UPD",
    }[t] ?? t.toUpperCase()
  );
}
