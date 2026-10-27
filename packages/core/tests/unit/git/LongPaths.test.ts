import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { rmSync } from 'node:fs';
import { join } from 'node:path';
import { LongPaths } from '../../../src/git/LongPaths.js';
import { TempRepo } from '../helpers/TempRepo.js';
import { makeTempDir } from '../helpers/TempDir.js';

describe('LongPaths', () => {
  let repo: TempRepo;
  let configHome: string;

  // Eigene, leere globale Git-Konfiguration: sonst entscheidet ein global gesetztes
  // core.longpaths des Rechners über das Ergebnis von ensure (die Datei darf fehlen).
  beforeEach(() => {
    configHome = makeTempDir('harness-gitconfig-');
    vi.stubEnv('GIT_CONFIG_GLOBAL', join(configHome, 'gitconfig'));
  });

  afterEach(() => {
    repo?.dispose();
    rmSync(configHome, { recursive: true, force: true });
    vi.unstubAllEnvs();
  });

  const value = (): string => {
    try {
      return repo.git('config', '--local', '--get', 'core.longpaths');
    } catch {
      return '';
    }
  };

  it('setzt core.longpaths unter Windows einmalig und ist idempotent', () => {
    repo = new TempRepo();
    const longPaths = new LongPaths();
    expect(longPaths.ensure(repo.path, 'win32')).toBe(true);
    expect(value()).toBe('true');
    expect(longPaths.ensure(repo.path, 'win32')).toBe(false);
    expect(value()).toBe('true');
  });

  it('tut unter anderen Plattformen nichts', () => {
    repo = new TempRepo();
    expect(new LongPaths().ensure(repo.path, 'linux')).toBe(false);
    expect(value()).toBe('');
  });
});
