import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { mkdtempSync, readFileSync, rmSync, writeFileSync, existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { EnvFileWriter } from '../../../src/hooks/EnvFileWriter.js';

describe('EnvFileWriter', () => {
  let dir: string;
  let file: string;
  const writer = new EnvFileWriter();

  beforeEach(() => {
    dir = mkdtempSync(join(tmpdir(), 'harness-env-'));
    file = join(dir, 'ordner mit leerzeichen', 'env.sh');
  });
  afterEach(() => rmSync(dir, { recursive: true, force: true }));

  it('legt die Datei an und schreibt export-Zeilen', async () => {
    const lines = await writer.append(file, { HARNESS_SESSION: 'abc', HARNESS_TICKET: 'T-0042' }, {});
    expect(lines).toBe(2);
    expect(readFileSync(file, 'utf8')).toBe("export HARNESS_SESSION='abc'\nexport HARNESS_TICKET='T-0042'\n");
  });

  it('hängt an vorhandenen Inhalt an', async () => {
    await writer.append(file, { A: '1' }, {});
    writeFileSync(file, readFileSync(file, 'utf8') + 'export B=2\n');
    await writer.append(file, { C: '3' }, {});
    expect(readFileSync(file, 'utf8')).toBe("export A='1'\nexport B=2\nexport C='3'\n");
  });

  it('überschreibt keine bereits gesetzte Umgebungsvariable', async () => {
    const lines = await writer.append(file, { HARNESS_TICKET: 'T-0001', HARNESS_SESSION: 's' }, { HARNESS_TICKET: 'T-0099' });
    expect(lines).toBe(1);
    expect(readFileSync(file, 'utf8')).toBe("export HARNESS_SESSION='s'\n");
  });

  it('maskiert Anführungszeichen', async () => {
    await writer.append(file, { X: "a'b" }, {});
    expect(readFileSync(file, 'utf8')).toBe("export X='a'\\''b'\n");
  });

  it('tut nichts ohne Dateinamen', async () => {
    expect(await writer.append(undefined, { A: '1' }, {})).toBe(0);
    expect(existsSync(file)).toBe(false);
  });
});
