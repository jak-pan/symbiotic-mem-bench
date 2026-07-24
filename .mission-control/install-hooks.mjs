#!/usr/bin/env node
// mission-control:managed version=2.2.0
import { execFileSync } from 'node:child_process';
import { chmod, mkdir, readFile, writeFile } from 'node:fs/promises';
import { basename, dirname, isAbsolute, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = dirname(dirname(fileURLToPath(import.meta.url)));
const begin = '# mission-control:begin';
const end = '# mission-control:end';
const tools = new Set(['codex', 'claude-code', 'hermes', 'pi', 'symbiotic', 'custom']);

function option(args, name) {
  const index = args.indexOf(name);
  return index >= 0 ? args[index + 1] : undefined;
}

function enrollmentSelection(input) {
  const enrollmentPath = input?.enrollmentPath;
  const tool = input?.tool;
  if (!enrollmentPath || !/^\.mission-control\/enrollments\/[a-z0-9](?:[a-z0-9-]*[a-z0-9])?\.local\.json$/.test(enrollmentPath)) {
    throw new Error('A managed .mission-control/enrollments/*.local.json path is required');
  }
  if (!tool || !tools.has(tool)) throw new Error('A supported Mission Control enrollment tool is required');
  return { enrollmentPath, tool };
}

function hookBlock(input) {
  const { enrollmentPath, tool } = enrollmentSelection(input);
  const environment = `MC_ENROLLMENT_PATH='${enrollmentPath}' MC_ENROLLMENT_TOOL='${tool}'`;
  return `${begin}
repo_root="$(git rev-parse --show-toplevel 2>/dev/null)" || exit 0
${environment} node "$repo_root/.mission-control/bridge.mjs" tasks sync --quiet >/dev/null 2>&1 || true
${environment} node "$repo_root/.mission-control/bridge.mjs" pulse running --summary "Repository checkpoint committed." --quiet >/dev/null 2>&1 || true
${end}`;
}

export function upsertHookBlock(content, input) {
  const block = hookBlock(input);
  const pattern = new RegExp(`${begin.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}[\\s\\S]*?${end.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}`);
  const base = pattern.test(content) ? content.replace(pattern, block) : `${content.trimEnd()}\n\n${block}\n`;
  return base.startsWith('#!') ? base : `#!/usr/bin/env sh\n${base}`;
}

async function main() {
  const args = process.argv.slice(2);
  const selection = enrollmentSelection({
    enrollmentPath: option(args, '--enrollment-path') ?? process.env.MC_ENROLLMENT_PATH,
    tool: option(args, '--tool') ?? process.env.MC_ENROLLMENT_TOOL
  });
  const gitPath = execFileSync('git', ['-C', root, 'rev-parse', '--git-path', 'hooks'], { encoding: 'utf8' }).trim();
  const hooksDirectory = isAbsolute(gitPath) ? gitPath : resolve(root, gitPath);
  const hookPath = join(hooksDirectory, 'post-commit');
  await mkdir(hooksDirectory, { recursive: true });
  let current = '';
  try { current = await readFile(hookPath, 'utf8'); } catch {}
  if (current && !current.startsWith('#!') ) throw new Error(`Refusing to modify non-script hook: ${hookPath}`);
  if (current && !/^#!.*\b(sh|bash|zsh)\b/.test(current.split(/\r?\n/, 1)[0])) throw new Error(`Refusing to modify non-shell hook: ${hookPath}`);
  await writeFile(hookPath, upsertHookBlock(current, selection), 'utf8');
  await chmod(hookPath, 0o755);
  process.stdout.write(`${JSON.stringify({ installed: true, hookPath, enrollmentPath: selection.enrollmentPath, tool: selection.tool }, null, 2)}\n`);
}

if (process.argv[1] && basename(process.argv[1]) === 'install-hooks.mjs') {
  main().catch((error) => {
    process.stderr.write(`[mission-control] ${error instanceof Error ? error.message : String(error)}\n`);
    process.exitCode = 1;
  });
}
