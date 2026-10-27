import { vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import { ApiProvider } from '../../src/api/ApiContext.js';
import type { DbCardDto, DbTicketDetailDto, DbTicketPatch } from '../../src/api/dbTypes.js';
import { DbTicketPanel, type DbTicketPanelProps } from '../../src/components/db/DbTicketPanel.js';
import { LiveProvider } from '../../src/live/LiveProvider.js';
import { dbCard, dbDetail, dbView, fakeApi } from './fakeApi.js';

export const DETAIL = dbDetail(dbCard('ARC-0001', 'Login bauen', 'backlog', { rev: 3, labels: ['ui'], assignee: 'claude', hasAcceptance: true, needsPlanning: true }), { acceptance: 'Login geht' });
export const COLUMNS = dbView([]).columns.map(({ id, title }) => ({ id, title }));

// Rendert das Ticket-Panel allein (Live-Meldungen über FakeEventSource); patchDbTicket antwortet mit dem geänderten Stand und nächster Version
export function openPanel(overrides: Parameters<typeof fakeApi>[0] = {}, detail: DbTicketDetailDto = DETAIL, boardTickets: DbCardDto[] = [], props: Partial<DbTicketPanelProps> = {}) {
  let current = detail;
  const patchDbTicket = vi.fn(async (_id: string, patch: DbTicketPatch) => (current = { ...current, ...patch, rev: current.rev + 1 }));
  const onClose = vi.fn();
  const onChanged = vi.fn();
  const onOpenTicket = vi.fn();
  render(
    <ApiProvider api={fakeApi({ getDbTicket: async () => detail, patchDbTicket, ...overrides })}>
      <LiveProvider pid="p1">
        <MemoryRouter>
          <DbTicketPanel ticketId={detail.id} boardTickets={boardTickets} columns={COLUMNS} onClose={onClose} onChanged={onChanged} onOpenTicket={onOpenTicket} {...props} />
        </MemoryRouter>
      </LiveProvider>
    </ApiProvider>,
  );
  return { onClose, onChanged, onOpenTicket, patchDbTicket };
}

// Wechselt im geöffneten Panel auf den Reiter „Schritte“ (wartet, bis das Ticket geladen ist)
export async function openStepsTab() {
  await userEvent.click(await screen.findByRole('tab', { name: /^Schritte/ }));
}
