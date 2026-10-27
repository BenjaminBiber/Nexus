import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { TempRepo } from '../helpers/TempRepo.js';
import { MainCheckout } from '../../../src/dispatch/git/MainCheckout.js';

describe('MainCheckout', () => {
  let repo: TempRepo;
  const checkout = new MainCheckout();

  beforeEach(() => {
    repo = new TempRepo();
    repo.git('config', 'user.name', 't');
    repo.git('config', 'user.email', 't@t');
    writeFileSync(join(repo.path, 'tracked.txt'), 'a');
    repo.git('add', '.');
    repo.git('commit', '-q', '-m', 'tracked');
  });
  afterEach(() => repo.dispose());

  it('meldet Branch und einen sauberen Ordner', () => {
    const branch = repo.git('rev-parse', '--abbrev-ref', 'HEAD');
    expect(checkout.inspect(repo.path)).toEqual({ branch, busy: null, blockingFiles: [], stagedFiles: [] });
  });

  it('listet geänderte und neue Dateien außer der Plugin-Einstellung', () => {
    writeFileSync(join(repo.path, 'tracked.txt'), 'b');
    mkdirSync(join(repo.path, 'config'));
    writeFileSync(join(repo.path, 'config', 'app.yaml'), 'x');
    writeFileSync(join(repo.path, 'neu ä.txt'), 'x');
    mkdirSync(join(repo.path, '.claude'));
    writeFileSync(join(repo.path, '.claude', 'settings.local.json'), '{}');
    expect(checkout.inspect(repo.path).blockingFiles).toEqual(['config/app.yaml', 'neu ä.txt', 'tracked.txt']);
  });

  it('lässt den Spec-Zeiger .specify/feature.json durch, solange er nicht gestaged ist', () => {
    mkdirSync(join(repo.path, '.specify'));
    writeFileSync(join(repo.path, '.specify', 'feature.json'), '{}');
    expect(checkout.inspect(repo.path).blockingFiles).toEqual([]);
    repo.git('add', '.specify/feature.json');
    expect(checkout.inspect(repo.path).blockingFiles).toEqual(['.specify/feature.json']);
  });

  it('erkennt Detached HEAD und laufenden Merge', () => {
    repo.git('checkout', '-q', '--detach');
    expect(checkout.inspect(repo.path).branch).toBeNull();
    repo.git('checkout', '-q', '-');
    writeFileSync(join(repo.path, '.git', 'MERGE_HEAD'), repo.git('rev-parse', 'HEAD'));
    expect(checkout.inspect(repo.path).busy).toBe('Im Hauptordner läuft gerade ein Merge.');
  });

  it('nennt die gestagten Dateien gesondert', () => {
    writeFileSync(join(repo.path, 'tracked.txt'), 'b');
    writeFileSync(join(repo.path, 'gestagt.txt'), 'x');
    repo.git('add', 'gestagt.txt');
    const state = checkout.inspect(repo.path);
    expect(state.blockingFiles).toEqual(['gestagt.txt', 'tracked.txt']);
    expect(state.stagedFiles).toEqual(['gestagt.txt']);
  });

  it('nennt bei Umbenennung den neuen Pfad', () => {
    writeFileSync(join(repo.path, 'alt.yaml'), 'inhalt');
    repo.git('add', '.');
    repo.git('commit', '-q', '-m', 'alt');
    repo.git('mv', 'alt.yaml', 'neu.yaml');
    expect(checkout.inspect(repo.path).blockingFiles).toEqual(['neu.yaml']);
  });

  it('lässt die Plugin-Einstellung immer durch, auch gestagt', () => {
    mkdirSync(join(repo.path, '.claude'));
    writeFileSync(join(repo.path, '.claude', 'settings.local.json'), '{}');
    repo.git('add', '-f', '.claude/settings.local.json');
    expect(checkout.inspect(repo.path).blockingFiles).toEqual([]);
  });

  it('lässt die Rules für Doku- und Board-Regeln durch, auch ohne Eintrag in info/exclude', () => {
    mkdirSync(join(repo.path, '.claude', 'rules'), { recursive: true });
    writeFileSync(join(repo.path, '.claude', 'rules', 'ai-harness-docs.md'), 'x');
    writeFileSync(join(repo.path, '.claude', 'rules', 'ai-harness-board.md'), 'x');
    expect(checkout.inspect(repo.path).blockingFiles).toEqual([]);
  });
});
