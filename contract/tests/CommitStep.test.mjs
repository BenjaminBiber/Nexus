import { execFileSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { CommitStep } from '../lib/CommitStep.mjs';
import { GIT_IDENTITY_ENV } from '../lib/GitIdentity.mjs';
import { Variables } from '../lib/Variables.mjs';

describe('CommitStep', () => {
  let repos;
  let repo;
  let context;

  // Führt Git im Repo aus und liefert die getrimmte Ausgabe
  const git = (args, cwd = repo) =>
    execFileSync('git', args, { cwd, encoding: 'utf8', windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'], env: { ...process.env, ...GIT_IDENTITY_ENV } }).trim();

  beforeEach(() => {
    repos = mkdtempSync(join(tmpdir(), 'harness-contract-repos-'));
    repo = join(repos, 'REPO1');
    mkdirSync(repo, { recursive: true });
    writeFileSync(join(repo, 'a.txt'), 'alt\n');
    writeFileSync(join(repo, 'weg.txt'), 'geht weg\n');
    git(['init', '-q', '-b', 'main']);
    git(['add', '-A']);
    git(['commit', '-q', '-m', 'init']);
    context = { reposDir: repos, variables: new Variables({ REPO1: repo }) };
  });

  afterEach(() => {
    rmSync(repos, { recursive: true, force: true });
  });

  // Führt einen Schritt der Art aus
  const run = (raw) => new CommitStep().run({ raw }, context);

  it('nennt seine eigenen Felder', () => {
    expect(new CommitStep().keys).toEqual(['cwd', 'files', 'remove', 'message']);
  });

  it('schreibt Dateien, committet sie und meldet Branch und Commit', async () => {
    const result = await run({ cwd: '{{REPO1}}', files: { 'src/neu.ts': 'neu\n' }, message: 'BEN-0001: neu' });
    expect(result.response.json.branch).toBe('main');
    expect(result.response.json.commit).toBe(git(['rev-parse', '--short', 'HEAD']));
    expect(git(['log', '-1', '--pretty=%s'])).toBe('BEN-0001: neu');
    expect(git(['show', '--pretty=', '--name-only', 'HEAD'])).toBe('src/neu.ts');
  });

  it('löscht die Dateien aus remove und nimmt die Löschung in den Commit auf', async () => {
    await run({ cwd: '{{REPO1}}', remove: ['weg.txt'], message: 'BEN-0001: aufgeräumt' });
    expect(existsSync(join(repo, 'weg.txt'))).toBe(false);
    expect(git(['show', '--pretty=', '--name-status', 'HEAD'])).toBe('D\tweg.txt');
  });

  it('nimmt nur die genannten Pfade auf, nicht alles im Ordner', async () => {
    writeFileSync(join(repo, 'fremd.txt'), 'gehört nicht dazu\n');
    await run({ cwd: '{{REPO1}}', files: { 'a.txt': 'neu\n' }, message: 'BEN-0001: geändert' });
    expect(git(['show', '--pretty=', '--name-only', 'HEAD'])).toBe('a.txt');
    expect(git(['status', '--porcelain'])).toBe('?? fremd.txt');
  });

  it('zeichnet Ordner, Dateien und Nachricht als Anfrage auf', async () => {
    const result = await run({ cwd: '{{REPO1}}', files: { 'a.txt': 'neu\n' }, remove: ['weg.txt'], message: 'BEN-0001: beides' });
    expect(result.request).toEqual({ kind: 'commit', cwd: repo, files: ['a.txt'], remove: ['weg.txt'], message: 'BEN-0001: beides' });
  });

  it('liefert bei festem Datum und fester Identität dieselbe Commit-Kennung', async () => {
    const first = (await run({ cwd: '{{REPO1}}', files: { 'b.txt': 'fest\n' }, message: 'BEN-0001: fest' })).response.json.commit;
    git(['reset', '-q', '--hard', 'HEAD~1']);
    const second = (await run({ cwd: '{{REPO1}}', files: { 'b.txt': 'fest\n' }, message: 'BEN-0001: fest' })).response.json.commit;
    expect(second).toBe(first);
  });

  it('meldet einen Ordner außerhalb der Fixture-Repos', async () => {
    await expect(run({ cwd: tmpdir(), files: { 'a.txt': 'x' }, message: 'm' })).rejects.toThrow(/liegt nicht in den Fixture-Repos/);
  });

  it('meldet einen Ordner, den es nicht gibt', async () => {
    await expect(run({ cwd: '{{REPO1}}-worktrees/BEN-0001', files: { 'a.txt': 'x' }, message: 'm' })).rejects.toThrow(/gibt es nicht/);
  });

  it('meldet einen Commit ohne Dateien', async () => {
    await expect(run({ cwd: '{{REPO1}}', message: 'm' })).rejects.toThrow(/braucht files oder remove/);
  });

  it('meldet eine fehlende Nachricht', async () => {
    await expect(run({ cwd: '{{REPO1}}', files: { 'a.txt': 'x' } })).rejects.toThrow(/message muss die Commit-Nachricht/);
  });

  it('meldet einen Dateiinhalt, der kein Text ist', async () => {
    await expect(run({ cwd: '{{REPO1}}', files: { 'a.txt': 7 }, message: 'm' })).rejects.toThrow(/files\["a\.txt"\] muss ein Text sein/);
  });

  it('meldet remove mit einer Datei, die es nicht gibt', async () => {
    await expect(run({ cwd: '{{REPO1}}', remove: ['fehlt.txt'], message: 'm' })).rejects.toThrow(/remove nennt fehlt\.txt/);
  });

  it('meldet remove, das keine Liste ist', async () => {
    await expect(run({ cwd: '{{REPO1}}', remove: 'weg.txt', message: 'm' })).rejects.toThrow(/remove muss eine Liste von Pfaden sein/);
  });
});
