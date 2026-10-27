import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { HarnessHome } from '../../../src/registry/HarnessHome.js';
import { BoardRepoFile } from '../../../src/board/BoardRepoFile.js';
import { BoardRepoPaths, MemoryBoardRepoStore } from '../../../src/board/BoardRepoPaths.js';
import type { Board } from '../../../src/board/BoardTypes.js';

const ME = '11111111-1111-1111-1111-111111111111';
const OTHER = '22222222-2222-2222-2222-222222222222';
const board = (id: number, ownerId: string, repoPath: string | null): Board => ({ id, projectId: 1, ownerId, name: `B${id}`, repoPath, createdAt: '' });

describe('BoardRepoFile', () => {
  let dir: string;
  let file: BoardRepoFile;

  beforeEach(() => {
    dir = mkdtempSync(join(tmpdir(), 'harness-repos-'));
    file = new BoardRepoFile(new HarnessHome({ HARNESS_HOME: dir }));
  });
  afterEach(() => rmSync(dir, { recursive: true, force: true }));

  it('liest ohne Datei eine leere Zuordnung', async () => {
    expect(await file.read()).toEqual(new Map());
  });

  it('schreibt die Zuordnung nach board-repos.yaml und liest sie wieder', async () => {
    await file.write(new Map<number, string | null>([[7, 'C:\\Projects\\Arches'], [8, null]]));
    expect(file.file).toBe(join(dir, 'board-repos.yaml'));
    expect(readFileSync(join(dir, 'board-repos.yaml'), 'utf8')).toContain('C:\\Projects\\Arches');
    expect(await file.read()).toEqual(new Map<number, string | null>([[7, 'C:\\Projects\\Arches'], [8, null]]));
  });

  it('meldet kaputtes YAML und ungültige Einträge', async () => {
    writeFileSync(join(dir, 'board-repos.yaml'), ': : [');
    await expect(file.read()).rejects.toMatchObject({ code: 'board-repos-invalid' });
    writeFileSync(join(dir, 'board-repos.yaml'), 'abc: C:/x\n');
    await expect(file.read()).rejects.toMatchObject({ code: 'board-repos-invalid' });
    writeFileSync(join(dir, 'board-repos.yaml'), '7: 12\n');
    await expect(file.read()).rejects.toMatchObject({ code: 'board-repos-invalid' });
  });
});

describe('BoardRepoPaths', () => {
  it('nimmt den lokalen Eintrag, auch für fremde Boards', async () => {
    const paths = new BoardRepoPaths(new MemoryBoardRepoStore(new Map([[1, 'C:/mein'], [2, 'D:/kollege']])), ME);
    const [own, foreign] = await paths.apply([board(1, ME, 'C:/alt'), board(2, OTHER, 'X:/fremd')]);
    expect(own).toMatchObject({ repoPath: 'C:/mein', repoSource: 'local' });
    expect(foreign).toMatchObject({ repoPath: 'D:/kollege', repoSource: 'local' });
  });

  it('fällt nur bei eigenen Boards auf den DB-Wert zurück (Altbestand)', async () => {
    const paths = new BoardRepoPaths(new MemoryBoardRepoStore(), ME);
    const [own, foreign] = await paths.apply([board(1, ME, 'C:/alt'), board(2, OTHER, 'X:/fremd')]);
    expect(own).toMatchObject({ repoPath: 'C:/alt', repoSource: 'legacy' });
    expect(foreign).toMatchObject({ repoPath: null, repoSource: null });
  });

  it('ohne Benutzer gibt es keinen Rückfall', async () => {
    const [own] = await new BoardRepoPaths(new MemoryBoardRepoStore(), null).apply([board(1, ME, 'C:/alt')]);
    expect(own).toMatchObject({ repoPath: null, repoSource: null });
  });

  it('setzt und entfernt den eigenen Pfad; entfernt verdrängt auch den Altbestand', async () => {
    const store = new MemoryBoardRepoStore();
    const paths = new BoardRepoPaths(store, ME);
    await paths.set(1, '  C:/neu  ');
    expect(await paths.one(board(1, ME, 'C:/alt'))).toMatchObject({ repoPath: 'C:/neu', repoSource: 'local' });
    await paths.set(1, null);
    expect(await paths.one(board(1, ME, 'C:/alt'))).toMatchObject({ repoPath: null, repoSource: null });
    await paths.set(1, ' ');
    expect(await store.read()).toEqual(new Map([[1, null]]));
  });
});
