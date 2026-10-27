import { afterEach, describe, expect, it } from 'vitest';
import { realpathSync, rmSync } from 'node:fs';
import { makeTempDir } from './TempDir.js';

describe('makeTempDir', () => {
  let dir = '';
  afterEach(() => {
    if (dir) rmSync(dir, { recursive: true, force: true });
    dir = '';
  });

  it('liefert den langen Pfad, auch wenn TEMP ein 8.3-Kurzpfad ist', () => {
    dir = makeTempDir('harness-tempdir-');
    // Ein 8.3-Kurzname enthält immer eine Tilde; der lange Pfad nie.
    expect(dir).not.toContain('~');
    // Der Pfad ist bereits kanonisch, erneutes Auflösen ändert ihn nicht.
    expect(realpathSync.native(dir)).toBe(dir);
  });
});
