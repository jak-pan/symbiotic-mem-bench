#!/usr/bin/env node
// mission-control:managed version=2.2.0
import { createHash, randomUUID, sign } from 'node:crypto';
import { readFile, readdir, rename, writeFile } from 'node:fs/promises';
import { basename, dirname, join, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { loadProjectContext } from './config.mjs';

const root = dirname(dirname(fileURLToPath(import.meta.url)));
let requestCredential;

export function taskSourcePath(sourcePath) {
  const path = resolve(root, sourcePath);
  if (path !== root && !path.startsWith(`${root}${sep}`)) throw new Error(`Task source escapes project root: ${sourcePath}`);
  return path;
}

function option(args, name) {
  const index = args.indexOf(name);
  return index >= 0 ? args[index + 1] : undefined;
}

export function normalizeTaskStatus(value) {
  const status = String(value).trim().toLowerCase().replace(/[ -]+/g, '_');
  if (status === 'pending') return 'backlog';
  if (status === 'active' || status === 'started') return 'in_progress';
  if (status === 'pending_review') return 'review';
  if (status.startsWith('complete') || status.startsWith('done') || status === 'completed' || status === 'archived') return 'done';
  if (status.startsWith('in_progress')) return 'in_progress';
  if (['backlog', 'ready', 'in_progress', 'blocked', 'review', 'done', 'cancelled'].includes(status)) return status;
  return 'backlog';
}

function normalizePriority(value) {
  const priority = String(value).toUpperCase();
  return ['P0', 'P1', 'P2', 'P3'].includes(priority) ? priority : 'P3';
}

export function parseTaskJson(content) {
  const input = JSON.parse(content);
  if (!Array.isArray(input)) throw new Error('task-json-v1 requires a top-level array');
  return input.map((item) => ({
    sourceId: String(item.sourceId ?? item.source_id ?? `task:${item.id}`),
    title: String(item.title ?? item.task ?? '').trim(),
    status: normalizeTaskStatus(item.status),
    priority: normalizePriority(item.priority),
    blockedBySourceIds: (item.blockedBySourceIds ?? item.blocked_by ?? []).map((id) => String(id).startsWith('task:') ? String(id) : `task:${id}`),
    sourceUrl: item.sourceUrl ?? item.source_url,
    revision: item.revision
  })).filter((item) => item.title);
}

export function parseMarkdownTaskTable(content) {
  const rows = [];
  for (const line of content.split(/\r?\n/)) {
    const cells = line.split('|').slice(1, -1).map((cell) => cell.trim());
    if (cells.length < 4) continue;
    const id = cells[0].replace(/^#/, '');
    if (!/^[A-Za-z0-9][A-Za-z0-9._-]*$/.test(id) || !/\d/.test(id)) continue;
    const nativeTitle = cells[1].trim();
    if (!nativeTitle) continue;
    rows.push({
      sourceId: `task:${id}`,
      title: nativeTitle.slice(0, 300),
      ...(nativeTitle.length > 300 ? { summary: nativeTitle.slice(0, 1000) } : {}),
      status: normalizeTaskStatus(cells[2]),
      priority: normalizePriority(cells[3]),
      blockedBySourceIds: []
    });
  }
  return rows;
}

function markdownSection(content, heading) {
  const lines = content.split(/\r?\n/);
  const start = lines.findIndex((line) => line.trim().toLowerCase() === `## ${heading.toLowerCase()}`);
  if (start < 0) return [];
  const end = lines.findIndex((line, index) => index > start && /^##\s+/.test(line));
  return lines.slice(start + 1, end < 0 ? lines.length : end);
}

export function extractChunkDeclarations(content) {
  const declarations = [];
  const seen = new Set();
  for (const line of markdownSection(content, 'Chunks')) {
    for (const match of line.matchAll(/((?:\d+[a-z]?)-[a-z0-9][a-z0-9._-]*\.md)/gi)) {
      const filename = match[1];
      if (seen.has(filename)) continue;
      seen.add(filename);
      const nativeStatus = line.match(/\b(IN[ _-]?PROGRESS|PENDING[ _-]?REVIEW|PENDING|REVIEW|DONE|COMPLETE|BLOCKED|CANCELLED)\b/i)?.[1];
      declarations.push({ filename, nativeStatus, context: line });
    }
  }
  return declarations;
}

export function parseMarkdownTaskFile(content, sourceId = 'task:file') {
  const nativeStatus = content.match(/^\*\*Status\*\*:\s*`?([^\n`]+)/mi)?.[1]?.trim()
    ?? content.match(/^## Status\s*\n+\s*`?([^\n`]+)/mi)?.[1]?.trim();
  const heading = content.match(/^#\s+(.+)$/m)?.[1]?.trim();
  return {
    sourceId,
    title: heading ?? sourceId,
    status: normalizeTaskStatus(nativeStatus ?? 'pending'),
    nativeStatus,
    priority: 'P3',
    blockedBySourceIds: []
  };
}

export function fileRevision(content) {
  return `sha256:${createHash('sha256').update(content).digest('hex')}`;
}

export function applyTaskJsonPatch(content, sourceId, patch) {
  const input = JSON.parse(content);
  if (!Array.isArray(input)) throw new Error('task-json-v1 requires a top-level array');
  const item = input.find((candidate) => String(candidate.sourceId ?? candidate.source_id ?? `task:${candidate.id}`) === sourceId);
  if (!item) throw new Error(`Native task not found: ${sourceId}`);
  item.status = patch.nativeStatus;
  return `${JSON.stringify(input, null, 2)}\n`;
}

export function applyMarkdownTaskPatch(content, sourceId, patch) {
  const lines = content.split(/\r?\n/);
  const headerIndex = lines.findIndex((line) => {
    const cells = line.split('|').slice(1, -1).map((cell) => cell.trim().toLowerCase());
    return cells.includes('status') && (cells.includes('#') || cells.includes('id'));
  });
  if (headerIndex < 0) throw new Error('Markdown task table header not found');
  const headers = lines[headerIndex].split('|').slice(1, -1).map((cell) => cell.trim().toLowerCase());
  const idIndex = headers.includes('#') ? headers.indexOf('#') : headers.indexOf('id');
  const statusIndex = headers.indexOf('status');
  const nativeId = sourceId.replace(/^task:/, '');
  let changed = false;
  for (let index = headerIndex + 2; index < lines.length; index += 1) {
    const cells = lines[index].split('|').slice(1, -1).map((cell) => cell.trim());
    if (cells[idIndex]?.replace(/^#/, '') !== nativeId) continue;
    cells[statusIndex] = patch.nativeStatus;
    lines[index] = `| ${cells.join(' | ')} |`;
    changed = true;
    break;
  }
  if (!changed) throw new Error(`Native task not found: ${sourceId}`);
  return lines.join('\n');
}

export function applyMarkdownTaskFilePatch(content, _sourceId, patch) {
  const bold = /^(\*\*Status\*\*:\s*)`?[^\n`]+`?/mi;
  if (bold.test(content)) return content.replace(bold, `$1${patch.nativeStatus}`);
  const section = /^(## Status\s*\n+\s*)`?[^\n`]+`?/mi;
  if (section.test(content)) return content.replace(section, `$1${patch.nativeStatus}`);
  throw new Error('Markdown task file status field not found');
}

function tableRows(content) {
  const rows = [];
  for (const line of content.split(/\r?\n/)) {
    const cells = line.split('|').slice(1, -1).map((cell) => cell.trim());
    if (cells.length < 4 || !/^#?\d+$/.test(cells[0])) continue;
    rows.push({ id: cells[0].replace(/^#/, ''), title: cells[1], status: cells[2], priority: normalizePriority(cells[3]) });
  }
  return rows;
}

function shortSummary(content, fallback) {
  const tldr = content.match(/^\*\*TL;DR\*\*:\s*(.+)$/mi)?.[1]?.trim();
  if (tldr) return tldr.slice(0, 1000);
  const paragraph = content.split(/\n\s*\n/).map((part) => part.trim())
    .find((part) => part && !part.startsWith('#') && !part.startsWith('**Priority') && !part.startsWith('**Status'));
  return (paragraph ?? fallback).replace(/\s+/g, ' ').slice(0, 1000);
}

function repositoryFileUrl(config, path, directory = false) {
  if (!config.repository?.url) return undefined;
  const base = config.repository.url.replace(/\/$/, '');
  const branch = config.repository.defaultBranch ?? 'main';
  return `${base}/${directory ? 'tree' : 'blob'}/${branch}/${path}`;
}

export async function parseHierarchicalTaskRepo(config, source) {
  const activeContent = await readFile(taskSourcePath(source.path), 'utf8');
  const activeRevision = fileRevision(activeContent);
  const active = parseTaskJson(activeContent);
  const byId = new Map(active.map((item) => [item.sourceId.replace(/^task:/, ''), { ...item, archived: false }]));
  for (const historyPath of [source.summaryPath, source.archivePath].filter(Boolean)) {
    const history = await readFile(taskSourcePath(historyPath), 'utf8').catch(() => '');
    for (const row of tableRows(history)) {
      if (!byId.has(row.id)) byId.set(row.id, {
        sourceId: `task:${row.id}`, title: row.title, status: 'done', priority: row.priority,
        blockedBySourceIds: [], archived: true
      });
    }
  }

  const taskRoot = source.taskRoot ?? dirname(source.path);
  const directories = (await readdir(taskSourcePath(taskRoot), { withFileTypes: true }))
    .filter((entry) => entry.isDirectory());
  const directoryById = new Map();
  for (const directory of directories) {
    const id = directory.name.match(/^(\d+)-/)?.[1];
    if (id && !directoryById.has(id)) directoryById.set(id, directory.name);
  }

  const subprojects = source.subprojects ?? [];
  const tasks = [];
  for (const [nativeId, parent] of [...byId.entries()].sort((left, right) => Number(left[0]) - Number(right[0]))) {
    const directory = directoryById.get(nativeId);
    const readmePath = directory ? `${taskRoot}/${directory}/README.md` : undefined;
    const readme = readmePath ? await readFile(taskSourcePath(readmePath), 'utf8').catch(() => '') : '';
    tasks.push({
      ...parent,
      kind: 'initiative',
      projectId: config.projectId,
      labels: [config.taskProjection.source.system, 'initiative', parent.archived ? 'history:archived' : 'planning:active'],
      summary: readme ? shortSummary(readme, parent.title) : parent.title,
      sourceUrl: directory ? repositoryFileUrl(config, `${taskRoot}/${directory}`, true) : repositoryFileUrl(config, source.summaryPath ?? source.path),
      revision: parent.archived ? undefined : activeRevision,
      sourcePath: parent.archived ? (source.archivePath ?? source.summaryPath ?? source.path) : source.path,
      sourceFormat: parent.archived ? 'markdown-task-table-v1' : 'task-json-v1',
      writeback: !parent.archived,
      statusMap: source.statusMap
    });
    if (!directory || !readme) continue;

    let sequence = 0;
    for (const declaration of extractChunkDeclarations(readme)) {
      sequence += 1;
      const chunkPath = `${taskRoot}/${directory}/${declaration.filename}`;
      const content = await readFile(taskSourcePath(chunkPath), 'utf8').catch(() => undefined);
      const parsed = content ? parseMarkdownTaskFile(content) : undefined;
      const nativeStatus = parsed?.nativeStatus ?? declaration.nativeStatus ?? 'pending';
      const status = parent.archived ? 'done' : normalizeTaskStatus(nativeStatus);
      const scopeText = content ?? declaration.context ?? declaration.filename;
      const matchedScopes = subprojects.filter((child) =>
        (child.pathPrefixes ?? []).some((prefix) => scopeText.includes(prefix))
        || (!content && (child.scopeHints ?? []).some((hint) => scopeText.toLowerCase().includes(hint.toLowerCase())))
      );
      const projectId = matchedScopes.length === 1 ? matchedScopes[0].id : config.projectId;
      const scopeLabels = matchedScopes.length
        ? matchedScopes.map((child) => `scope:${child.scope ?? child.code.toLowerCase()}`)
        : ['scope:unassigned'];
      const sourceId = `task:${nativeId}:chunk:${declaration.filename.replace(/\.md$/i, '')}`;
      tasks.push({
        sourceId,
        parentSourceId: parent.sourceId,
        projectId,
        kind: 'chunk',
        archived: Boolean(parent.archived),
        title: parsed?.title?.replace(/^Chunk\s+\d+[a-z]?:?\s*/i, '') ?? declaration.filename.replace(/\.md$/i, '').replace(/^\d+[a-z]?-/, '').replace(/-/g, ' '),
        summary: content ? shortSummary(content, `Declared work for ${parent.title}.`) : `Declared by T${nativeId}; the chunk file will be created when work starts.`,
        status,
        priority: parent.priority,
        sequence,
        blockedBySourceIds: [],
        labels: [config.taskProjection.source.system, 'chunk', ...scopeLabels, ...(content ? [] : ['declared:not-created']), ...(parent.archived ? ['history:archived'] : [])],
        sourceUrl: repositoryFileUrl(config, content ? chunkPath : readmePath),
        revision: content ? fileRevision(content) : undefined,
        sourcePath: content ? chunkPath : readmePath,
        sourceFormat: content ? 'markdown-task-file-v1' : 'hierarchical-task-repo-v1',
        writeback: Boolean(content && !parent.archived),
        statusMap: content ? {
          backlog: 'pending', ready: 'pending', in_progress: 'in_progress', blocked: 'blocked', review: 'review', done: 'complete', cancelled: 'cancelled'
        } : undefined
      });
    }
  }
  return { tasks, subprojects: subprojects.map(({ id, name, code, summary, lead, repository }) => ({ id, name, code, summary, ...(lead ? { lead } : {}), ...(repository ? { repository } : {}) })) };
}

async function nativeTaskUrl(config, source, item) {
  if (!config.repository?.url) return item.sourceUrl;
  const base = config.repository.url.replace(/\/$/, '');
  const branch = config.repository.defaultBranch ?? 'main';
  const numericId = item.sourceId.match(/(?:^|:)(\d+)$/)?.[1];
  if (numericId) {
    const taskDirectory = taskSourcePath(dirname(source.path));
    try {
      const match = (await readdir(taskDirectory, { withFileTypes: true }))
        .find((entry) => entry.isDirectory() && entry.name.startsWith(`${numericId}-`));
      if (match) return `${base}/tree/${branch}/${dirname(source.path)}/${match.name}`;
    } catch {
      // The explicit source file remains the fallback record.
    }
  }
  return item.sourceUrl ?? `${base}/blob/${branch}/${source.path}`;
}

async function request(apiUrl, path, { method = 'POST', body } = {}) {
  const configuredTimeout = Number(process.env.MC_BRIDGE_TIMEOUT_MS ?? 5000);
  const timeout = Number.isInteger(configuredTimeout) && configuredTimeout >= 250 && configuredTimeout <= 30_000 ? configuredTimeout : 5000;
  const serialized = body === undefined ? undefined : JSON.stringify(body);
  const headers = { 'content-type': 'application/json' };
  if (requestCredential?.keyId && requestCredential?.privateKeyPem) {
    const target = new URL(`${apiUrl}${path}`);
    const timestamp = String(Math.floor(Date.now() / 1000));
    const nonce = randomUUID();
    const canonicalPath = `${target.pathname}${target.search}`;
    const canonical = [method, canonicalPath, createHash('sha256').update(serialized ?? '').digest('hex'), timestamp, nonce].join('\n');
    Object.assign(headers, {
      'x-mc-key-id': requestCredential.keyId,
      'x-mc-timestamp': timestamp,
      'x-mc-nonce': nonce,
      'x-mc-signature': sign(null, Buffer.from(canonical), requestCredential.privateKeyPem).toString('base64url')
    });
  }
  const response = await fetch(`${apiUrl}${path}`, {
    method,
    signal: AbortSignal.timeout(timeout),
    headers,
    ...(serialized === undefined ? {} : { body: serialized })
  });
  const result = await response.json();
  if (!response.ok) throw new Error(result.error ?? `Mission Control returned HTTP ${response.status}`);
  return result;
}

async function syncTasks(config, apiUrl) {
  if (config.taskAuthority === 'provider') {
    return { received: 0, created: 0, updated: 0, unchanged: 0, skipped: 'Canonical task provider is authoritative; legacy projection is disabled.' };
  }
  const combined = [];
  let sourceFormat;
  let sourcePath;
  let subprojects = [];
  for (const source of config.taskSources ?? []) {
    if (source.format === 'hierarchical-task-repo-v1') {
      const hierarchy = await parseHierarchicalTaskRepo(config, source);
      combined.push(...hierarchy.tasks);
      subprojects = hierarchy.subprojects;
      sourceFormat = source.format;
      sourcePath = source.path;
      continue;
    }
    const content = await readFile(taskSourcePath(source.path), 'utf8');
    const revision = fileRevision(content);
    sourceFormat = source.format;
    sourcePath = source.path;
    const parsed = source.format === 'task-json-v1'
      ? parseTaskJson(content)
      : source.format === 'markdown-task-table-v1'
        ? parseMarkdownTaskTable(content)
        : (() => { throw new Error(`Unsupported task source format: ${source.format}`); })();
    for (const item of parsed) combined.push({
      ...item,
      sourceUrl: await nativeTaskUrl(config, source, item),
      revision: item.revision ?? revision
    });
  }
  if (!config.taskSources?.length) return { received: 0, created: 0, updated: 0, unchanged: 0, skipped: 'No task source declared' };
  return request(apiUrl, '/task-projections', { body: {
    schemaVersion: 1,
    projectId: config.projectId,
    source: config.taskProjection.source,
    sourcePath,
    sourceFormat,
    ...(config.connection.grants.includes('tasks:write') ? { writebackConnectionId: config.connection.sourceInstanceId } : {}),
    ...(config.taskSources[0].statusMap ? { statusMap: config.taskSources[0].statusMap } : {}),
    observedAt: new Date().toISOString(),
    reporter: config.taskProjection.reporter,
    ...(config.project ? { project: config.project } : {}),
    ...(subprojects.length ? { subprojects } : {}),
    tasks: combined
  } });
}

export function pulseContinuity(current, state, explicit = {}) {
  const activeStates = ['running', 'waiting', 'queued', 'paused'];
  const inherit = activeStates.includes(state) && current && activeStates.includes(current.state);
  const taskId = explicit.taskId ?? (inherit ? current.taskId : undefined);
  const nextAction = explicit.nextAction ?? (inherit ? current.nextAction : undefined);
  const progress = explicit.progress ?? (inherit ? current.progress : undefined);
  return {
    ...(taskId ? { taskId } : {}),
    ...(nextAction ? { nextAction } : {}),
    ...(progress === undefined ? {} : { progress })
  };
}

export async function preflightTaskLink(config, apiUrl, taskId) {
  try {
    if (config.taskAuthority === 'provider') {
      const providerId = config.taskProvider?.providerId;
      if (!providerId) throw new Error('project manifest has provider authority but no taskProvider.providerId');
      const result = await request(apiUrl, `/tasks/${encodeURIComponent(taskId)}/link-preflight`, {
        body: { projectId: config.projectId, providerId }
      });
      if (result.task?.projectId !== config.projectId) throw new Error(`task belongs to ${result.task?.projectId ?? 'an unknown project'}`);
      return { linked: true, outcome: result.outcome, task: result.task };
    }
    const task = await request(apiUrl, `/tasks/${encodeURIComponent(taskId)}`, { method: 'GET' });
    if (task.projectId !== config.projectId) throw new Error(`task belongs to ${task.projectId}, not ${config.projectId}`);
    return { linked: true, outcome: 'projected', task };
  } catch (error) {
    return {
      linked: false,
      outcome: 'not_published',
      taskId,
      detail: error instanceof Error ? error.message : String(error)
    };
  }
}

export async function publishPulse(config, apiUrl, state, args, endpoint = '/pulses') {
  const summary = option(args, '--summary');
  if (!summary) throw new Error('pulse requires --summary TEXT');
  const progressValue = option(args, '--progress');
  const progress = progressValue === undefined ? undefined : Number(progressValue);
  if (progress !== undefined && (!Number.isInteger(progress) || progress < 0 || progress > 100)) throw new Error('--progress must be an integer from 0 to 100');
  const explicit = {
    taskId: option(args, '--task'),
    nextAction: option(args, '--next'),
    progress
  };
  let current;
  if (['running', 'waiting', 'queued', 'paused'].includes(state)
    && (explicit.taskId === undefined || explicit.nextAction === undefined || explicit.progress === undefined)) {
    try {
      const query = new URLSearchParams({ projectId: config.projectId });
      const pulses = await request(apiUrl, `/pulses?${query}`, { method: 'GET' });
      current = pulses.find((pulse) => pulse.source.instanceId === config.reporting.source.instanceId);
    } catch {
      // Reporting must remain non-blocking; publish the explicit checkpoint without continuity.
    }
  }
  const continuity = pulseContinuity(current, state, explicit);
  if (continuity.taskId) {
    const preflight = await preflightTaskLink(config, apiUrl, continuity.taskId);
    if (!preflight.linked) {
      process.stderr.write(`[mission-control] task pulse not published: ${preflight.detail}\n`);
      return preflight;
    }
  }
  return request(apiUrl, endpoint, { body: {
    schemaVersion: 1,
    idempotencyKey: `${config.reporting.source.instanceId}:${Date.now()}:${randomUUID()}`,
    projectId: config.projectId,
    source: { ...config.reporting.source, ...(config.repository?.url ? { sourceUrl: config.repository.url } : {}) },
    observedAt: new Date().toISOString(),
    reporter: config.reporting.reporter,
    capabilities: config.reporting.capabilities,
    state,
    summary,
    ...continuity,
    ...(['running', 'waiting', 'queued', 'paused'].includes(state) ? { heartbeatAt: new Date().toISOString() } : {})
  } });
}

export async function verifyPulseLane(config, apiUrl, args) {
  try {
    return await publishPulse(config, apiUrl, 'idle', args, '/pulses/verify');
  } catch (error) {
    return {
      outcome: 'not_verified',
      detail: `Could not verify the reporting lane without mutating it: ${error instanceof Error ? error.message : String(error)}`
    };
  }
}

async function acknowledgeWriteback(apiUrl, command, outcome, observedRevision, detail) {
  return request(apiUrl, `/task-writebacks/${encodeURIComponent(command.id)}/ack`, { body: {
    connectionId: command.connectionId,
    outcome,
    ...(observedRevision ? { observedRevision } : {}),
    detail
  } });
}

function commandParser(_source, command, content) {
  if (command.sourceFormat === 'task-json-v1') return parseTaskJson(content);
  if (command.sourceFormat === 'markdown-task-file-v1') return [parseMarkdownTaskFile(content, command.sourceId)];
  return parseMarkdownTaskTable(content);
}

async function reconcileTasks(config, apiUrl) {
  if (config.taskAuthority === 'provider') {
    return { received: 0, applied: 0, conflicts: 0, failed: 0, skipped: 'Canonical task provider is authoritative; legacy write-back is disabled.' };
  }
  if (!config.connection.grants.includes('tasks:write')) {
    return { received: 0, applied: 0, conflicts: 0, failed: 0, skipped: 'Connection has no tasks:write grant' };
  }
  const query = new URLSearchParams({ projectId: config.projectId, connectionId: config.connection.sourceInstanceId });
  const pending = await request(apiUrl, `/task-writebacks?${query}`, { method: 'GET' });
  const groups = new Map();
  for (const command of pending) {
    const source = (config.taskSources ?? []).find((item) =>
      (item.path === command.sourcePath && item.format === command.sourceFormat)
      || (item.format === 'hierarchical-task-repo-v1'
        && ((command.sourceFormat === 'task-json-v1' && command.sourcePath === item.path)
          || (command.sourceFormat === 'markdown-task-file-v1'
            && command.sourcePath.startsWith(`${item.taskRoot ?? dirname(item.path)}/`))))
    );
    if (!source) {
      await acknowledgeWriteback(apiUrl, command, 'failed', undefined, 'Command source is not declared by this project manifest.');
      continue;
    }
    const key = `${command.sourceFormat}:${command.sourcePath}`;
    groups.set(key, { source, commands: [...(groups.get(key)?.commands ?? []), command] });
  }

  let applied = 0;
  let conflicts = 0;
  let failed = pending.length - [...groups.values()].reduce((sum, group) => sum + group.commands.length, 0);
  for (const { source, commands } of groups.values()) {
    const path = taskSourcePath(commands[0].sourcePath);
    let content;
    try {
      content = await readFile(path, 'utf8');
    } catch (error) {
      for (const command of commands) {
        await acknowledgeWriteback(apiUrl, command, 'failed', undefined, `Could not read ${source.path}: ${error instanceof Error ? error.message : String(error)}`);
        failed += 1;
      }
      continue;
    }
    const observedRevision = fileRevision(content);
    const eligible = [];
    const parsed = commandParser(source, commands[0], content);
    for (const command of commands) {
      if (command.expectedRevision === observedRevision) {
        eligible.push(command);
        continue;
      }
      const current = parsed.find((item) => item.sourceId === command.sourceId);
      if (current?.status === command.patch.status) {
        await acknowledgeWriteback(apiUrl, command, 'succeeded', observedRevision, 'Native source already contains the requested state.');
        applied += 1;
      } else {
        await acknowledgeWriteback(apiUrl, command, 'conflict', observedRevision, `Expected ${command.expectedRevision}; observed ${observedRevision}.`);
        conflicts += 1;
      }
    }
    if (!eligible.length) continue;
    try {
      let next = content;
      for (const command of eligible) {
        next = command.sourceFormat === 'task-json-v1'
          ? applyTaskJsonPatch(next, command.sourceId, command.patch)
          : command.sourceFormat === 'markdown-task-file-v1'
            ? applyMarkdownTaskFilePatch(next, command.sourceId, command.patch)
            : applyMarkdownTaskPatch(next, command.sourceId, command.patch);
      }
      const temporary = `${path}.mission-control-${randomUUID()}.tmp`;
      await writeFile(temporary, next, 'utf8');
      await rename(temporary, path);
      const nextRevision = fileRevision(next);
      for (const command of eligible) {
        await acknowledgeWriteback(apiUrl, command, 'succeeded', nextRevision, `Patched ${source.path}; repository commit remains project-owned.`);
        applied += 1;
      }
    } catch (error) {
      for (const command of eligible) {
        await acknowledgeWriteback(apiUrl, command, 'failed', observedRevision, error instanceof Error ? error.message : String(error));
        failed += 1;
      }
    }
  }
  const projection = await syncTasks(config, apiUrl);
  return { received: pending.length, applied, conflicts, failed, projection };
}

async function main() {
  const [family, action, ...args] = process.argv.slice(2);
  const config = await loadProjectContext();
  requestCredential = config.credential;
  const apiUrl = (process.env.MC_API_URL ?? config.apiUrl ?? 'http://127.0.0.1:4200/api').replace(/\/$/, '');
  const quiet = process.argv.includes('--quiet');
  const result = family === 'tasks' && action === 'sync'
    ? await syncTasks(config, apiUrl)
    : family === 'tasks' && action === 'reconcile'
      ? await reconcileTasks(config, apiUrl)
    : family === 'connection' && action === 'register'
      ? await request(apiUrl, '/connections', { body: config.connection })
    : family === 'connection' && action === 'request'
      ? {
          schemaVersion: 1,
          kit: config.kit,
          keyId: config.credential.keyId,
          publicKeyPem: config.credential.publicKeyPem,
          projectId: config.projectId,
          sourceInstanceId: config.connection.sourceInstanceId,
          actor: config.reporting.reporter,
          grants: config.connection.grants,
          connection: config.connection
        }
    : family === 'project' && action === 'register'
      ? await request(apiUrl, '/projects', { body: config.registration })
    : family === 'pulse' && action === 'verify'
      ? await verifyPulseLane(config, apiUrl, args)
    : family === 'pulse'
      ? await publishPulse(config, apiUrl, action, args)
      : family === 'work' && ['start', 'checkpoint', 'wait', 'finish', 'fail'].includes(action)
        ? await publishPulse(config, apiUrl, { start: 'running', checkpoint: 'running', wait: 'waiting', finish: 'completed', fail: 'failed' }[action], args)
      : (() => { throw new Error('Usage: bridge.mjs project register | connection request|register | tasks sync | tasks reconcile | pulse verify|STATE --summary TEXT [--task TASK] [--next TEXT] [--progress 0..100] | work start|checkpoint|wait|finish|fail --task TASK --summary TEXT'); })();
  if (!quiet) process.stdout.write(`${JSON.stringify(result, null, 2)}\n`);
}

if (process.argv[1] && basename(process.argv[1]) === 'bridge.mjs') {
  main().catch((error) => {
    process.stderr.write(`[mission-control] ${error instanceof Error ? error.message : String(error)}\n`);
    process.exitCode = 1;
  });
}
