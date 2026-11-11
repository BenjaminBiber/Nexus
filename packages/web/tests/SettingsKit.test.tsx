// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ApiProvider } from '../src/api/ApiContext.js';
import type { UserSettingsDto } from '../src/api/types.js';
import { InterfacePanel } from '../src/components/settings/InterfacePanel.js';
import { InfoTip } from '../src/components/settings/InfoTip.js';
import { SettingRow } from '../src/components/settings/SettingRow.js';
import { SettingsPage } from '../src/pages/SettingsPage.js';
import { fakeApi, projects } from './helpers/fakeApi.js';
import { renderWithApp } from './helpers/renderWithApp.js';

afterEach(cleanup);

const SETTINGS: UserSettingsDto = { notifications: 'off', heroImages: false, theme: 'system', claudeUsage: false, compactCards: false, hideNotNeeded: false, hideInPlanning: false, doneGrouping: 'time', doneArchiveDays: 0, ntfy: false, ntfyServer: 'https://ntfy.sh', ntfyTopic: '', ntfyToken: '', setupPrompts: { pluginDismissed: false, migrationDismissed: [] } };

describe('Bausteine der Einstellungen', () => {
  it('verbindet Beschriftung und Hinweis einer Zeile mit dem Eingabefeld', () => {
    render(<SettingRow label="Versuche" hint="Danach geht das Ticket nach Blockiert." info="Mehr dazu"><input /></SettingRow>);
    const input = screen.getByLabelText('Versuche');
    const hint = screen.getByText('Danach geht das Ticket nach Blockiert.');
    expect(input.getAttribute('aria-describedby')).toBe(hint.id);
    // Der (i)-Knopf gehört nicht zur Beschriftung des Felds
    expect(screen.getByRole('button', { name: 'Erklärung: Versuche' })).toBeTruthy();
  });

  it('öffnet die Erklärung über den (i)-Knopf als Popover', () => {
    render(<InfoTip label="WIP">So viele Tickets gleichzeitig.</InfoTip>);
    const button = screen.getByRole('button', { name: 'Erklärung: WIP' });
    const pop = screen.getByText('So viele Tickets gleichzeitig.');
    expect(pop.getAttribute('popover')).toBe('auto');
    expect(button.getAttribute('popovertarget')).toBe(pop.id);
  });

  it('meldet den Speicherstand eines sofort speichernden Bereichs', async () => {
    const saveUserSettings = vi.fn(async (patch: Partial<UserSettingsDto>) => ({ ...SETTINGS, ...patch }));
    render(<ApiProvider api={fakeApi({ getUserSettings: async () => SETTINGS, saveUserSettings })}><InterfacePanel /></ApiProvider>);
    expect(await screen.findByText('Änderungen werden sofort gespeichert')).toBeTruthy();
    await userEvent.click(await screen.findByRole('checkbox', { name: 'Kompakte Ticket-Kacheln' }));
    expect(await screen.findByText('Gespeichert')).toBeTruthy();
  });

  it('zeigt einen Speicherfehler als Meldung', async () => {
    const saveUserSettings = vi.fn(async () => { throw new Error('Datei schreibgeschützt'); });
    render(<ApiProvider api={fakeApi({ getUserSettings: async () => SETTINGS, saveUserSettings })}><InterfacePanel /></ApiProvider>);
    await userEvent.click(await screen.findByRole('checkbox', { name: 'Kompakte Ticket-Kacheln' }));
    expect((await screen.findByRole('alert')).textContent).toBe('Datei schreibgeschützt');
  });

  it('hat beim Dispatcher einen eigenen Unterpunkt für Planungen', async () => {
    renderWithApp(<SettingsPage />, { api: fakeApi({ getProjects: async () => projects, getSettings: async () => ({ project: projects[0], token: 'x' }) }), path: '/p/p1/settings#dispatch/planning', route: '/p/:pid/settings' });
    const nav = await screen.findByRole('navigation', { name: 'Einstellungsbereiche' });
    expect(within(nav).getByRole('link', { name: /Planungen/ })).toBeTruthy();
    expect(await screen.findByLabelText('Tickets mit Planungsbedarf automatisch planen lassen')).toBeTruthy();
    expect(screen.queryByLabelText('Neue Fragen zu meinen Boards automatisch recherchieren lassen')).toBeNull();
  });
});

describe('Archiv der erledigten Tickets', () => {
  // Rendert die Oberflächen-Einstellungen mit dem gespeicherten Stand und liefert die Auswahl, sobald sie bedienbar ist
  async function openArchive(settings: UserSettingsDto) {
    const saveUserSettings = vi.fn(async (patch: Partial<UserSettingsDto>) => ({ ...settings, ...patch }));
    render(<ApiProvider api={fakeApi({ getUserSettings: async () => settings, saveUserSettings })}><InterfacePanel /></ApiProvider>);
    const select = (await screen.findByLabelText('Erledigte Tickets archivieren')) as HTMLSelectElement;
    await waitFor(() => expect(select.disabled).toBe(false));
    return { select, saveUserSettings };
  }

  const labels = (select: HTMLSelectElement) => Array.from(select.options).map((option) => option.textContent);

  it('bietet Nie, 7, 14 und 30 Tage an und zeigt den gespeicherten Stand', async () => {
    const { select } = await openArchive(SETTINGS);
    expect(labels(select)).toEqual(['Nie', 'nach 7 Tagen', 'nach 14 Tagen', 'nach 30 Tagen']);
    expect(select.value).toBe('0');
    expect(screen.getByText('Gemergte Tickets verschwinden danach vom Board. Die Suche findet sie weiter.')).toBeTruthy();
  });

  it('speichert die gewählte Dauer', async () => {
    const { select, saveUserSettings } = await openArchive(SETTINGS);
    await userEvent.selectOptions(select, 'nach 14 Tagen');
    expect(saveUserSettings).toHaveBeenCalledTimes(1);
    expect(saveUserSettings).toHaveBeenCalledWith({ doneArchiveDays: 14 });
    await waitFor(() => expect(select.value).toBe('14'));
    expect(await screen.findByText('Gespeichert')).toBeTruthy();
  });

  it('zeigt eine gespeicherte Dauer außerhalb der Auswahl als eigene Option', async () => {
    const { select } = await openArchive({ ...SETTINGS, doneArchiveDays: 21 });
    expect(labels(select)).toEqual(['Nie', 'nach 7 Tagen', 'nach 14 Tagen', 'nach 21 Tagen', 'nach 30 Tagen']);
    expect(select.value).toBe('21');
  });

  it('schreibt einen einzelnen Tag in der Einzahl', async () => {
    const { select } = await openArchive({ ...SETTINGS, doneArchiveDays: 1 });
    expect(labels(select)).toEqual(['Nie', 'nach 1 Tag', 'nach 7 Tagen', 'nach 14 Tagen', 'nach 30 Tagen']);
    expect(select.value).toBe('1');
  });
});
