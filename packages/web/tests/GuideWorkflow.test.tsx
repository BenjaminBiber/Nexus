// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, screen, within } from '@testing-library/react';
import { GuidePage } from '../src/pages/GuidePage.js';
import { WORKFLOW_PHASES } from '../../core/src/workflow/WorkflowPhases.js';
import { fakeApi } from './helpers/fakeApi.js';
import { renderWithApp } from './helpers/renderWithApp.js';
afterEach(cleanup);
describe('Anleitung mit angewendetem Workflow', () => {
  it('zeigt den Ablauf, den der Server liefert', async () => {
    const phases = WORKFLOW_PHASES.map((p) => (p.id === 'umsetzen' ? { ...p, steps: [...p.steps, { step: 'Review', command: '/ai-harness:mein-review', when: 'Vor dem Abschluss.' }] } : p));
    renderWithApp(<GuidePage />, { api: fakeApi({ getWorkflow: async () => ({ source: 'custom', phases }) }), path: '/p/p1/guide', route: '/p/:pid/guide' });
    const list = screen.getByRole('list', { name: 'Ticket umsetzen' });
    expect(await within(list).findByText('/ai-harness:mein-review')).toBeTruthy();
  });

  it('zeigt im Standard den Superpowers-Ablauf und behält den Abschnitt Dispatcher auch bei eigenem Workflow', async () => {
    const plan = WORKFLOW_PHASES.find((p) => p.id === 'planen')!;
    expect(plan.steps.map((s) => s.step)).toEqual(['Anforderungen klären', 'Plan schreiben', 'Tickets anlegen', 'Für den Dispatcher vorbereiten']);
    expect(JSON.stringify(WORKFLOW_PHASES)).not.toMatch(/\/speckit-|harness-tasks/);
    const phases = [
      { id: 'planen', title: 'Eigene Planung', hint: 'h', steps: [{ step: 'Klären', command: '/ai-harness:harness-grill', when: 'x' }] },
      { id: 'umsetzen', title: 'Eigene Umsetzung', hint: 'h', steps: [{ step: 'Bauen', command: '/ai-harness:harness-implement', when: 'x' }] },
    ];
    renderWithApp(<GuidePage />, { api: fakeApi({ getWorkflow: async () => ({ source: 'custom', phases }) }), path: '/p/p1/guide', route: '/p/:pid/guide' });
    expect(await screen.findByRole('list', { name: 'Eigene Planung' })).toBeTruthy();
    expect(screen.queryByText('/superpowers:writing-plans')).toBeNull();
    expect(screen.getByRole('heading', { name: 'Dispatcher', level: 2 })).toBeTruthy();
  });
});
