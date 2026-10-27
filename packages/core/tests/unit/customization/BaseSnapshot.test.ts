import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { BaseSnapshot } from '../../../src/customization/BaseSnapshot.js';

describe('BaseSnapshot', () => {
  let dir: string;
  const snapshot = new BaseSnapshot();
  const file = (name: string, text: string) => {
    const path = join(dir, name);
    mkdirSync(join(path, '..'), { recursive: true });
    writeFileSync(path, text);
    return path;
  };

  beforeEach(() => { dir = mkdtempSync(join(tmpdir(), 'harness-base-')); });
  afterEach(() => rmSync(dir, { recursive: true, force: true }));

  it('erkennt fehlende, gleiche und veraltete Basis einer Datei', () => {
    const standard = file('std.md', 'neu\n');
    expect(snapshot.state(join(dir, 'fehlt.md'), standard)).toBe('unknown');
    expect(snapshot.state(file('gleich.md', 'neu\r\n'), standard)).toBe('current');
    expect(snapshot.state(file('alt.md', 'alt\n'), standard)).toBe('outdated');
  });

  it('vergleicht Ordner mit allen Dateien', () => {
    file('std/SKILL.md', 'a');
    file('std/extra.md', 'b');
    file('base/SKILL.md', 'a');
    expect(snapshot.state(join(dir, 'base'), join(dir, 'std'))).toBe('outdated');
    snapshot.save(join(dir, 'std'), join(dir, 'base'));
    expect(snapshot.state(join(dir, 'base'), join(dir, 'std'))).toBe('current');
  });

  it('liefert die Unterschiede Basis → Standard', () => {
    const diff = snapshot.diff(file('alt.md', 'alt\n'), file('std.md', 'neu\n'));
    expect(diff).toContain('-alt');
    expect(diff).toContain('+neu');
  });

  it('fasst Zustand, Warnung und Diff zusammen', () => {
    const standard = file('std.md', 'neu\n');
    expect(snapshot.check(file('gleich.md', 'neu\n'), standard)).toEqual({ state: 'current', warnings: [] });
    const outdated = snapshot.check(file('alt.md', 'alt\n'), standard);
    expect(outdated.state).toBe('outdated');
    expect(outdated.warnings.map((w) => w.code)).toEqual(['base-outdated']);
    expect(outdated.diff).toContain('+neu');
    expect(snapshot.check(join(dir, 'fehlt.md'), standard).warnings.map((w) => w.code)).toEqual(['base-unknown']);
  });

  it('legt die Basis aus Text an und formatiert JSON einheitlich', () => {
    const base = join(dir, '.base', 'workflow.json');
    snapshot.saveText(BaseSnapshot.formatJson([1]), base);
    expect(snapshot.state(base, file('std.json', '[\n  1\n]\n'))).toBe('current');
    expect(BaseSnapshot.formatJson([1])).toBe('[\n  1\n]\n');
  });
});
