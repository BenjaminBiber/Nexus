// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { DbCard } from '../src/components/db/DbCard.js';
import { DbTicketSteps } from '../src/components/db/DbTicketSteps.js';
import type { DbTicketDetailDto } from '../src/api/dbTypes.js';
import { openPanel, openStepsTab } from './helpers/dbPanel.js';
import { dbCard, dbDetail } from './helpers/fakeApi.js';

vi.mock('mermaid', () => ({ default: { initialize: vi.fn(), render: vi.fn(async () => ({ svg: '<svg></svg>' })) } }));

afterEach(cleanup);

const STEPS = [{ text: 'Test schreiben', done: true }, { text: 'Umsetzen', done: false, detail: 'Erwartet: **grün**' }];
const WITH = dbDetail(dbCard('ARC-0001', 'Login bauen', 'in_progress', {
  stepsDone: 1, stepsTotal: 2, spec: '001-login', specTask: 'T001', branch: 'ticket/ARC-0001-login-bauen',
  worktree: 'C:/w/ARC-0001', touches: ['src/login'], consumes: ['GET /a'], produces: ['<Login>'], sessions: ['aaaaaaaa-1111-2222'],
}), { steps: STEPS });

describe('Schritte und Umsetzung im Ticket-Panel', () => {
  it('zeigt Schritte mit Fortschritt, Haken und aufklappbarer Anleitung', async () => {
    openPanel({}, WITH);
    await openStepsTab();
    const section = await screen.findByRole('region', { name: 'Schritte' });
    expect(within(section).getByText('1 von 2 erledigt')).toBeTruthy();
    expect((within(section).getByRole('checkbox', { name: 'Schritt 1: Test schreiben' }) as HTMLInputElement).checked).toBe(true);
    expect((within(section).getByRole('checkbox', { name: 'Schritt 2: Umsetzen' }) as HTMLInputElement).checked).toBe(false);
    expect(within(section).getByText('Anleitung')).toBeTruthy();
  });

  it('hakt einen Schritt ab und übernimmt den neuen Stand', async () => {
    const setDbStep = vi.fn(async (_id: string, n: number, done: boolean) => ({
      ...WITH, rev: WITH.rev + 1, stepsDone: 2, steps: STEPS.map((s, i) => (i === n - 1 ? { ...s, done } : s)),
    }));
    const { onChanged } = openPanel({ setDbStep }, WITH);
    await openStepsTab();
    await userEvent.click(await screen.findByRole('checkbox', { name: 'Schritt 2: Umsetzen' }));
    expect(setDbStep).toHaveBeenCalledWith('ARC-0001', 2, true, 'Umsetzen');
    expect(await screen.findByText('2 von 2 erledigt')).toBeTruthy();
    expect(onChanged).toHaveBeenCalled();
  });

  it('zeigt Spec, Branch, Worktree, Bereiche, Schnittstellen und Sessions', async () => {
    openPanel({}, WITH);
    await openStepsTab();
    const work = await screen.findByRole('region', { name: 'Umsetzung' });
    for (const text of ['001-login / T001', 'ticket/ARC-0001-login-bauen', 'C:/w/ARC-0001', 'src/login', 'GET /a', '<Login>', 'aaaaaaaa']) {
      expect(within(work).getByText(text), text).toBeTruthy();
    }
  });

  it('zeigt den Zielbranch vor dem Branch und lässt ihn ohne Zielbranch weg', async () => {
    openPanel({}, { ...WITH, targetBranch: 'release/1' });
    const work = await screen.findByRole('region', { name: 'Umsetzung' });
    const labels = within(work).getAllByRole('term').map((t) => t.textContent);
    expect(labels.indexOf('Zielbranch')).toBeGreaterThanOrEqual(0);
    expect(labels.indexOf('Zielbranch')).toBe(labels.indexOf('Branch') - 1);
    expect(within(work).getByText('release/1').className).toContain('mono');
    cleanup();
    openPanel({}, WITH);
    const plain = await screen.findByRole('region', { name: 'Umsetzung' });
    expect(within(plain).queryByText('Zielbranch')).toBeNull();
  });

  it('zeigt Schritte auch ohne Einträge (mit „+ Schritt“) und blendet Umsetzung ohne Angaben aus', async () => {
    openPanel();
    await openStepsTab();
    await screen.findByDisplayValue('Login bauen');
    const section = screen.getByRole('region', { name: 'Schritte' });
    expect(within(section).getByText('Noch keine Schritte.')).toBeTruthy();
    expect(within(section).getByRole('button', { name: '+ Schritt' })).toBeTruthy();
    expect(screen.queryByRole('region', { name: 'Umsetzung' })).toBeNull();
  });
});

describe('DbCard mit Schritten und Sessions', () => {
  it('zeigt Fortschritt und Zahl der Sessions', () => {
    render(<DbCard card={dbCard('ARC-0007', 'Export', 'in_progress', { stepsDone: 1, stepsTotal: 3, sessions: ['s1', 's2'] })} onOpen={vi.fn()} onMove={vi.fn()} />);
    expect(screen.getByText('1/3 Schritte')).toBeTruthy();
    expect(screen.getByText('2 Sessions').getAttribute('title')).toBe('s1\ns2');
  });

  it('zeigt kompakt nur die Schrittzahl ohne Balken neben dem Tag-Umschalter', () => {
    render(<DbCard card={dbCard('ARC-0007', 'Export', 'in_progress', { stepsDone: 2, stepsTotal: 5, labels: ['ui'] })} onOpen={vi.fn()} onMove={vi.fn()} compact />);
    expect(screen.queryByRole('progressbar')).toBeNull();
    const foot = screen.getByRole('button', { name: 'Tags (1)' }).parentElement!;
    expect(within(foot).getByText('2/5 Schritte')).toBeTruthy();
  });

  it('zeigt ohne Schritte und Sessions keine Kennzeichen dafür', () => {
    render(<DbCard card={dbCard('ARC-0007', 'Export')} onOpen={vi.fn()} onMove={vi.fn()} />);
    expect(screen.queryByText(/Schritte$/)).toBeNull();
    expect(screen.queryByText(/Session/)).toBeNull();
  });
});

describe('DbTicketSteps ohne Schrittliste', () => {
  it('stürzt nicht ab, wenn das Ticket keine Schritte mitliefert, und bietet „+ Schritt“ an', () => {
    const { steps: _steps, ...without } = dbDetail(dbCard('ARC-0008', 'Alt'));
    render(<DbTicketSteps ticket={without as DbTicketDetailDto} busy={false} onToggle={vi.fn()} onAdd={vi.fn()} onEdit={vi.fn()} onRemove={vi.fn()} onMove={vi.fn()} />);
    expect(screen.getByText('Noch keine Schritte.')).toBeTruthy();
    expect(screen.getByRole('button', { name: '+ Schritt' })).toBeTruthy();
  });
});
