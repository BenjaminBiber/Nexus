// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ApiError } from '../src/api/ApiError.js';
import type { DbTicketDetailDto } from '../src/api/dbTypes.js';
import { DbCard } from '../src/components/db/DbCard.js';
import { openPanel } from './helpers/dbPanel.js';
import { dbCard, dbDetail, dbQuestion, dbQuestionDetail, timelineFrom } from './helpers/fakeApi.js';

vi.mock('mermaid', () => ({ default: { initialize: vi.fn(), render: vi.fn(async () => ({ svg: '<svg></svg>' })) } }));

afterEach(cleanup);

const WAITING = [{ id: 'ARC-F0001', title: 'Welcher Speicher?' }];

// Öffnet den Reiter „Fragen“ des Ticket-Panels und liefert seinen Inhalt
async function openQuestions() {
  await userEvent.click(await screen.findByRole('tab', { name: /^Fragen/ }));
  return screen.getByRole('region', { name: 'Fragen' });
}

describe('DbCard mit offener Frage', () => {
  it('zeigt Kennzeichen und „wartet auf Frage“, das Ticket bleibt ziehbar', () => {
    render(<DbCard card={dbCard('ARC-0007', 'Export', 'planned', { waitingOnQuestions: WAITING })} onOpen={vi.fn()} onMove={vi.fn()} />);
    expect(screen.getByText('Frage').getAttribute('title')).toBe('ARC-F0001 Welcher Speicher?');
    expect(screen.getByText('wartet auf Frage ARC-F0001')).toBeTruthy();
    expect(screen.getByRole('article').getAttribute('draggable')).toBe('true');
  });

  it('zeigt ohne offene Frage kein Kennzeichen', () => {
    render(<DbCard card={dbCard('ARC-0007', 'Export')} onOpen={vi.fn()} onMove={vi.fn()} />);
    expect(screen.queryByText('Frage')).toBeNull();
  });
});

describe('Ticket-Panel: Reiter „Fragen“', () => {
  it('bietet ohne offene Frage nur „Frage stellen“', async () => {
    openPanel();
    const section = await openQuestions();
    expect(within(section).getByText(/Keine offene Frage/)).toBeTruthy();
    expect(within(section).getByRole('button', { name: 'Frage stellen' })).toBeTruthy();
  });

  it('zeigt offene Fragen und stellt eine neue Frage zum Ticket', async () => {
    let current: DbTicketDetailDto = dbDetail(dbCard('ARC-0001', 'Login bauen', 'backlog', { rev: 3, waitingOnQuestions: WAITING }));
    const getDbTicket = vi.fn(async () => current);
    const createDbQuestion = vi.fn(async () => dbQuestionDetail(dbQuestion('ARC-F0002', 'Neue')));
    const { onChanged } = openPanel({ getDbTicket, createDbQuestion }, current);
    const section = await openQuestions();
    expect(within(section).getByRole('link', { name: /ARC-F0001/ }).getAttribute('href')).toContain('/questions/ARC-F0001');
    expect(within(section).getByText(/In Arbeit und Erledigt sind gesperrt/)).toBeTruthy();
    current = { ...current, rev: 4, waitingOnQuestions: [...WAITING, { id: 'ARC-F0002', title: 'Neue' }] };
    await userEvent.click(within(section).getByRole('button', { name: 'Frage stellen' }));
    const dialog = screen.getByRole('dialog', { name: 'Frage zu ARC-0001 stellen' });
    await userEvent.type(within(dialog).getByLabelText('Titel'), 'Neue');
    await userEvent.click(within(dialog).getByRole('button', { name: 'Frage stellen' }));
    expect(createDbQuestion).toHaveBeenCalledWith('ARC', { title: 'Neue', occasion: 'Ticket ARC-0001', tags: [], ticketIds: ['ARC-0001'] });
    expect(await within(screen.getByRole('region', { name: 'Fragen' })).findByRole('link', { name: /ARC-F0002/ })).toBeTruthy();
    expect(screen.getByRole('tab', { name: /Fragen \(2\)/ })).toBeTruthy();
    expect(screen.queryByRole('dialog', { name: 'Frage zu ARC-0001 stellen' })).toBeNull();
    expect(onChanged).toHaveBeenCalled();
  });

  it('beschriftet Verlaufseinträge einer Frage im Ticketverlauf', async () => {
    const detail = dbDetail(dbCard('ARC-0001', 'Login bauen'), {
      history: [{ ticketId: 'ARC-0001', at: '2026-10-14T12:00:00.000Z', byId: 'u1', byName: 'Florian', field: 'question', from: 'open', to: 'answered', note: 'Frage entschieden: ARC-F0001 „Welcher Speicher?“' }],
    });
    openPanel({ getDbTicketTimeline: async () => timelineFrom(detail.history) }, detail);
    await userEvent.click(await screen.findByRole('tab', { name: 'Verlauf' }));
    expect(await screen.findByText(/Frage offen → beantwortet/)).toBeTruthy();
  });

  it('beschriftet auch „verworfen“ und lässt unbekannte Zustände unverändert', async () => {
    const entry = { ticketId: 'ARC-0001', at: '2026-10-14T12:00:00.000Z', byId: 'u1', byName: 'Florian', field: 'question', note: null };
    const detail = dbDetail(dbCard('ARC-0001', 'Login bauen'), {
      history: [{ ...entry, from: 'answered', to: 'open' }, { ...entry, from: 'open', to: 'discarded' }, { ...entry, from: null, to: 'seltsam' }],
    });
    openPanel({ getDbTicketTimeline: async () => timelineFrom(detail.history) }, detail);
    await userEvent.click(await screen.findByRole('tab', { name: 'Verlauf' }));
    expect(await screen.findByText(/Frage beantwortet → offen/)).toBeTruthy();
    expect(screen.getByText(/Frage offen → verworfen/)).toBeTruthy();
    expect(screen.getByText(/Frage – → seltsam/)).toBeTruthy();
  });

  it('nennt bei zwei offenen Fragen beide und spricht in der Mehrzahl', async () => {
    openPanel({}, dbDetail(dbCard('ARC-0001', 'Login bauen', 'backlog', { waitingOnQuestions: [...WAITING, { id: 'ARC-F0002', title: 'Welcher Port?' }] })));
    const section = await openQuestions();
    expect(within(section).getByText('Wartet auf 2 Fragen: In Arbeit und Erledigt sind gesperrt, bis alle entschieden sind.')).toBeTruthy();
    expect(within(section).getAllByRole('link', { name: /ARC-F000/ })).toHaveLength(2);
  });

  it('Esc im Dialog „Frage stellen“ schließt nur den Dialog, nicht das Panel', async () => {
    const { onClose } = openPanel();
    const section = await openQuestions();
    await userEvent.click(within(section).getByRole('button', { name: 'Frage stellen' }));
    const dialog = screen.getByRole('dialog', { name: 'Frage zu ARC-0001 stellen' });
    expect(dialog.contains(document.activeElement)).toBe(true);
    await userEvent.keyboard('{Escape}');
    expect(screen.queryByRole('dialog', { name: 'Frage zu ARC-0001 stellen' })).toBeNull();
    // Auch ein Esc direkt auf dem Dialog erreicht das Panel nicht
    await userEvent.click(within(section).getByRole('button', { name: 'Frage stellen' }));
    fireEvent.keyDown(screen.getByRole('dialog', { name: 'Frage zu ARC-0001 stellen' }), { key: 'Escape' });
    expect(screen.queryByRole('dialog', { name: 'Frage zu ARC-0001 stellen' })).toBeNull();
    expect(onClose).not.toHaveBeenCalled();
    expect(screen.getByRole('region', { name: 'Fragen' })).toBeTruthy();
  });

  it('zeigt einen Fehler des Servers im Dialog und lässt ihn offen', async () => {
    const createDbQuestion = vi.fn(async () => {
      throw new ApiError('question-ticket-other-project', 'Ticket OTH-0001 gehört zu einem anderen Projekt. Eine Frage betrifft nur Tickets ihres Projekts.', 422);
    });
    const { onClose, onChanged } = openPanel({ createDbQuestion });
    const section = await openQuestions();
    await userEvent.click(within(section).getByRole('button', { name: 'Frage stellen' }));
    const dialog = screen.getByRole('dialog', { name: 'Frage zu ARC-0001 stellen' });
    await userEvent.type(within(dialog).getByLabelText('Titel'), 'Neue');
    const tickets = within(dialog).getByLabelText('Betroffene Tickets');
    await userEvent.clear(tickets);
    await userEvent.type(tickets, 'OTH-0001');
    await userEvent.click(within(dialog).getByRole('button', { name: 'Frage stellen' }));
    expect((await within(dialog).findByRole('alert')).textContent).toContain('gehört zu einem anderen Projekt');
    expect(screen.getByRole('dialog', { name: 'Frage zu ARC-0001 stellen' })).toBe(dialog);
    expect((within(dialog).getByLabelText('Titel') as HTMLInputElement).value).toBe('Neue');
    expect(onClose).not.toHaveBeenCalled();
    expect(onChanged).not.toHaveBeenCalled();
  });
});
