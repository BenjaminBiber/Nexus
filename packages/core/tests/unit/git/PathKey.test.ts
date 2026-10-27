import { describe, it, expect } from 'vitest';
import { realpathSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { PathKey } from '../../../src/git/PathKey.js';

describe('PathKey', () => {
  it('setzt Kurznamen, Rückstriche und Endstrich mit der kanonischen Schreibweise gleich', () => {
    const keys = new PathKey();
    // os.tmpdir() enthält unter Windows oft einen 8.3-Kurznamen, realpathSync.native den langen Namen
    expect(keys.same(tmpdir(), realpathSync.native(tmpdir()))).toBe(true);
    expect(keys.same(`${tmpdir()}/`, tmpdir().replace(/\\/g, '/'))).toBe(true);
  });

  it('ignoriert Groß-/Kleinschreibung nur unter Windows', () => {
    expect(new PathKey('win32').same('/nicht-da/Ordner', '/NICHT-DA/ordner')).toBe(true);
    expect(new PathKey('linux').same('/nicht-da/Ordner', '/NICHT-DA/ordner')).toBe(false);
  });

  it('löst nicht vorhandene Pfade nur auf', () => {
    const missing = join(tmpdir(), 'gibt-es-nicht', '..', 'auch-nicht');
    expect(new PathKey('linux').of(missing)).toBe(join(tmpdir(), 'auch-nicht').replace(/\\/g, '/'));
  });
});
