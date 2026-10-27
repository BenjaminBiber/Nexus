import { describe, it, expect, beforeAll, afterAll } from 'vitest';
import { spawn } from 'node:child_process';
import { writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { TempRepo } from '../unit/helpers/TempRepo.js';
import { HarnessProject } from '../../src/project/HarnessProject.js';
import { Counter } from '../../src/counter/Counter.js';
import { HarnessError } from '../../src/errors/HarnessError.js';

const CORE = resolve(__dirname, '../..');
const ROOT = resolve(CORE, '../..');
const WORKER = join(CORE, 'tests/concurrency/worker.ts');
const TSX_LOADER = pathToFileURL(join(ROOT, 'node_modules/tsx/dist/esm/index.mjs')).href;

// Startet einen Hilfsprozess und liefert die gezogenen Nummern
function runWorker(cwd: string, count: number): Promise<number[]> {
  return new Promise((resolvePromise, reject) => {
    const child = spawn(process.execPath, ['--import', TSX_LOADER, WORKER, cwd, String(count)], { stdio: ['ignore', 'pipe', 'pipe'], windowsHide: true });
    let out = '';
    let err = '';
    child.stdout.on('data', (d) => (out += d));
    child.stderr.on('data', (d) => (err += d));
    child.on('close', (code) => (code === 0 ? resolvePromise(JSON.parse(out)) : reject(new Error(err))));
  });
}

describe('Nebenläufigkeit von Zähler und Sperre', () => {
  let repo: TempRepo;
  let worktree: string;

  beforeAll(async () => {
    repo = new TempRepo();
    worktree = repo.addWorktree('wt');
    await HarnessProject.init(repo.path);
  });
  afterAll(() => repo.dispose());

  it('vergibt bei 10 Prozessen in 2 Worktrees 200 verschiedene, lückenlose Nummern', async () => {
    const runs = Array.from({ length: 10 }, (_, i) => runWorker(i % 2 === 0 ? repo.path : worktree, 20));
    const numbers = (await Promise.all(runs)).flat();
    expect(numbers).toHaveLength(200);
    expect([...numbers].sort((a, b) => a - b)).toEqual(Array.from({ length: 200 }, (_, i) => i + 1));
  });

  it('stoppt bei beschädigter Zähler-Datei, statt bei 1 zu beginnen', async () => {
    const project = await HarnessProject.open(repo.path);
    writeFileSync(project.layout.countersFile, '{kaputt');
    const counter = new Counter(project.layout);
    await expect(counter.next('session')).rejects.toThrow(HarnessError);
    await expect(counter.next('session')).rejects.toThrow(/beschädigt/);
  });
});
