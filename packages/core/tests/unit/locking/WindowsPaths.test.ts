import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { AtomicWriter } from '../../../src/locking/AtomicWriter.js';
import { FileLock } from '../../../src/locking/FileLock.js';

// Windows-Besonderheiten: Pfade mit Leerzeichen und Umlauten, Überschreiben, gleichzeitiges Schreiben
describe('Dateizugriff mit Sonderzeichen im Pfad', () => {
  let dir: string;

  beforeEach(() => {
    dir = join(mkdtempSync(join(tmpdir(), 'harness-win-')), 'Mein Ordner ä ö ü ß');
    mkdirSync(dir);
  });
  afterEach(() => rmSync(join(dir, '..'), { recursive: true, force: true }));

  it('schreibt und überschreibt atomar', async () => {
    const writer = new AtomicWriter();
    const file = join(dir, 'datei mit leerzeichen.json');
    await writer.write(file, 'eins');
    await writer.write(file, 'zwei');
    expect(readFileSync(file, 'utf8')).toBe('zwei');
    expect(readdirSync(dir)).toEqual(['datei mit leerzeichen.json']);
  });

  it('übersteht viele gleichzeitige Schreibzugriffe auf dieselbe Datei', async () => {
    const writer = new AtomicWriter();
    const file = join(dir, 'x.json');
    await Promise.all(Array.from({ length: 30 }, (_, i) => writer.write(file, `Inhalt ${i}`)));
    expect(readFileSync(file, 'utf8')).toMatch(/^Inhalt \d+$/);
    expect(readdirSync(dir).filter((n) => n.endsWith('.tmp'))).toEqual([]);
  });

  it('sperrt und gibt frei in einem Ordner mit Sonderzeichen', async () => {
    const lock = new FileLock(join(dir, 'ä.lock'));
    await lock.withLock(() => undefined);
    expect(readdirSync(dir)).toEqual([]);
  });
});
