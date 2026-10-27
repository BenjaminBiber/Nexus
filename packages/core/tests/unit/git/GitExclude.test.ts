import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { GitExclude } from '../../../src/git/GitExclude.js';

describe('GitExclude', () => {
  let commonDir: string;
  const exclude = new GitExclude();
  const read = () => readFileSync(join(commonDir, 'info', 'exclude'), 'utf8');

  beforeEach(() => {
    commonDir = mkdtempSync(join(tmpdir(), 'harness-exclude-'));
  });

  afterEach(() => rmSync(commonDir, { recursive: true, force: true }));

  it('legt info/exclude mit der Zeile an, wenn es die Datei noch nicht gibt', async () => {
    expect(await exclude.add(commonDir, '/.claude/rules/x.md')).toBe(true);
    expect(read()).toBe('/.claude/rules/x.md\n');
  });

  it('ist wiederholbar und meldet false, wenn die Zeile schon eingetragen ist', async () => {
    await exclude.add(commonDir, '/.claude/rules/x.md');
    expect(await exclude.add(commonDir, '/.claude/rules/x.md')).toBe(false);
    expect(read()).toBe('/.claude/rules/x.md\n');
  });

  it('hängt an vorhandenen Inhalt an, auch wenn der letzte Zeilenumbruch fehlt', async () => {
    mkdirSync(join(commonDir, 'info'));
    writeFileSync(join(commonDir, 'info', 'exclude'), '# Kommentar\n*.log');
    await exclude.add(commonDir, '/.claude/rules/x.md');
    expect(read()).toBe('# Kommentar\n*.log\n/.claude/rules/x.md\n');
  });
});
