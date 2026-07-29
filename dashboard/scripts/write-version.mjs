// Post-build: bind the static landing to its source commit, release identity,
// bundled snapshot, and a deterministic digest of every dist file except this
// evidence file. Excluding version.json avoids a self-referential digest.
import { createHash } from "node:crypto";
import { readFileSync, readdirSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, relative, resolve, sep } from "node:path";
import { execSync } from "node:child_process";

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, "..", "..");
const dist = resolve(here, "..", "dist");

function gitSha() {
  try {
    return execSync("git rev-parse HEAD", {
      cwd: root,
      stdio: ["ignore", "pipe", "ignore"],
    })
      .toString()
      .trim();
  } catch {
    return "unknown";
  }
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function filesBelow(directory) {
  const files = [];
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const path = resolve(directory, entry.name);
    if (entry.isDirectory()) files.push(...filesBelow(path));
    else if (entry.isFile()) files.push(path);
  }
  return files;
}

function distTreeDigest() {
  const digest = createHash("sha256");
  const entries = filesBelow(dist)
    .filter((path) => relative(dist, path) !== "version.json")
    .map((path) => [relative(dist, path).split(sep).join("/"), path])
    .sort(([left], [right]) => left.localeCompare(right));
  for (const [path, absolute] of entries) {
    digest.update(`${path}\0${sha256(readFileSync(absolute))}\n`);
  }
  return digest.digest("hex");
}

const release = JSON.parse(readFileSync(resolve(root, "release", "release.json"), "utf8"));
const snapshotPath = resolve(root, release.landing.snapshot_path);
const builtSnapshotPath = resolve(dist, "data", "leaderboard.json");
const sourceSnapshot = readFileSync(snapshotPath);
const builtSnapshot = readFileSync(builtSnapshotPath);
const snapshotSha256 = sha256(sourceSnapshot);
if (snapshotSha256 !== sha256(builtSnapshot)) {
  throw new Error("built leaderboard snapshot differs from the committed release snapshot");
}

const html = readFileSync(resolve(dist, "index.html"), "utf8");
const match = html.match(/index-([A-Za-z0-9_-]+)\.js/);
const bundle = match ? match[1] : "unknown";

const payload = {
  schema: "membench.landing-evidence.v1",
  version: release.version,
  tag: release.tag,
  commit: gitSha(),
  records_digest: release.records_digest,
  snapshot_sha256: snapshotSha256,
  dist_tree_sha256: distTreeDigest(),
  bundle,
};

writeFileSync(resolve(dist, "version.json"), JSON.stringify(payload, null, 2) + "\n");
console.log(
  `wrote dist/version.json  bundle=${bundle}  commit=${payload.commit.slice(0, 12)} ` +
    `dist=${payload.dist_tree_sha256.slice(0, 12)}`,
);
