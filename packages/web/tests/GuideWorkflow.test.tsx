// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, screen, within } from '@testing-library/react';
import { GuidePage } from '../src/pages/GuidePage.js';
import type { WorkflowPhase } from '../src/api/generated/workflow.js';
import { TEST_WORKFLOW_PHASES } from './helpers/workflowPhases.js';
import { fakeApi } from './helpers/fakeApi.js';
import { renderWithApp } from './helpers/renderWithApp.js';
afterEach(cleanup);
describe('Anleitung mit angewendetem Workflow', () => {
  it('zeigt den Ablauf, den der Server liefert', async () => {
    const phases = TEST_WORKFLOW_PHASES.map((p) => (p.id === 'umsetzen' ? { ...p, steps: [...p.steps, { step: 'Review', command: '/nexus:mein-review', when: 'Vor dem Abschluss.' }] } : p));
    renderWithApp(<GuidePage />, { api: fakeApi({ getWorkflow: async () => ({ source: 'custom', phases }) }), path: '/p/p1/guide', route: '/p/:pid/guide' });
    const list = await screen.findByRole('list', { name: 'Ticket umsetzen' });
    expect(await within(list).findByText('/nexus:mein-review')).toBeTruthy();
  });

  it('behält den Abschnitt Dispatcher auch bei eigenem Workflow', async () => {
    const phases: WorkflowPhase[] = [
      { id: 'planen', title: 'Eigene Planung', hint: 'h', steps: [{ step: 'Klären', command: '/nexus:harness-grill', when: 'x' }] },
      { id: 'umsetzen', title: 'Eigene Umsetzung', hint: 'h', steps: [{ step: 'Bauen', command: '/nexus:harness-implement', when: 'x' }] },
    ];
    renderWithApp(<GuidePage />, { api: fakeApi({ getWorkflow: async () => ({ source: 'custom', phases }) }), path: '/p/p1/guide', route: '/p/:pid/guide' });
    expect(await screen.findByRole('list', { name: 'Eigene Planung' })).toBeTruthy();
    expect(screen.queryByText('/superpowers:writing-plans')).toBeNull();
    expect(screen.getByRole('heading', { name: 'Dispatcher', level: 2 })).toBeTruthy();
  });
});
