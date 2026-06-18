// Minimal hash router as a rune-backed singleton. Routes:
//   #/            leaderboard
//   #/debug       debugger/tuner/runner
//   #/debug/<run> debugger focused on a run id (rest of hash)

function parse(): { view: string; arg: string } {
  const raw = window.location.hash.replace(/^#/, "") || "/";
  if (raw === "/" || raw === "") return { view: "leaderboard", arg: "" };
  if (raw.startsWith("/debug")) {
    const arg = raw.slice("/debug".length).replace(/^\//, "");
    return { view: "debug", arg: decodeURIComponent(arg) };
  }
  return { view: "leaderboard", arg: "" };
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
    go(view: string, arg = "") {
      window.location.hash = arg
        ? `/${view === "leaderboard" ? "" : view}/${encodeURIComponent(arg)}`
        : `/${view === "leaderboard" ? "" : view}`;
    },
    openRun(runId: string) {
      window.location.hash = `/debug/${encodeURIComponent(runId)}`;
    },
  };
}

export const router = createRouter();
