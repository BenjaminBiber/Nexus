// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ApiProvider } from '../src/api/ApiContext.js';
import type { DbTicketDetailDto } from '../src/api/dbTypes.js';
import type { DispatchRunDto } from '../src/api/dispatchTypes.js';
import { DbPlanStart } from '../src/components/db/DbPlanStart.js';
import { dbCard, dbDetail, dispatchStatus, fakeApi } from './helpers/fakeApi.js';

afterEach(cleanup);

const TICKET = dbDetail(dbCard('ARC-0007', 'Rabatt', 'backlog', { needsPlanning: true }));
const RUN: DispatchRunDto = {
  id: 'ARC-0007-20261019-100000-plan', kind: 'plan', ticketId: 'ARC-0007', title: 'Rabatt', profile: 'standard', model: 'opus', effort: 'high',
  startedAt: '2026-10-19T10:00:00.000Z', log: 'ARC-0007-20261019-100000-plan.log',
};

// Rendert den Knopf mit gemockter API
function show(ticket: DbTicketDetailDto = TICKET, mine = true, overrides: Parameters<typeof fakeApi>[0] = {}) {
  const onDone = vi.fn();
  const startTicketPlanning = vi.fn(async () => ({ ...dispatchStatus(), started: RUN }));
  render(<ApiProvider api={fakeApi({ startTicketPlanning, ...overrides })}><DbPlanStart ticket={ticket} mine={mine} onDone={onDone} /></ApiProvider>);
  return { onDone, startTicketPlanning };
}

describe('DbPlanStart', () => {
  it('startet die Planung auf eigenem Board und zeigt den laufenden Lauf', async () => {
    const { onDone, startTicketPlanning } = show();
    expect(screen.getByText('nexus board plan-start ARC-0007')).toBeTruthy();
    await userEvent.click(screen.getByRole('button', { name: 'Planen' }));
    expect(startTicketPlanning).toHaveBeenCalledWith('ARC-0007', 1);
    expect(await screen.findByRole('status')).toHaveProperty('textContent', expect.stringContaining('ARC-0007-20261019-100000-plan.log'));
    expect((screen.getByRole('button', { name: 'Planen' }) as HTMLButtonElement).disabled).toBe(true);
    expect(onDone).toHaveBeenCalled();
  });

  it('fehlt auf fremden Boards und in Status ohne Planung', () => {
    show(TICKET, false);
    expect(screen.queryByRole('button', { name: 'Planen' })).toBeNull();
    cleanup();
    show(dbDetail(dbCard('ARC-0008', 'Läuft', 'in_progress')));
    expect(screen.queryByRole('button', { name: 'Planen' })).toBeNull();
  });

  it('ist gesperrt mit Hinweis bei offener Frage und bei Pause; Fehler erscheinen als Hinweis', async () => {
    show({ ...TICKET, waitingOnQuestions: [{ id: 'ARC-F0001', title: 'Wie?' }] } as DbTicketDetailDto);
    expect((screen.getByRole('button', { name: 'Planen' }) as HTMLButtonElement).disabled).toBe(true);
    expect(screen.getByText(/Wartet auf Frage ARC-F0001/)).toBeTruthy();
    cleanup();
    show(TICKET, true, { getTicketPlanning: async () => ({ ticketId: 'ARC-0007', run: null, pausedUntil: '2026-10-19T11:30:00.000Z' }) });
    await waitFor(() => expect(screen.getByText(/pausiert bis/)).toBeTruthy());
    expect((screen.getByRole('button', { name: 'Planen' }) as HTMLButtonElement).disabled).toBe(true);
    cleanup();
    show(TICKET, true, { startTicketPlanning: async () => { throw new Error('Für ARC-0007 läuft schon eine Planung.'); } });
    await userEvent.click(screen.getByRole('button', { name: 'Planen' }));
    expect((await screen.findByRole('alert')).textContent).toBe('Für ARC-0007 läuft schon eine Planung.');
  });
});
