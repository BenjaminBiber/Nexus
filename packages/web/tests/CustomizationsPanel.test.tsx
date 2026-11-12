// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, screen, within } from '@testing-library/react';
import { CustomizationsPanel } from '../src/components/CustomizationsPanel.js';
import type { CustomizationsDto } from '../src/api/types.js';
import { fakeApi } from './helpers/fakeApi.js';
import { renderWithApp } from './helpers/renderWithApp.js';
afterEach(cleanup);
const open = (data: CustomizationsDto) =>
  renderWithApp(<CustomizationsPanel />, { api: fakeApi({ getCustomizations: async () => data }), path: '/p/p1/settings', route: '/p/:pid/settings' });
const base = { version: 1 as const, createdAt: '2026-10-14T10:00:00.000Z', cloneDir: '/h', customDir: '/h/custom', harnessVersion: '0.2.0' };
describe('CustomizationsPanel', () => {
  it('zeigt Erweiterungspunkte, Warnungen und Rückfälle', async () => {
    open({
      ...base,
      workflow: 'custom',
      entries: [
        { point: 'workflow', name: 'workflow', kind: 'replace', state: 'active', base: 'current', problems: [], warnings: [] },
        { point: 'skill', name: 'harness-implement', kind: 'replace', state: 'active', base: 'outdated', problems: [], warnings: [{ code: 'base-outdated', message: 'Der Standard hat sich seit deiner Kopie geändert.' }] },
        { point: 'skill', name: 'mein-review', kind: 'new', state: 'active', base: 'none', problems: [], warnings: [] },
        { point: 'output-style', name: 'kurz', kind: 'new', state: 'fallback', base: 'none', problems: [{ code: 'description-missing', message: 'Im Kopf fehlt description.' }], warnings: [] },
      ],
    });
    const panel = await screen.findByRole('region', { name: 'Anpassungen' });
    expect(await within(panel).findByText('angepasst')).toBeTruthy();
    expect(within(panel).getByText('harness-implement (ersetzt), mein-review (neu)')).toBeTruthy();
    expect(within(panel).getByText('Standard')).toBeTruthy();
    expect(within(within(panel).getByRole('list', { name: 'Warnungen' })).getByText('Skill harness-implement: Der Standard hat sich seit deiner Kopie geändert.')).toBeTruthy();
    expect(within(within(panel).getByRole('list', { name: 'Rückfälle auf den Standard' })).getByText('Output-Style kurz: Im Kopf fehlt description.')).toBeTruthy();
    // Nur lesend: außer Erklärungen (i) gibt es nur den Kopieren-Knopf
    const actions = within(panel).getAllByRole('button').map((b) => b.getAttribute('aria-label')).filter((name) => !name?.startsWith('Erklärung:'));
    expect(actions).toEqual(['Befehl kopieren: /nexus:harness-customize']);
  });
  it('meldet, wenn es keine Anpassungen gibt', async () => {
    open({ ...base, workflow: 'standard', entries: [] });
    expect(await screen.findByText('Keine Anpassungen, überall gilt der Standard.')).toBeTruthy();
  });
  it('weist auf setup hin, wenn die Übersicht fehlt', async () => {
    open({ state: 'unknown' });
    expect(await screen.findByText('Noch keine Übersicht. Führe im Nexus-Ordner einmal npm run build:plugin und nexus install aus.')).toBeTruthy();
  });
});
