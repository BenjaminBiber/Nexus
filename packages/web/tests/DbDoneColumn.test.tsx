// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { DbCardDto } from '../src/api/dbTypes.js';
import { DbBoardPage } from '../src/pages/DbBoardPage.js';
import { FakeEventSource } from './helpers/FakeEventSource.js';
import { dbCard, dbDetail, dbView, dispatchStatus, fakeApi } from './helpers/fakeApi.js';
import { renderWithApp } from './helpers/renderWithApp.js';

beforeEach(() => FakeEventSource.reset());
afterEach(cleanup);

const DAY = 24 * 60 * 60 * 1000;
// Zeitpunkt vor so vielen Millisekunden, bezogen auf die echte Uhr (die Seite rechnet mit „jetzt“)
const ago = (ms: number) => new Date(Date.now() - ms).toISOString();

const CARDS = [
  dbCard('ARC-0001', 'Login bauen', 'in_progress', { hasAcceptance: true }),
  dbCard('ARC-0002', 'Heute fertig', 'done', { doneAt: ago(1000) }),
  dbCard('ARC-0003', 'Vor drei Tagen', 'done', { doneAt: ago(3 * DAY) }),
  dbCard('ARC-0004', 'Vor vier Tagen', 'done', { doneAt: ago(4 * DAY) }),
  dbCard('ARC-0005', 'Lange her', 'done', { doneAt: ago(30 * DAY) }),
  dbCard('ARC-0006', 'Wartet auf den Merge', 'done', { doneAt: ago(20 * DAY), branch: 'ticket/ARC-0006-merge' }),
];

const SETTINGS = { notifications: 'off', heroImages: false, theme: 'system', claudeUsage: false, compactCards: false, hideNotNeeded: false, hideInPlanning: false, doneGrouping: 'time', doneArchiveDays: 0 };

// Rendert das Board unter /p/p1/board mit austauschbaren Aufrufen und Karten
function open(overrides: Parameters<typeof fakeApi>[0] = {}, cards: DbCardDto[] = CARDS) {
  const api = fakeApi({ getDbBoardView: vi.fn(async () => dbView(cards)), ...overrides });
  renderWithApp(<DbBoardPage />, { api, path: '/p/p1/board', route: '/p/:pid/board/*' });
  return api;
}

const doneColumn = () => screen.getByRole('region', { name: 'Erledigt' });
// IDs der sichtbaren Zeilen in ihrer Reihenfolge
const rowIds = () => Array.from(doneColumn().querySelectorAll('.db-done-row')).map((row) => row.getAttribute('data-id'));
// Abschnittsköpfe als [Beschriftung, aufgeklappt]
const heads = () => Array.from(doneColumn().querySelectorAll('.db-done-section-head')).map((head) => [head.getAttribute('aria-label'), head.getAttribute('aria-expanded')]);
const count = () => doneColumn().querySelector('.count')?.textContent;
const search = () => screen.getByRole('searchbox', { name: 'Suche im Board' });

describe('Spalte „Erledigt“', () => {
  it('zeigt Zeilen in Abschnitten: „Wartet auf Merge“ zuerst, „Älter“ eingeklappt', async () => {
    open();
    await screen.findByText('Heute fertig');
    expect(heads()).toEqual([
      ['Wartet auf Merge (1)', 'true'],
      ['Heute (1)', 'true'],
      ['Letzte 7 Tage (2)', 'true'],
      ['Älter (1)', 'false'],
    ]);
    expect(rowIds()).toEqual(['ARC-0006', 'ARC-0002', 'ARC-0003', 'ARC-0004']);
    // Der Zähler nennt alle fünf erledigten Tickets; Zeilen haben kein Statusmenü, andere Spalten zeigen weiter Karten
    expect(count()).toBe('5');
    expect(within(doneColumn()).queryByRole('combobox')).toBeNull();
    expect(within(screen.getByRole('region', { name: 'In Arbeit' })).getByRole('combobox', { name: 'Status von ARC-0001 ändern' })).toBeTruthy();
  });

  it('zeigt je Zeile ID, Titel und Erledigt-Datum', async () => {
    open();
    await screen.findByText('Heute fertig');
    const today = within(doneColumn()).getByRole('button', { name: /^ARC-0002/ });
    expect(within(today).getByText('Heute fertig').getAttribute('title')).toBe('Heute fertig');
    expect(today.querySelector('time')?.getAttribute('datetime')).toBe(CARDS[1].doneAt);
    expect(today.querySelector('time')?.textContent).toMatch(/^\d{2}:\d{2}$/);
    const earlier = within(doneColumn()).getByRole('button', { name: /^ARC-0003/ });
    expect(earlier.querySelector('time')?.textContent).toMatch(/^\d{2}\.\d{2}\.(\d{2})?$/);
  });

  it('klappt einen Abschnitt über seinen Kopf auf und zu', async () => {
    const user = userEvent.setup();
    open();
    await screen.findByText('Heute fertig');
    await user.click(within(doneColumn()).getByRole('button', { name: 'Älter (1)' }));
    expect(rowIds()).toEqual(['ARC-0006', 'ARC-0002', 'ARC-0003', 'ARC-0004', 'ARC-0005']);
    await user.click(within(doneColumn()).getByRole('button', { name: 'Heute (1)' }));
    expect(rowIds()).toEqual(['ARC-0006', 'ARC-0003', 'ARC-0004', 'ARC-0005']);
    expect(heads()).toContainEqual(['Heute (1)', 'false']);
  });

  it('zeigt die Pille „Merge“, solange der Dispatcher das Ticket mergt', async () => {
    open({ getDispatchStatus: async () => dispatchStatus({ merges: [{ ticketId: 'ARC-0006', phase: 'session', branch: 'ticket/ARC-0006-merge' }] }) });
    await screen.findByText('Heute fertig');
    const pill = await within(doneColumn()).findByText('Merge');
    expect(pill.getAttribute('title')).toBe('Merge-Session läuft');
    expect(pill.closest('.db-done-row')?.getAttribute('data-id')).toBe('ARC-0006');
  });

  it('öffnet das Ticket per Klick auf die Zeile', async () => {
    const user = userEvent.setup();
    open({ getDbTicket: vi.fn(async () => dbDetail(CARDS[2])) });
    await screen.findByText('Heute fertig');
    await user.click(within(doneColumn()).getByRole('button', { name: /^ARC-0003/ }));
    expect(await screen.findByRole('dialog', { name: 'Ticket ARC-0003' })).toBeTruthy();
  });

  it('lässt sich aus der Spalte ziehen', async () => {
    const moveDbTicket = vi.fn(async () => dbDetail({ ...CARDS[1], status: 'in_progress', column: 'in_progress' }));
    open({ moveDbTicket });
    await screen.findByText('Heute fertig');
    const row = doneColumn().querySelector('[data-id="ARC-0002"]') as HTMLElement;
    expect(row.getAttribute('draggable')).toBe('true');
    const setData = vi.fn();
    fireEvent.dragStart(row, { dataTransfer: { setData } });
    expect(setData).toHaveBeenCalledWith('text/plain', 'ARC-0002');
    fireEvent.drop(screen.getByRole('region', { name: 'In Arbeit' }), { dataTransfer: { getData: () => 'ARC-0002' } });
    await waitFor(() => expect(moveDbTicket).toHaveBeenCalledWith('ARC-0002', { status: 'in_progress', rev: 1 }));
  });

  it('stellt eine hineingezogene Karte sofort unter „Heute“ nach oben', async () => {
    // Die Antwort bleibt offen: Es zählt die sofortige Anzeige
    const moveDbTicket = vi.fn(() => new Promise<never>(() => {}));
    open({ moveDbTicket });
    await screen.findByText('Heute fertig');
    fireEvent.drop(doneColumn(), { dataTransfer: { getData: () => 'ARC-0001' } });
    await waitFor(() => expect(heads()).toContainEqual(['Heute (2)', 'true']));
    expect(rowIds().slice(0, 3)).toEqual(['ARC-0006', 'ARC-0001', 'ARC-0002']);
    expect(count()).toBe('6');
  });

  it('gruppiert nach der Einstellung „spec“ je Spec, „Ohne Spec“ zuletzt', async () => {
    const cards = [
      dbCard('ARC-0002', 'Heute fertig', 'done', { doneAt: ago(1000), spec: 'theme-store' }),
      dbCard('ARC-0003', 'Vor drei Tagen', 'done', { doneAt: ago(3 * DAY) }),
      dbCard('ARC-0005', 'Lange her', 'done', { doneAt: ago(30 * DAY), spec: 'theme-store' }),
      dbCard('ARC-0007', 'Alte Spec', 'done', { doneAt: ago(40 * DAY), spec: 'login' }),
    ];
    open({ getUserSettings: async () => ({ ...SETTINGS, doneGrouping: 'spec' }) }, cards);
    await screen.findByText('Heute fertig');
    await waitFor(() =>
      expect(heads()).toEqual([
        ['theme-store (2)', 'true'],
        ['login (1)', 'false'],
        ['Ohne Spec (1)', 'true'],
      ]),
    );
    expect(rowIds()).toEqual(['ARC-0002', 'ARC-0005', 'ARC-0003']);
  });

  it('blendet archivierte Tickets aus und findet sie über die Suche', async () => {
    const user = userEvent.setup();
    open({ getUserSettings: async () => ({ ...SETTINGS, doneArchiveDays: 14 }) });
    await screen.findByText('Heute fertig');
    expect(await within(doneColumn()).findByText('1 im Archiv')).toBeTruthy();
    expect(count()).toBe('4');
    // Das Ticket mit Branch (20 Tage alt) bleibt, das gemergte von vor 30 Tagen fehlt
    expect(heads()).toEqual([
      ['Wartet auf Merge (1)', 'true'],
      ['Heute (1)', 'true'],
      ['Letzte 7 Tage (2)', 'true'],
    ]);
    await user.type(search(), 'Lange');
    expect(rowIds()).toEqual(['ARC-0005']);
    expect(within(doneColumn()).queryByText('1 im Archiv')).toBeNull();
    expect(count()).toBe('1');
  });

  it('zeigt bei aktiver Suche alle Abschnitte offen und schaltet sie nicht um', async () => {
    const user = userEvent.setup();
    open();
    await screen.findByText('Heute fertig');
    await user.type(search(), 'ARC-000');
    expect(heads()).toEqual([
      ['Wartet auf Merge (1)', 'true'],
      ['Heute (1)', 'true'],
      ['Letzte 7 Tage (2)', 'true'],
      ['Älter (1)', 'true'],
    ]);
    expect(rowIds()).toEqual(['ARC-0006', 'ARC-0002', 'ARC-0003', 'ARC-0004', 'ARC-0005']);
    await user.click(within(doneColumn()).getByRole('button', { name: 'Älter (1)' }));
    expect(rowIds()).toContain('ARC-0005');
  });

  it('zeigt ohne erledigte Tickets den Hinweis „Keine Tickets“', async () => {
    open({}, CARDS.slice(0, 1));
    await screen.findByText('Login bauen');
    expect(within(doneColumn()).getByText('Keine Tickets')).toBeTruthy();
    expect(count()).toBe('0');
    expect(heads()).toEqual([]);
  });
});
