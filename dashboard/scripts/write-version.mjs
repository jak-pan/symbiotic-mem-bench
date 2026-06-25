// Post-build: stamp the built bundle's content hash into dist/version.json so
// the UI can surface the exact bundle it's running — the same hash Vite uses
// for cache-busting, so it changes on every rebuild that changes the bundle
// (no commit or version bump required).
import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";
import { execSync } from "node:child_process";

const here = dirname(fileURLToPath(import.meta.url));
const dist = resolve(here, "..", "dist");

function gitSha() {
  try {
    return execSync("git rev-parse --short HEAD", {
      stdio: ["ignore", "pipe", "ignore"],
    })
      .toString()
      .trim();
  } catch {
    return "unknown";
  }
}

const html = readFileSync(resolve(dist, "index.html"), "utf8");
const match = html.match(/index-([A-Za-z0-9_-]+)\.js/);
const bundle = match ? match[1] : "unknown";

const payload = {
  bundle,
  git: gitSha(),
  built: new Date().toISOString(),
};

writeFileSync(resolve(dist, "version.json"), JSON.stringify(payload, null, 2) + "\n");
console.log(`wrote dist/version.json  bundle=${bundle}  git=${payload.git}`);
