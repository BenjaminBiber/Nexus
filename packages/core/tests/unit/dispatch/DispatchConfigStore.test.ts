import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { HarnessHome } from '../../../src/registry/HarnessHome.js';
import { DispatchConfigStore } from '../../../src/dispatch/DispatchConfigStore.js';

// Knappe eigene Vorlage für work, die alle Pflichtteile erfüllt
const WORK = [
  '{{agent}} setzt {{ticketId}} („{{title}}“) um.',
  'Fertig: harness board done {{ticketId}} --by {{agent}}',
  'Blockiert: harness board set {{ticketId}} --status blocked',
  'Frage: harness board question add "…" --ticket {{ticketId}}',
].join('\n');

describe('DispatchConfigStore', () => {
  let dir: string;
  let store: DispatchConfigStore;

  beforeEach(() => {
    dir = mkdtempSync(join(tmpdir(), 'harness-dispatch-'));
    store = new DispatchConfigStore(new HarnessHome({ HARNESS_HOME: dir }));
  });
  afterEach(() => rmSync(dir, { recursive: true, force: true }));

  it('liefert ohne Datei das Beispiel, ausgeschaltet', async () => {
    const read = await store.read();
    expect(read.exists).toBe(false);
    expect(read.config.enabled).toBe(false);
    expect(Object.keys(read.config.profiles)).toEqual(['klein', 'standard', 'schwer']);
    expect(read.raw.profiles).toBeDefined();
  });

  it('schreibt geprüft und liest wieder', async () => {
    const saved = await store.write({ enabled: true, wip: 2 });
    expect(saved).toMatchObject({ enabled: true, wip: 2 });
    expect(readFileSync(join(dir, 'dispatch.yaml'), 'utf8')).toContain('enabled: true');
    const read = await store.read();
    expect(read.exists).toBe(true);
    expect(read.config.wip).toBe(2);
  });

  it('schreibt Ungültiges nicht', async () => {
    await expect(store.write({ wip: -3 })).rejects.toMatchObject({ code: 'dispatch-config-invalid' });
    expect((await store.read()).exists).toBe(false);
  });

  it('meldet kaputtes YAML', async () => {
    writeFileSync(join(dir, 'dispatch.yaml'), 'wip: [');
    await expect(store.read()).rejects.toMatchObject({ code: 'dispatch-config-invalid' });
  });

  it('speichert eine gültige eigene Vorlage', async () => {
    const saved = await store.write({ prompts: { work: WORK } });
    expect(saved.prompts.work).toBe(WORK);
    expect(readFileSync(join(dir, 'dispatch.yaml'), 'utf8')).toContain('harness board done');
    const read = await store.read();
    expect(read.config.prompts.work).toBe(WORK);
    expect(read.promptProblems).toEqual([]);
  });

  it('lehnt eine Vorlage mit unbekanntem Platzhalter beim Speichern ab', async () => {
    await expect(store.write({ prompts: { work: `${WORK}\n{{unbekannt}}` } }))
      .rejects.toMatchObject({ code: 'dispatch-prompt-invalid', message: expect.stringMatching(/Vorlage „work“.*\{\{unbekannt\}\}/s) });
    expect((await store.read()).exists).toBe(false);
  });

  it('lehnt eine Vorlage mit fehlendem Pflichtteil beim Speichern ab und nennt ihn', async () => {
    await expect(store.write({ prompts: { plan: 'Hallo {{agent}}, plane {{ticketId}}.' } }))
      .rejects.toMatchObject({ code: 'dispatch-prompt-invalid', message: expect.stringMatching(/Vorlage „plan“.*plan\.json/s) });
  });

  it('bricht beim Lesen nicht an einer von Hand verfälschten Vorlage', async () => {
    writeFileSync(join(dir, 'dispatch.yaml'), 'prompts:\n  plan: "kaputt {{agent}}"\n');
    const read = await store.read();
    expect(read.config.prompts.plan).toBeNull();
    expect(read.promptProblems.map((p) => p.kind)).toEqual(['plan']);
    expect(read.promptProblems[0].missingParts.length).toBeGreaterThan(0);
  });
});
