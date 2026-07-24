// mission-control:managed version=2.2.0
import { readFile, readdir } from 'node:fs/promises';
import { dirname, isAbsolute, join, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = dirname(dirname(fileURLToPath(import.meta.url)));

async function json(path) {
  return JSON.parse(await readFile(path, 'utf8'));
}

export async function loadProjectContext() {
  const project = await json(join(root, '.mission-control/project.json'));
  if (project.reporting && project.connection) return project;
  const explicit = process.env.MC_ENROLLMENT_PATH;
  if (explicit) {
    const path = isAbsolute(explicit) ? explicit : resolve(root, explicit);
    if (path !== root && !path.startsWith(`${root}${sep}`)) throw new Error('MC_ENROLLMENT_PATH must stay inside the project checkout');
    const enrollment = await json(path);
    return { ...project, ...enrollment, enrollment };
  }
  const directory = join(root, '.mission-control/enrollments');
  const files = (await readdir(directory).catch(() => []))
    .filter((name) => name.endsWith('.local.json') && !name.endsWith('.request.local.json'));
  const candidates = [];
  for (const name of files) {
    const enrollment = await json(join(directory, name));
    if (!process.env.MC_ENROLLMENT_TOOL || enrollment.reporting?.source?.kind === process.env.MC_ENROLLMENT_TOOL) candidates.push({ name, enrollment });
  }
  if (candidates.length === 1) return { ...project, ...candidates[0].enrollment, enrollment: candidates[0].enrollment };
  if (!candidates.length) throw new Error(`No local Mission Control enrollment matches ${process.env.MC_ENROLLMENT_TOOL ?? 'this process'}; rerun project setup for this tool.`);
  throw new Error(`Multiple local Mission Control enrollments match; set MC_ENROLLMENT_PATH to one of: ${candidates.map(({ name }) => `.mission-control/enrollments/${name}`).join(', ')}`);
}

export { root as projectRoot };
