import { existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterAll, beforeAll, describe, expect, it } from 'vitest';
import { buildPlugin, PLUGIN_OUT, ROOT } from './helpers/pluginBuild.js';

const WORKFLOW = [
  "import { WORKFLOW_PHASES, type WorkflowPhase } from '@harness/core';",
  'export default WORKFLOW_PHASES.map((p) =>',
  "  p.id === 'umsetzen' ? { ...p, steps: [...p.steps, { step: 'Review', command: '/superpowers:requesting-code-review', when: 'Vor dem Abschluss.' }] } : p,",
  ') satisfies WorkflowPhase[];',
].join('\n');

const read = (rel: string) => JSON.parse(readFileSync(join(PLUGIN_OUT, rel), 'utf8'));

describe('Anpassungen im Plugin-Build', () => {
  let custom: string;

  beforeAll(() => {
    custom = mkdtempSync(join(tmpdir(), 'harness-custom-'));
    mkdirSync(custom, { recursive: true });
    writeFileSync(join(custom, 'workflow.ts'), WORKFLOW);
    mkdirSync(join(custom, 'skills', 'harness-grill'), { recursive: true });
    writeFileSync(join(custom, 'skills', 'harness-grill', 'SKILL.md'), '---\nname: harness-grill\ndescription: Eigene Fassung\n---\nEigene Fassung');
    mkdirSync(join(custom, 'skills', 'mein-review'), { recursive: true });
    writeFileSync(join(custom, 'skills', 'mein-review', 'SKILL.md'), '---\ndescription: Review vor dem Abschluss\n---\nPrüfe.');
    mkdirSync(join(custom, 'output-styles'), { recursive: true });
    writeFileSync(join(custom, 'output-styles', 'software-engineer.md'), '---\nname: software-engineer\ndescription: Eigen\n---\nEigener Stil');
  });

  afterAll(() => rmSync(custom, { recursive: true, force: true }));

  it('übernimmt einen gültigen eigenen Workflow', () => {
    const built = buildPlugin(custom);
    expect(built.ok, built.message).toBe(true);
    const overview = read('customizations/overview.json');
    expect(overview).toMatchObject({ version: 1, workflow: 'custom', cloneDir: ROOT, customDir: custom });
    const steps = read('customizations/workflow.json')[1].steps.map((s: { step: string }) => s.step);
    expect(steps).toContain('Review');
    expect(readFileSync(join(PLUGIN_OUT, 'skills/harness-grill/SKILL.md'), 'utf8')).toContain('Eigene Fassung');
    expect(existsSync(join(PLUGIN_OUT, 'skills/mein-review/SKILL.md'))).toBe(true);
    expect(readFileSync(join(PLUGIN_OUT, 'output-styles/software-engineer.md'), 'utf8')).toContain('Eigener Stil');
    const names = readdirSync(join(PLUGIN_OUT, 'skills'));
    expect(new Set(names).size).toBe(names.length);
    expect(overview.entries.map((e: { name: string; kind: string }) => `${e.name}:${e.kind}`)).toEqual(['workflow:replace', 'harness-grill:replace', 'mein-review:new', 'software-engineer:replace']);
  }, 120000);

  it('fällt bei einem unerwarteten Fehler der Typprüfung nur für den Workflow zurück', () => {
    // Echter Fehler statt Mock: ohne Temp-Ordner scheitert die Typprüfung des Workflows beim Anlegen ihres Arbeitsordners
    const missing = join(tmpdir(), 'harness-no-tmp-dir');
    const built = buildPlugin(custom, { TEMP: missing, TMP: missing, TMPDIR: missing });
    expect(built.ok, built.message).toBe(true);
    const overview = read('customizations/overview.json');
    expect(overview).toMatchObject({ version: 1, workflow: 'fallback', cloneDir: ROOT, customDir: custom });
    // Der Workflow fällt zurück (check-failed), die übrigen Anpassungen bleiben aktiv
    expect(overview.entries.map((e: { name: string; state: string }) => `${e.name}:${e.state}`)).toEqual(['workflow:fallback', 'harness-grill:active', 'mein-review:active', 'software-engineer:active']);
    expect(overview.entries[0]).toMatchObject({ point: 'workflow', problems: [{ code: 'check-failed', file: expect.stringMatching(/workflow\.ts$/) }] });
    expect(existsSync(join(PLUGIN_OUT, 'customizations/workflow.json'))).toBe(false);
    expect(readFileSync(join(PLUGIN_OUT, 'skills/harness-grill/SKILL.md'), 'utf8')).toContain('Eigene Fassung');
  }, 120000);

  it('bricht bei einem unerwarteten Fehler beim Anwenden nicht ab und fällt auf den Standard zurück', () => {
    // Echter Fehler statt Mock: custom/skills als Datei statt Ordner lässt das Auflisten der Skills scheitern
    const broken = mkdtempSync(join(tmpdir(), 'harness-custom-broken-'));
    try {
      writeFileSync(join(broken, 'skills'), 'kein Ordner');
      const built = buildPlugin(broken);
      expect(built.ok, built.message).toBe(true);
      expect(built.message).toContain('Es gilt der Standard.');
      const overview = read('customizations/overview.json');
      expect(overview).toMatchObject({ version: 1, workflow: 'fallback', cloneDir: ROOT, customDir: broken });
      expect(overview.entries).toHaveLength(1);
      expect(overview.entries[0]).toMatchObject({ point: 'workflow', state: 'fallback', problems: [{ code: 'apply-failed' }] });
      expect(existsSync(join(PLUGIN_OUT, 'customizations/workflow.json'))).toBe(false);
    } finally {
      rmSync(broken, { recursive: true, force: true });
    }
  }, 120000);

  it('baut ohne Anpassungen mit leerer Übersicht', () => {
    const built = buildPlugin();
    expect(built.ok, built.message).toBe(true);
    expect(read('customizations/overview.json')).toMatchObject({ workflow: 'standard', entries: [] });
    expect(existsSync(join(PLUGIN_OUT, 'customizations/workflow.json'))).toBe(false);
  }, 120000);
});
