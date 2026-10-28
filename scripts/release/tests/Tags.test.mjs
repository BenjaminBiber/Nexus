import { execFileSync } from 'node:child_process';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { highestTag, listTags, tagVersions } from '../lib/Tags.mjs';

describe('Tags', () => {
  it('nimmt die höchste Release-Version und übergeht Vorabversionen', () => {
    expect(highestTag(['v0.9.0', 'v0.10.0', 'v0.10.0-rc.1'])).toBe('0.10.0');
  });

  it('sortiert nach Zahlen, nicht nach Text', () => {
    expect(highestTag(['v0.2.0', 'v0.10.0', 'v0.9.9'])).toBe('0.10.0');
  });

  it('nimmt Tags mit und ohne „v“', () => {
    expect(highestTag(['0.9.0', 'v0.10.0'])).toBe('0.10.0');
  });

  it('teilt Tags in Versionen, Vorabversionen und Übersprungenes', () => {
    expect(tagVersions(['v0.9.0', 'v0.10.0-rc.1', 'v1.0', 'release-2026', 'v1.0.0+build'])).toEqual({
      versions: ['0.9.0', '1.0.0'],
      prerelease: ['v0.10.0-rc.1'],
      skipped: ['v1.0', 'release-2026'],
    });
  });

  it('liefert null, wenn kein Tag eine Release-Version ist', () => {
    expect(highestTag([])).toBeNull();
    expect(highestTag(['v0.1.0-rc.1', 'keine-version'])).toBeNull();
  });

  describe('listTags', () => {
    let root;

    beforeEach(() => {
      root = mkdtempSync(join(tmpdir(), 'harness-release-tags-'));
    });

    afterEach(() => {
      rmSync(root, { recursive: true, force: true });
    });

    const git = (...args) => execFileSync('git', args, { cwd: root, encoding: 'utf8', windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'] });

    it('liest die Tags „v*“ eines Repos und lässt andere Tags liegen', () => {
      git('init', '-q', '-b', 'main');
      writeFileSync(join(root, 'datei.txt'), 'inhalt\n');
      git('add', '-A');
      git('-c', 'user.name=Test', '-c', 'user.email=test@example.invalid', 'commit', '-q', '-m', 'erster');
      for (const tag of ['v0.9.0', 'v0.10.0', 'v0.10.0-rc.1', 'start']) git('tag', tag);
      expect(listTags(root).sort()).toEqual(['v0.10.0', 'v0.10.0-rc.1', 'v0.9.0']);
      expect(highestTag(listTags(root))).toBe('0.10.0');
    });

    it('liefert eine leere Liste für ein Repo ohne Tags', () => {
      git('init', '-q', '-b', 'main');
      expect(listTags(root)).toEqual([]);
    });

    it('liefert null, wenn es dort kein Git-Repo gibt', () => {
      expect(listTags(join(root, 'gibtsnicht'))).toBeNull();
    });
  });
});
