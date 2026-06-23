// Minimal hash router as a rune-backed singleton. Routes:
//   #/            leaderboard
//   #/debug       debugger/tuner/runner
//   #/debug/<run> debugger focused on a run id (rest of hash)
//   #/debug/<run>/<subscreen> debugger focused on overview/questions/compare/traces/live/tuner

export type DebugSubscreen =
  | "overview"
  | "questions"
  | "compare"
  | "traces"
  | "live"
  | "tuner";

const DEBUG_SUBSCREENS = new Set<DebugSubscreen>([
  "overview",
  "questions",
  "compare",
  "traces",
  "live",
  "tuner",
]);

function parse(): { view: string; arg: string; subscreen: string } {
  const raw = window.location.hash.replace(/^#/, "") || "/";
  if (raw === "/" || raw === "") {
    return { view: "leaderboard", arg: "", subscreen: "" };
  }
  if (raw.startsWith("/debug")) {
    const arg = raw.slice("/debug".length).replace(/^\//, "");
    if (!arg) return { view: "debug", arg: "", subscreen: "overview" };

    const parts = arg.split("/");
    const first = parts[0] ?? "";
    const last = parts[parts.length - 1] ?? "";

    // Preferred shape is /debug/<encoded-run>/<subscreen>. Also accept
    // /debug/<subscreen>/<encoded-run> so hand-edited links are forgiving.
    if (DEBUG_SUBSCREENS.has(last as DebugSubscreen) && parts.length > 1) {
      return {
        view: "debug",
        arg: decodeURIComponent(parts.slice(0, -1).join("/")),
        subscreen: last,
      };
    }
    if (DEBUG_SUBSCREENS.has(first as DebugSubscreen) && parts.length > 1) {
      return {
        view: "debug",
        arg: decodeURIComponent(parts.slice(1).join("/")),
        subscreen: first,
      };
    }
    return { view: "debug", arg: decodeURIComponent(arg), subscreen: "overview" };
  }
  return { view: "leaderboard", arg: "", subscreen: "" };
}

function createRouter() {
  let route = $state(parse());
  window.addEventListener("hashchange", () => {
    route = parse();
  });
  return {
    get view() {
      return route.view;
    },
    get arg() {
      return route.arg;
    },
    get subscreen() {
      return route.subscreen;
    },
    go(view: string, arg = "", subscreen = "") {
      if (view === "leaderboard") {
        window.location.hash = "/";
        return;
      }
      const encoded = arg ? `/${encodeURIComponent(arg)}` : "";
      const suffix = arg && subscreen ? `/${subscreen}` : "";
      window.location.hash = `/${view}${encoded}${suffix}`;
    },
    openRun(runId: string, subscreen = route.subscreen || "overview") {
      this.go("debug", runId, subscreen);
    },
    openRunSubscreen(runId: string, subscreen: DebugSubscreen) {
      this.go("debug", runId, subscreen);
    },
  };
}

export const router = createRouter();
