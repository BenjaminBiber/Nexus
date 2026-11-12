// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cleanup, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { DbBoardPage } from '../src/pages/DbBoardPage.js';
import { FakeEventSource } from './helpers/FakeEventSource.js';
import { dbCard, dbDetail, dbView, fakeApi } from './helpers/fakeApi.js';
import { renderWithApp } from './helpers/renderWithApp.js';

beforeEach(() => FakeEventSource.reset());
afterEach(cleanup);

const DAY = 24 * 60 * 60 * 1000;
// Zeitpunkt vor so vielen Millisekunden, bezogen auf die echte Uhr (die Seite rechnet mit „jetzt“)
const ago = (ms: number) => new Date(Date.now() - ms).toISOString();

const CARDS = [
  dbCard('ARC-0001', 'Login bauen', 'in_progress', { hasAcceptance: true }),
  dbCard('ARC-0002', 'Heute fertig', 'done', { doneAt: ago(1000) }),
  dbCard('ARC-0003', 'Vor drei Tagen', 'done', { doneAt: ago(3 * DAY), spec: 'theme-store' }),
  dbCard('ARC-0004', 'Vor vier Tagen', 'done', { doneAt: ago(4 * DAY) }),
  dbCard('ARC-0005', 'Lange her', 'done', { doneAt: ago(30 * DAY), spec: 'theme-store' }),
  dbCard('ARC-0006', 'Wartet auf den Merge', 'done', { doneAt: ago(20 * DAY), branch: 'ticket/ARC-0006-merge' }),
];

const SETTINGS = { notifications: 'off', heroImages: false, theme: 'system', claudeUsage: false, compactCards: false, hideNotNeeded: false, hideInPlanning: false, doneGrouping: 'time', doneArchiveDays: 0 };
const MENU = 'Optionen der Spalte Erledigt';
const TIME_HEADS = ['Wartet auf Merge (1)', 'Heute (1)', 'Letzte 7 Tage (2)', 'Älter (1)'];

// Rendert das Board mit austauschbaren Aufrufen und wartet, bis die Spalte „Erledigt“ ihre Zeilen zeigt
async function open(overrides: Parameters<typeof fakeApi>[0] = {}, path = '/p/p1/board') {
  const api = fakeApi({ getDbBoardView: vi.fn(async () => dbView(CARDS)), ...overrides });
  renderWithApp(<DbBoardPage />, { api, path, route: '/p/:pid/board/*' });
  await screen.findByText('Heute fertig');
  return userEvent.setup();
}

const doneColumn = () => screen.getByRole('region', { name: 'Erledigt' });
const trigger = () => screen.getByRole('button', { name: MENU });
// Beschriftungen der Abschnittsköpfe in ihrer Reihenfolge
const headLabels = () => Array.from(doneColumn().querySelectorAll('.db-done-section-head')).map((head) => head.getAttribute('aria-label'));
const count = () => doneColumn().querySelector('.count')?.textContent;

describe('Menü der Spalte „Erledigt“', () => {
  it('steht nur am Kopf der Spalte „Erledigt“ und ist anfangs zu', async () => {
    await open();
    const buttons = screen.getAllByRole('button', { name: MENU });
    expect(buttons).toHaveLength(1);
    expect(doneColumn().querySelector('.column-head')?.contains(buttons[0])).toBe(true);
    expect(buttons[0].getAttribute('aria-expanded')).toBe('false');
    expect(screen.queryByRole('menu')).toBeNull();
  });

  it('öffnet die Auswahl der Gruppierung, „Zeit“ ist angehakt', async () => {
    const user = await open();
    await user.click(trigger());
    const menu = screen.getByRole('menu', { name: MENU });
    expect(trigger().getAttribute('aria-expanded')).toBe('true');
    expect(within(menu).getByText('Gruppieren nach')).toBeTruthy();
    expect(within(menu).getByRole('menuitemradio', { name: 'Zeit' }).getAttribute('aria-checked')).toBe('true');
    expect(within(menu).getByRole('menuitemradio', { name: 'Spec' }).getAttribute('aria-checked')).toBe('false');
    // Ohne archivierte Tickets gibt es keinen Eintrag für das Archiv
    expect(within(menu).queryByRole('menuitemcheckbox')).toBeNull();
  });

  it('schließt mit Escape und gibt den Fokus an den Knopf zurück', async () => {
    const user = await open();
    await user.click(trigger());
    await user.keyboard('{Escape}');
    expect(screen.queryByRole('menu')).toBeNull();
    expect(document.activeElement).toBe(trigger());
  });

  it('schließt bei einem Klick außerhalb', async () => {
    const user = await open();
    await user.click(trigger());
    await user.click(screen.getByRole('heading', { level: 2, name: 'Backlog' }));
    expect(screen.queryByRole('menu')).toBeNull();
  });

  it('gruppiert nach Spec und speichert die Wahl', async () => {
    const saveUserSettings = vi.fn(async (patch: Record<string, unknown>) => ({ ...SETTINGS, ...patch }));
    const user = await open({ getUserSettings: async () => SETTINGS, saveUserSettings });
    await user.click(trigger());
    await user.click(screen.getByRole('menuitemradio', { name: 'Spec' }));
    expect(screen.queryByRole('menu')).toBeNull();
    expect(headLabels()).toEqual(['Wartet auf Merge (1)', 'theme-store (2)', 'Ohne Spec (2)']);
    expect(saveUserSettings).toHaveBeenCalledTimes(1);
    expect(saveUserSettings).toHaveBeenCalledWith({ doneGrouping: 'spec' });
    await user.click(trigger());
    expect(screen.getByRole('menuitemradio', { name: 'Spec' }).getAttribute('aria-checked')).toBe('true');
    expect(screen.getByRole('menuitemradio', { name: 'Zeit' }).getAttribute('aria-checked')).toBe('false');
  });

  it('speichert nicht, wenn die gewählte Gruppierung schon gilt', async () => {
    const saveUserSettings = vi.fn(async (patch: Record<string, unknown>) => ({ ...SETTINGS, ...patch }));
    const user = await open({ getUserSettings: async () => SETTINGS, saveUserSettings });
    await user.click(trigger());
    await user.click(screen.getByRole('menuitemradio', { name: 'Zeit' }));
    expect(screen.queryByRole('menu')).toBeNull();
    expect(saveUserSettings).not.toHaveBeenCalled();
    expect(headLabels()).toEqual(TIME_HEADS);
  });

  it('nimmt die Wahl zurück und meldet den Fehler, wenn das Speichern scheitert', async () => {
    const saveUserSettings = vi.fn(async () => {
      throw new Error('Einstellungen sind nicht erreichbar.');
    });
    const user = await open({ getUserSettings: async () => SETTINGS, saveUserSettings });
    await user.click(trigger());
    await user.click(screen.getByRole('menuitemradio', { name: 'Spec' }));
    expect((await screen.findByRole('alert')).textContent).toBe('Einstellungen sind nicht erreichbar.');
    await waitFor(() => expect(headLabels()).toEqual(TIME_HEADS));
  });

  it('blendet archivierte Tickets über das Menü ein und wieder aus', async () => {
    const user = await open({ getUserSettings: async () => ({ ...SETTINGS, doneArchiveDays: 14 }) });
    await within(doneColumn()).findByText('1 im Archiv');
    expect(count()).toBe('4');
    await user.click(trigger());
    await user.click(screen.getByRole('menuitemcheckbox', { name: 'Archiv anzeigen (1)' }));
    expect(within(doneColumn()).queryByText('1 im Archiv')).toBeNull();
    expect(headLabels()).toEqual(TIME_HEADS);
    expect(count()).toBe('5');
    await user.click(trigger());
    const shown = screen.getByRole('menuitemcheckbox', { name: 'Archiv anzeigen' });
    expect(shown.getAttribute('aria-checked')).toBe('true');
    await user.click(shown);
    expect(within(doneColumn()).getByText('1 im Archiv')).toBeTruthy();
    expect(count()).toBe('4');
  });

  it('lässt ein offenes Ticket-Panel offen', async () => {
    const user = await open({ getDbTicket: vi.fn(async () => dbDetail(CARDS[0])) }, '/p/p1/board/t/ARC-0001');
    await screen.findByRole('dialog', { name: 'Ticket ARC-0001' });
    await user.click(trigger());
    await user.click(screen.getByText('Gruppieren nach'));
    expect(screen.getByRole('menu', { name: MENU })).toBeTruthy();
    expect(screen.getByRole('dialog', { name: 'Ticket ARC-0001' })).toBeTruthy();
  });
});
