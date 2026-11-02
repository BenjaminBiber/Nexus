// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ApiProvider } from '../src/api/ApiContext.js';
import type { UserSettingsDto } from '../src/api/types.js';
import { AppearancePanel } from '../src/components/AppearancePanel.js';
import { InfoTip } from '../src/components/settings/InfoTip.js';
import { SettingRow } from '../src/components/settings/SettingRow.js';
import { SettingsPage } from '../src/pages/SettingsPage.js';
import { fakeApi, projects } from './helpers/fakeApi.js';
import { renderWithApp } from './helpers/renderWithApp.js';

afterEach(cleanup);

const SETTINGS: UserSettingsDto = { notifications: 'off', heroImages: false, theme: 'system', claudeUsage: false, compactCards: false, hideNotNeeded: false, hideInPlanning: false, ntfy: false, ntfyServer: 'https://ntfy.sh', ntfyTopic: '', ntfyToken: '', setupPrompts: { pluginDismissed: false, migrationDismissed: [] } };

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
    render(<ApiProvider api={fakeApi({ getUserSettings: async () => SETTINGS, saveUserSettings })}><AppearancePanel /></ApiProvider>);
    expect(await screen.findByText('Änderungen werden sofort gespeichert')).toBeTruthy();
    await userEvent.click(await screen.findByRole('checkbox', { name: 'Kompakte Ticket-Kacheln' }));
    expect(await screen.findByText('Gespeichert')).toBeTruthy();
  });

  it('zeigt einen Speicherfehler als Meldung', async () => {
    const saveUserSettings = vi.fn(async () => { throw new Error('Datei schreibgeschützt'); });
    render(<ApiProvider api={fakeApi({ getUserSettings: async () => SETTINGS, saveUserSettings })}><AppearancePanel /></ApiProvider>);
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
