#!/usr/bin/env node
// mission-control:managed version=2.2.0
import { createHash, randomUUID, sign } from 'node:crypto';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { createInterface } from 'node:readline';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { loadProjectContext } from './config.mjs';

const execFileAsync = promisify(execFile);
const kit = dirname(fileURLToPath(import.meta.url));
const workTools = ['start', 'checkpoint', 'wait', 'finish', 'fail'].map((action) => ({
  name: `work_${action}`,
  description: `${action === 'start' ? 'Report the start of' : action === 'checkpoint' ? 'Report meaningful progress on' : action === 'wait' ? 'Report a wait boundary for' : action === 'fail' ? 'Report a failed execution boundary for' : 'Report a verified handoff boundary for'} an explicitly named task. This never completes the provider task automatically.`,
  inputSchema: { type: 'object', properties: { taskId: { type: 'string' }, summary: { type: 'string', minLength: 2 }, nextAction: { type: 'string' }, progress: { type: 'integer', minimum: 0, maximum: 100 } }, required: ['taskId', 'summary'], additionalProperties: false }
}));
const tools = [
  { name: 'project_get', description: 'Inspect this project, its tasks, runs, workflow lanes, and questions.', inputSchema: { type: 'object', properties: {}, additionalProperties: false } },
  { name: 'task_graph', description: 'Read this project dependency graph and parallel-ready frontier.', inputSchema: { type: 'object', properties: {}, additionalProperties: false } },
  { name: 'tasks_list', description: 'List canonical tasks for this project by status or text query.', inputSchema: { type: 'object', properties: { status: { type: 'string' }, query: { type: 'string' } }, additionalProperties: false } },
  { name: 'task_get', description: 'Inspect one task, provider link, priority, blockers, and synchronization state.', inputSchema: { type: 'object', properties: { taskId: { type: 'string' } }, required: ['taskId'], additionalProperties: false } },
  { name: 'task_transition', description: 'Apply a version-checked task lifecycle transition through Mission Control and its provider outbox.', inputSchema: { type: 'object', properties: { taskId: { type: 'string' }, status: { enum: ['backlog', 'ready', 'in_progress', 'blocked', 'review', 'done', 'cancelled'] }, reason: { type: 'string', minLength: 2 } }, required: ['taskId', 'status', 'reason'], additionalProperties: false } },
  { name: 'tasks_sync', description: 'Refresh a declared migration ledger; provider-authoritative projects return a safe no-op.', inputSchema: { type: 'object', properties: {}, additionalProperties: false } },
  { name: 'tasks_reconcile', description: 'Apply explicitly granted legacy write-backs; provider-authoritative projects return a safe no-op.', inputSchema: { type: 'object', properties: {}, additionalProperties: false } },
  ...workTools
];

function stringArg(args, name, required = false) {
  const value = args[name];
  if (value === undefined && !required) return undefined;
  if (typeof value !== 'string' || (required && !value.trim())) throw new Error(`${name} must be a non-empty string`);
  return value;
}
function api(context) {
  const url = new URL((process.env.MC_API_URL ?? context.apiUrl ?? 'http://127.0.0.1:4200/api').replace(/\/$/, ''));
  if (process.env.MC_MCP_ALLOW_REMOTE !== 'true' && !context.credential?.privateKeyPem && !['127.0.0.1', 'localhost', '::1'].includes(url.hostname)) throw new Error('Project MCP refuses a non-loopback API without an enrolled connector identity');
  return url.toString().replace(/\/$/, '');
}
async function request(context, path, init = {}) {
  const configuredTimeout = Number(process.env.MC_MCP_TIMEOUT_MS ?? 5000);
  const timeout = Number.isInteger(configuredTimeout) && configuredTimeout >= 250 && configuredTimeout <= 30_000 ? configuredTimeout : 5000;
  const method = init.method ?? 'GET';
  const body = init.body ?? '';
  const target = new URL(`${api(context)}${path}`);
  const headers = { 'content-type': 'application/json', ...(init.headers ?? {}) };
  if (context.credential?.keyId && context.credential?.privateKeyPem) {
    const timestamp = String(Math.floor(Date.now() / 1000));
    const nonce = randomUUID();
    const canonical = [method, `${target.pathname}${target.search}`, createHash('sha256').update(body).digest('hex'), timestamp, nonce].join('\n');
    Object.assign(headers, {
      'x-mc-key-id': context.credential.keyId,
      'x-mc-timestamp': timestamp,
      'x-mc-nonce': nonce,
      'x-mc-signature': sign(null, Buffer.from(canonical), context.credential.privateKeyPem).toString('base64url')
    });
  }
  const response = await fetch(target, { ...init, signal: init.signal ?? AbortSignal.timeout(timeout), headers });
  const result = await response.json();
  if (!response.ok) throw new Error(result.error ?? `Mission Control returned HTTP ${response.status}`);
  return result;
}
async function bridge(family, action, args = []) {
  const { stdout } = await execFileAsync(process.execPath, [join(kit, 'bridge.mjs'), family, action, ...args], {
    cwd: dirname(kit), env: process.env, timeout: 5000, maxBuffer: 1024 * 1024
  });
  return JSON.parse(stdout);
}
async function callTool(name, args) {
  const context = await loadProjectContext();
  const projectId = context.projectId;
  if (name === 'project_get') return request(context, `/projects/${encodeURIComponent(projectId)}`);
  if (name === 'task_graph') return request(context, `/projects/${encodeURIComponent(projectId)}/task-graph`);
  if (name === 'tasks_list') {
    const query = new URLSearchParams({ projectId });
    const status = stringArg(args, 'status');
    const text = stringArg(args, 'query');
    if (status) query.set('status', status);
    if (text) query.set('q', text);
    return request(context, `/tasks?${query}`);
  }
  if (name === 'task_get') return request(context, `/tasks/${encodeURIComponent(stringArg(args, 'taskId', true))}`);
  if (name === 'task_transition') {
    const taskId = stringArg(args, 'taskId', true);
    const current = await request(context, `/tasks/${encodeURIComponent(taskId)}`);
    return request(context, `/tasks/${encodeURIComponent(taskId)}/transition`, { method: 'POST', body: JSON.stringify({
      status: stringArg(args, 'status', true), expectedVersion: current.version,
      actor: context.reporting.reporter, reason: stringArg(args, 'reason', true)
    }) });
  }
  if (name === 'tasks_sync') return bridge('tasks', 'sync');
  if (name === 'tasks_reconcile') return bridge('tasks', 'reconcile');
  if (name.startsWith('work_')) {
    const action = name.slice('work_'.length);
    const cli = ['--task', stringArg(args, 'taskId', true), '--summary', stringArg(args, 'summary', true)];
    const next = stringArg(args, 'nextAction');
    if (next) cli.push('--next', next);
    if (args.progress !== undefined) {
      if (!Number.isInteger(args.progress) || args.progress < 0 || args.progress > 100) throw new Error('progress must be an integer from 0 to 100');
      cli.push('--progress', String(args.progress));
    }
    return bridge('work', action, cli);
  }
  throw new Error(`Unknown tool: ${name}`);
}
function send(payload) { process.stdout.write(`${JSON.stringify(payload)}\n`); }
async function handle(message) {
  if (message.method?.startsWith('notifications/')) return;
  const id = message.id ?? null;
  if (message.method === 'initialize') {
    send({ jsonrpc: '2.0', id, result: { protocolVersion: '2025-06-18', capabilities: { tools: { listChanged: false } }, serverInfo: { name: 'mission-control-project', version: '2.2.0' }, instructions: 'Project-scoped, code-blind task and progress tools. Provider tasks remain canonical.' } });
    return;
  }
  if (message.method === 'ping') { send({ jsonrpc: '2.0', id, result: {} }); return; }
  if (message.method === 'tools/list') { send({ jsonrpc: '2.0', id, result: { tools } }); return; }
  if (message.method === 'tools/call') {
    const params = message.params ?? {};
    const args = params.arguments && typeof params.arguments === 'object' ? params.arguments : {};
    try {
      const result = await callTool(String(params.name ?? ''), args);
      send({ jsonrpc: '2.0', id, result: { content: [{ type: 'text', text: JSON.stringify(result, null, 2) }] } });
    } catch (error) {
      send({ jsonrpc: '2.0', id, result: { isError: true, content: [{ type: 'text', text: error instanceof Error ? error.message : String(error) }] } });
    }
    return;
  }
  send({ jsonrpc: '2.0', id, error: { code: -32601, message: `Method not found: ${message.method}` } });
}
const lines = createInterface({ input: process.stdin, crlfDelay: Infinity });
lines.on('line', (line) => {
  if (!line.trim()) return;
  try {
    const message = JSON.parse(line);
    void handle(message).catch((error) => send({ jsonrpc: '2.0', id: message.id ?? null, error: { code: -32603, message: error instanceof Error ? error.message : String(error) } }));
  } catch (error) {
    send({ jsonrpc: '2.0', id: null, error: { code: -32700, message: error instanceof Error ? error.message : 'Parse error' } });
  }
});
