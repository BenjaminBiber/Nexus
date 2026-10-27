import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { HarnessHome } from '../../../src/registry/HarnessHome.js';
import { FavoritePersistence } from '../../../src/favorites/FavoritePersistence.js';
import type { Favorite } from '../../../src/favorites/Favorite.js';

const fav: Favorite = {
  projectId: 'p', sessionId: 's1', note: 'N', tickets: ['T-0001'], cwd: 'D:/wt', markedAt: '2026-10-10T10:00:00.000Z',
  lastState: 'idle', lastEventAt: '2026-10-10T10:00:00.000Z', resumedAs: [],
};

describe('FavoritePersistence', () => {
  let dir: string;
  let home: HarnessHome;
  let persistence: FavoritePersistence;

  beforeEach(() => {
    dir = mkdtempSync(join(tmpdir(), 'harness-fav-'));
    home = new HarnessHome({ HARNESS_HOME: join(dir, 'neu') });
    persistence = new FavoritePersistence(home);
  });
  afterEach(() => rmSync(dir, { recursive: true, force: true }));

  it('hat Pfade für Favoriten und Sicherungen im Harness-Ordner', () => {
    expect(home.favoritesFile.replace(/\\/g, '/')).toMatch(/neu\/favorites\.json$/);
    expect(home.backupsDir.replace(/\\/g, '/')).toMatch(/neu\/backups$/);
  });

  it('liefert ohne Datei eine leere Liste', async () => {
    expect(await persistence.read()).toEqual({ favorites: [], recovered: false });
  });

  it('schreibt und liest Favoriten, auch bei noch nicht vorhandenem Ordner', async () => {
    await persistence.write([fav]);
    expect(await persistence.read()).toEqual({ favorites: [fav], recovered: false });
  });

  it('sichert eine beschädigte Datei nach .bak und startet leer', async () => {
    mkdirSync(join(dir, 'neu'), { recursive: true });
    writeFileSync(home.favoritesFile, '{kaputt');
    const result = await persistence.read();
    expect(result).toEqual({ favorites: [], recovered: true });
    expect(readFileSync(`${home.favoritesFile}.bak`, 'utf8')).toBe('{kaputt');
    expect(existsSync(home.favoritesFile)).toBe(false);
  });
});
