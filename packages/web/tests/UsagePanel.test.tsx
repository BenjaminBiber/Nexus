// @vitest-environment jsdom
import { describe, it, expect, afterEach, vi } from 'vitest';
import { cleanup, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ApiProvider } from '../src/api/ApiContext.js';
import { UsagePanel } from '../src/components/UsagePanel.js';
import { USAGE_SETTING_EVENT } from '../src/components/ClaudeUsageMeter.js';
import type { UserSettingsDto } from '../src/api/types.js';
import { fakeApi } from './helpers/fakeApi.js';

afterEach(cleanup);

function open(initial: boolean) {
  let current: UserSettingsDto = { notifications: 'off', heroImages: false, theme: 'system', claudeUsage: initial, compactCards: false, hideNotNeeded: false, hideInPlanning: false, doneGrouping: 'time', doneArchiveDays: 0, ntfy: false, ntfyServer: 'https://ntfy.sh', ntfyTopic: '', ntfyToken: '', setupPrompts: { pluginDismissed: false, migrationDismissed: [] } };
  const saveUserSettings = vi.fn(async (patch: Partial<UserSettingsDto>) => (current = { ...current, ...patch }));
  render(<ApiProvider api={fakeApi({ getUserSettings: async () => current, saveUserSettings })}><UsagePanel /></ApiProvider>);
  return { saveUserSettings };
}

describe('UsagePanel', () => {
  it('zeigt die Warnung und schaltet erst nach der Bestätigung ein', async () => {
    const { saveUserSettings } = open(false);
    const warning = await screen.findByRole('note', { name: 'Warnung' });
    expect(warning.textContent).toContain('.credentials.json');
    expect(warning.textContent).toContain('nicht dokumentiert');
    const on = screen.getByRole('button', { name: 'Einschalten' }) as HTMLButtonElement;
    expect(on.disabled).toBe(true);

    const heard = vi.fn();
    window.addEventListener(USAGE_SETTING_EVENT, heard);
    await userEvent.click(screen.getByRole('checkbox'));
    expect(on.disabled).toBe(false);
    await userEvent.click(on);
    await waitFor(() => expect(saveUserSettings).toHaveBeenCalledWith({ claudeUsage: true }));
    expect(await screen.findByText(/Eingeschaltet/)).toBeTruthy();
    expect(heard).toHaveBeenCalledTimes(1);
    window.removeEventListener(USAGE_SETTING_EVENT, heard);
  });

  it('schaltet im eingeschalteten Zustand mit einem Klick wieder aus', async () => {
    const { saveUserSettings } = open(true);
    await userEvent.click(await screen.findByRole('button', { name: 'Ausschalten' }));
    await waitFor(() => expect(saveUserSettings).toHaveBeenCalledWith({ claudeUsage: false }));
    expect(await screen.findByRole('button', { name: 'Einschalten' })).toBeTruthy();
    expect((screen.getByRole('checkbox') as HTMLInputElement).checked).toBe(false);
  });
});
