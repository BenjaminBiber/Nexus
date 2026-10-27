import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { WorktreeLinks } from '../../../src/worktrees/WorktreeLinks.js';

describe('WorktreeLinks', () => {
  let dir: string;
  beforeEach(() => {
    dir = mkdtempSync(join(tmpdir(), 'harness-links-'));
  });
  afterEach(() => rmSync(dir, { recursive: true, force: true }));

  // Ziel außerhalb des Worktrees mit Inhalt (wie node_modules des Hauptordners)
  function target(): string {
    const t = join(dir, 'haupt', 'node_modules');
    mkdirSync(join(t, '.bin'), { recursive: true });
    writeFileSync(join(t, '.package-lock.json'), '{}');
    writeFileSync(join(t, '.bin', 'tool'), 'x');
    return t;
  }

  it('findet Junctions, ohne ihnen zu folgen, und entfernt nur den Link; das Ziel bleibt vollständig', () => {
    const t = target();
    const wt = join(dir, 'wt');
    mkdirSync(join(wt, 'app'), { recursive: true });
    writeFileSync(join(wt, 'app', 'a.txt'), 'a');
    symlinkSync(t, join(wt, 'app', 'node_modules'), 'junction');
    const links = new WorktreeLinks();
    expect(links.find(wt)).toEqual([join(wt, 'app', 'node_modules')]);
    expect(links.removeAll(wt)).toEqual({ removed: [join(wt, 'app', 'node_modules')], failed: [] });
    expect(existsSync(join(wt, 'app', 'node_modules'))).toBe(false);
    expect(readFileSync(join(t, '.package-lock.json'), 'utf8')).toBe('{}');
    expect(readFileSync(join(t, '.bin', 'tool'), 'utf8')).toBe('x');
    expect(readFileSync(join(wt, 'app', 'a.txt'), 'utf8')).toBe('a');
  });

  it('fasst einen Ordner, der selbst ein Link ist, nicht an', () => {
    const t = target();
    const wt = join(dir, 'wt-link');
    symlinkSync(t, wt, 'junction');
    expect(new WorktreeLinks().removeAll(wt)).toEqual({ removed: [], failed: [wt] });
    expect(existsSync(join(t, '.package-lock.json'))).toBe(true);
  });
});
