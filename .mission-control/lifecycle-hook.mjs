#!/usr/bin/env node
// mission-control:managed version=2.2.0
import { spawnSync } from 'node:child_process';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const kit = dirname(fileURLToPath(import.meta.url));

async function main() {
  let input = {};
  try {
    const chunks = [];
    for await (const chunk of process.stdin) chunks.push(chunk);
    if (chunks.length) input = JSON.parse(Buffer.concat(chunks).toString('utf8'));
  } catch {}
  const event = input.hook_event_name ?? process.argv[2];
  const running = event === 'SessionStart';
  const idle = event === 'Stop' || event === 'SessionEnd';
  if (!running && !idle) return;
  spawnSync(process.execPath, [join(kit, 'bridge.mjs'), 'pulse', running ? 'running' : 'idle', '--summary', running ? 'Agent session active.' : 'Agent turn idle.', '--quiet'], {
    cwd: dirname(kit), env: process.env, stdio: 'ignore', timeout: 1500
  });
}

main().catch(() => {}).finally(() => { process.exitCode = 0; });
