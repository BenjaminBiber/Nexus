import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { SuggestionStateStore } from '../../../../src/dispatch/optimization/SuggestionStateStore.js';
import { HarnessHome } from '../../../../src/registry/HarnessHome.js';

describe('SuggestionStateStore', () => {
  let dir: string;
  let home: HarnessHome;
  beforeEach(() => {
    dir = mkdtempSync(join(tmpdir(), 'harness-sugg-'));
    home = new HarnessHome({ HARNESS_HOME: dir });
  });
  afterEach(() => rmSync(dir, { recursive: true, force: true }));

  it('liefert ohne Datei einen leeren Zustand und speichert Änderungen', async () => {
    const store = new SuggestionStateStore(home);
    expect(home.dispatchSuggestionsFile).toBe(join(dir, 'dispatch-suggestions.json'));
    expect(await store.read()).toEqual({ dismissed: [], analysis: null });
    await store.update((s) => ({ ...s, dismissed: ['abcdef0123456789'] }));
    const analysis = { at: '2026-10-22T10:00:00.000Z', configHash: 'h', costUsd: 0.04, suggestions: [] };
    await store.update((s) => ({ ...s, analysis }));
    expect(await new SuggestionStateStore(home).read()).toEqual({ dismissed: ['abcdef0123456789'], analysis });
  });

  it('behandelt eine kaputte Datei als leer und warnt', async () => {
    writeFileSync(home.dispatchSuggestionsFile, '{kaputt');
    const warn = vi.fn();
    expect(await new SuggestionStateStore(home, warn).read()).toEqual({ dismissed: [], analysis: null });
    expect(warn).toHaveBeenCalledWith(expect.stringContaining('dispatch-suggestions.json'));
  });
});
