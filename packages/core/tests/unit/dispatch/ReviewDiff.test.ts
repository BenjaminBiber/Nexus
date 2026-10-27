import { writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { HarnessError } from '../../../src/errors/HarnessError.js';
import { ReviewDiff } from '../../../src/dispatch/git/ReviewDiff.js';
import { TempRepo } from '../helpers/TempRepo.js';

describe('ReviewDiff', () => {
  let repo: TempRepo;
  const diff = new ReviewDiff();

  // Schreibt eine Datei im Projekt und committet sie
  function commit(files: Record<string, string | Buffer>, message: string): void {
    for (const [name, content] of Object.entries(files)) writeFileSync(join(repo.path, name), content);
    repo.git('add', '-A');
    repo.git('-c', 'user.name=t', '-c', 'user.email=t@t', 'commit', '-q', '-m', message);
  }

  beforeEach(() => {
    repo = new TempRepo();
    commit({ 'alt.txt': 'a\nb\nc\n', 'weg.txt': 'weg\n', 'umbenennen.txt': 'eins\nzwei\ndrei\nvier\nfünf\n' }, 'basis');
    repo.git('checkout', '-q', '-b', 'ticket/x');
    commit({ 'alt.txt': 'a\nneu\nc\n', 'neu.txt': 'neu\n', 'bild.bin': Buffer.from([0, 1, 2, 0, 255]) }, 'ticket');
    repo.git('rm', '-q', 'weg.txt');
    repo.git('mv', 'umbenennen.txt', 'umbenannt.txt');
    repo.git('-c', 'user.name=t', '-c', 'user.email=t@t', 'commit', '-q', '-m', 'ticket 2');
    repo.git('checkout', '-q', '-');
    commit({ 'nur-ziel.txt': 'ziel\n' }, 'ziel');
  });
  afterEach(() => repo.dispose());

  it('liefert Ziel, Basis und geänderte Dateien ab der Merge-Basis', () => {
    const list = diff.files(repo.path, 'ticket/x');
    expect(list.target).toBe(repo.git('symbolic-ref', '--short', 'HEAD'));
    expect(list.base).toBe(repo.git('merge-base', 'HEAD', 'ticket/x'));
    const byPath = Object.fromEntries(list.files.map((f) => [f.path, f]));
    expect(Object.keys(byPath).sort()).toEqual(['alt.txt', 'bild.bin', 'neu.txt', 'umbenannt.txt', 'weg.txt']);
    expect(byPath['alt.txt']).toMatchObject({ status: 'M', additions: 1, deletions: 1, binary: false });
    expect(byPath['neu.txt']).toMatchObject({ status: 'A', additions: 1, deletions: 0 });
    expect(byPath['weg.txt']).toMatchObject({ status: 'D', additions: 0, deletions: 1 });
    expect(byPath['umbenannt.txt']).toMatchObject({ status: 'R', oldPath: 'umbenennen.txt' });
    expect(byPath['bild.bin']).toMatchObject({ binary: true, additions: 0, deletions: 0 });
  });

  it('liefert den Patch einer Datei der Liste', () => {
    const { patch, truncated } = diff.file(repo.path, 'ticket/x', 'alt.txt');
    expect(patch).toContain('+neu');
    expect(patch).toContain('-b');
    expect(truncated).toBe(false);
    expect(diff.file(repo.path, 'ticket/x', 'umbenannt.txt').patch).toContain('rename to umbenannt.txt');
  });

  it('lehnt Pfade außerhalb der Liste und Optionen ab', () => {
    for (const path of ['nur-ziel.txt', '--output=x', 'gibt-es-nicht.txt']) {
      expect(() => diff.file(repo.path, 'ticket/x', path)).toThrowError(expect.objectContaining({ code: 'review-file-unknown' }) as unknown as HarnessError);
    }
    expect(() => diff.files(repo.path, '--output=x')).toThrow(/Ungültiger Branch/);
  });

  it('kürzt sehr große Patches', () => {
    repo.git('checkout', '-q', 'ticket/x');
    commit({ 'gross.txt': 'zeile mit etwas text\n'.repeat(20_000) }, 'gross');
    repo.git('checkout', '-q', '-');
    const { patch, truncated } = diff.file(repo.path, 'ticket/x', 'gross.txt');
    expect(truncated).toBe(true);
    expect(patch.length).toBe(200_000);
  });
});

describe('ReviewDiff mit Zielbranch', () => {
  let repo: TempRepo;
  const diff = new ReviewDiff();

  // Schreibt eine Datei im Projekt und committet sie
  function commit(files: Record<string, string>, message: string): void {
    for (const [name, content] of Object.entries(files)) writeFileSync(join(repo.path, name), content);
    repo.git('add', '-A');
    repo.git('-c', 'user.name=t', '-c', 'user.email=t@t', 'commit', '-q', '-m', message);
  }

  // main (m.txt), release/1 von main mit r.txt, ticket/x von release/1 mit t.txt; ausgecheckt bleibt main
  beforeEach(() => {
    repo = new TempRepo();
    repo.git('branch', '-M', 'main');
    commit({ 'm.txt': 'main\n' }, 'basis');
    repo.git('checkout', '-q', '-b', 'release/1');
    commit({ 'r.txt': 'release\n' }, 'release');
    repo.git('checkout', '-q', '-b', 'ticket/x');
    commit({ 't.txt': 'ticket\n' }, 'ticket');
    repo.git('checkout', '-q', 'main');
  });
  afterEach(() => repo.dispose());

  it('vergleicht gegen den Zielbranch statt gegen den ausgecheckten Branch', () => {
    const list = diff.files(repo.path, 'ticket/x', 'release/1');
    expect(list.target).toBe('release/1');
    expect(list.base).toBe(repo.git('rev-parse', 'release/1'));
    expect(list.files.map((f) => f.path)).toEqual(['t.txt']);
  });

  it('bleibt ohne Zielbranch beim ausgecheckten Branch', () => {
    for (const target of [undefined, null, '']) {
      const list = diff.files(repo.path, 'ticket/x', target);
      expect(list.target).toBe('main');
      expect(list.base).toBe(repo.git('merge-base', 'HEAD', 'ticket/x'));
      expect(list.files.map((f) => f.path).sort()).toEqual(['r.txt', 't.txt']);
    }
  });

  it('fällt bei nicht vorhandenem Zielbranch auf HEAD zurück', () => {
    const list = diff.files(repo.path, 'ticket/x', 'release/2');
    expect(list.target).toBe('main');
    expect(list.files.map((f) => f.path).sort()).toEqual(['r.txt', 't.txt']);
  });

  it('liefert den Patch gegen den Zielbranch und lehnt Dateien nur des Zwischenstands ab', () => {
    expect(diff.file(repo.path, 'ticket/x', 't.txt', 'release/1').patch).toContain('+ticket');
    expect(() => diff.file(repo.path, 'ticket/x', 'r.txt', 'release/1')).toThrowError(expect.objectContaining({ code: 'review-file-unknown' }) as unknown as HarnessError);
    expect(diff.file(repo.path, 'ticket/x', 'r.txt').patch).toContain('+release');
  });

  it('zeigt im Patch einer auch auf dem Zielbranch vorhandenen Datei nur die Änderung des Tickets', () => {
    // ticket/y zweigt von release/1 ab und hängt eine Zeile an r.txt an, die es auf release/1 schon gibt
    repo.git('checkout', '-q', '-b', 'ticket/y', 'release/1');
    commit({ 'r.txt': 'release\nticket-zeile\n' }, 'ticket y');
    repo.git('checkout', '-q', 'main');

    const against = diff.file(repo.path, 'ticket/y', 'r.txt', 'release/1').patch;
    expect(against).toContain('+ticket-zeile');
    expect(against).not.toContain('+release');
    expect(against).not.toContain('new file');
    // Ohne Zielbranch (HEAD = main) ist r.txt für den Vergleich neu: auch die Zeile von release/1 gehört zur Änderung
    const head = diff.file(repo.path, 'ticket/y', 'r.txt').patch;
    expect(head).toContain('new file');
    expect(head).toContain('+release');
    expect(head).toContain('+ticket-zeile');
  });

  it('lehnt einen Zielbranch ab, den Git als Option lesen würde', () => {
    expect(() => diff.files(repo.path, 'ticket/x', '--output=x')).toThrowError(expect.objectContaining({ code: 'review-branch-invalid' }) as unknown as HarnessError);
  });
});
