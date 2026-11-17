// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import type { ApiClient } from '../src/api/ApiClient.js';
import { ApiProvider } from '../src/api/ApiContext.js';
import type { DbCommentDto } from '../src/api/dbTypes.js';
import { DbTicketWork } from '../src/components/db/DbTicketWork.js';
import { dbCard, dbDetail } from './helpers/fakeApi.js';

const comment = (id: number, body: string): DbCommentDto => ({ id, ticketId: 'ARC-0001', authorId: 'u', authorName: 'dispatcher', body, createdAt: '2026-10-28T10:00:00Z' });
const api = { getAccountNames: async () => ({ accounts: [{ id: 'max', name: 'Arbeit-Max' }, { id: 'pro-1', name: 'Pro 1' }] }) } as unknown as ApiClient;
const show = (ticket: Parameters<typeof DbTicketWork>[0]['ticket']) =>
  render(<ApiProvider api={api}><MemoryRouter><DbTicketWork ticket={ticket} /></MemoryRouter></ApiProvider>);
const START = '[dispatch] Start: an junior-ARC-0001 übergeben – Branch ticket/x, Profil schwer (Modell opus, Effort max, max. 150 Schritte, 120 min). Grund: Label profil:schwer.';

describe('DbTicketWork', () => {
  afterEach(cleanup);

  it('zeigt unter Umsetzung den Namen des Accounts des letzten Laufs, ID und Begründung als Hinweis', async () => {
    const ticket = dbDetail(dbCard('ARC-0001', 'Schwer'), {
      branch: 'ticket/x',
      comments: [
        comment(1, `${START}\nAccount: pro-1 (alle Accounts → pro-1).`),
        comment(2, '[dispatch] Pause: Nutzungslimit von Account pro-1 erreicht'),
        comment(3, `${START}\nAccount: max (Profil schwer → max, meiste freie Kapazität: max (20 %), pro-2 (?)).`),
      ],
    });
    show(ticket);
    const value = await screen.findByText('Arbeit-Max');
    expect(value.previousElementSibling?.textContent).toBe('Account');
    expect(value.getAttribute('title')).toBe('max – Profil schwer → max, meiste freie Kapazität: max (20 %), pro-2 (?)');
  });

  it('zeigt hinter dem Account ein (i) mit geschätzter Limit-Belastung und Dauer', async () => {
    const ticket = dbDetail(dbCard('ARC-0001', 'Schwer'), {
      branch: 'ticket/x',
      comments: [comment(1, `${START}\nAccount: max (alle Accounts → max; Reserve 5 h 4 %, Woche 1 % (aus 8 Läufen Profil schwer); Dauer etwa 95 min).`)],
    });
    show(ticket);
    const name = await screen.findByText('Arbeit-Max');
    expect(name.closest('dd')?.previousElementSibling?.textContent).toBe('Account');
    expect(screen.getByRole('button', { name: 'Erklärung: Geschätzter Verbrauch des Laufs' })).toBeTruthy();
    expect(screen.getByText('5-Stunden-Limit: etwa +4 %')).toBeTruthy();
    expect(screen.getByText('Wochenlimit: etwa +1 %')).toBeTruthy();
    expect(screen.getByText('Dauer: etwa 1 h 35 min')).toBeTruthy();
    expect(screen.getByText('Grundlage: aus 8 Läufen Profil schwer (75. Perzentil).')).toBeTruthy();
  });

  it('mit Puffer und ohne Dauer: Dauer unbekannt, Hinweis auf den Puffer', async () => {
    const ticket = dbDetail(dbCard('ARC-0001', 'Neu'), {
      branch: 'ticket/x',
      comments: [comment(1, `${START}\nAccount: max (alle Accounts → max; Reserve 5 h 10 %, Woche 2 % (Puffer)).`)],
    });
    show(ticket);
    await screen.findByText('Arbeit-Max');
    expect(screen.getByText('Dauer: noch unbekannt')).toBeTruthy();
    expect(screen.getByText(/zu wenige vergleichbare Läufe/)).toBeTruthy();
  });

  it('ohne Schätzung in der Begründung kein (i)', async () => {
    const ticket = dbDetail(dbCard('ARC-0001', 'Alt'), { branch: 'ticket/x', comments: [comment(1, `${START}\nAccount: max (alle Accounts → max).`)] });
    show(ticket);
    await screen.findByText('Arbeit-Max');
    expect(screen.queryByRole('button', { name: /Erklärung/ })).toBeNull();
  });

  it('ohne Account im letzten Start (älterer Lauf, ein Account) keine Zeile', () => {
    const ticket = dbDetail(dbCard('ARC-0001', 'Alt'), { branch: 'ticket/x', comments: [comment(1, START)] });
    show(ticket);
    expect(screen.queryByText('Account')).toBeNull();
  });

  it('unbekannter (entfernter) Account: die ID bleibt stehen', async () => {
    const ticket = dbDetail(dbCard('ARC-0001', 'Weg'), { branch: 'ticket/x', comments: [comment(1, `${START}\nAccount: weg (alle Accounts → weg).`)] });
    show(ticket);
    expect((await screen.findByText('weg')).previousElementSibling?.textContent).toBe('Account');
  });
});
