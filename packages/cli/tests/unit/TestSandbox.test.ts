import { afterEach, describe, expect, it, vi } from 'vitest';
import { existsSync, statSync } from 'node:fs';
import { homedir, tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { DEFAULT_SERVER_PORT, HarnessHome, HarnessProject } from '@harness/core';
import { HookRunner } from '../../src/hooks/HookRunner.js';
import { TempRepo } from '../../../core/tests/unit/helpers/TempRepo.js';

// Regression: Testläufe haben Testprojekte im echten ~/.harness und beim laufenden Server auf Port 4710 angemeldet.
// vitest.setup.ts gibt jeder Testdatei ein eigenes HARNESS_HOME und einen unbenutzten Port.
describe('Test-Umgebung des Harness', () => {
  const realHome = join(homedir(), '.harness');
  let repo: TempRepo | undefined;
  afterEach(() => {
    vi.restoreAllMocks();
    repo?.dispose();
    repo = undefined;
  });

  it('zeigt HARNESS_HOME und HARNESS_PORT auf eine eigene Umgebung', () => {
    const home = process.env.HARNESS_HOME!;
    expect(resolve(home).startsWith(resolve(tmpdir()))).toBe(true);
    expect(resolve(new HarnessHome().dir)).toBe(resolve(home));
    expect(resolve(new HarnessHome().dir)).not.toBe(resolve(realHome));
    expect(Number(process.env.HARNESS_PORT)).toBeGreaterThan(0);
    expect(Number(process.env.HARNESS_PORT)).not.toBe(DEFAULT_SERVER_PORT);
  });

  it('HookRunner mit Standard-Abhängigkeiten und Standard-Umgebung erreicht weder den echten Server noch ~/.harness', async () => {
    repo = new TempRepo();
    await HarnessProject.init(repo.path);
    const realProjects = join(realHome, 'projects.json');
    const before = existsSync(realProjects) ? statSync(realProjects).mtimeMs : null;
    const urls: string[] = [];
    const fetchSpy = vi.spyOn(globalThis, 'fetch').mockImplementation(async (input) => {
      urls.push(String(input instanceof Request ? input.url : input));
      throw new TypeError('fetch failed', { cause: { code: 'ECONNREFUSED' } });
    });

    // user-prompt: freier Port → keine Meldung und kein Serverstart, aber die Prüfung des Ports findet statt
    await new HookRunner({ boardBudgetMs: 500 }).run('user-prompt', { session_id: 'sandbox', cwd: repo.path, prompt: 'Hallo' }, process.env, () => undefined);

    expect(fetchSpy).toHaveBeenCalled();
    expect(urls.every((url) => new URL(url).port === process.env.HARNESS_PORT)).toBe(true);
    expect(urls.some((url) => url.includes(`:${DEFAULT_SERVER_PORT}`))).toBe(false);
    expect(existsSync(realProjects) ? statSync(realProjects).mtimeMs : null).toBe(before);
  });
});
