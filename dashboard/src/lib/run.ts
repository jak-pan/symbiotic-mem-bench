// Run-domain helpers shared across the registry tree, leaderboard, and detail
// header. Kept out of format.ts (pure display) because they reason about run
// records rather than formatting primitives.

import type { RunSummary } from "./types";

/** Trial badge text: FOCUSED when any marker is focused, otherwise TRIAL. */
export function trialBadge(run: Pick<RunSummary, "trial_markers">): string {
  return run.trial_markers.some((marker) => marker.focused) ? "FOCUSED" : "TRIAL";
}

/** Short chip label for a run kind (`imported-artifact` -> `import`). */
export function runKindLabel(kind: string): string {
  return kind === "imported-artifact" ? "import" : kind;
}

/** Chip color class for a run kind (native = green, otherwise cyan). */
export function runKindChipClass(kind: string): string {
  return kind === "native" ? "green" : "cyan";
}
