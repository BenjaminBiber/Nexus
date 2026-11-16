// @vitest-environment jsdom
import { describe, it, expect, afterEach, vi } from 'vitest';
import { cleanup, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ApiProvider } from '../src/api/ApiContext.js';
import type { NotificationMode, UserSettingsDto } from '../src/api/types.js';
import { NotificationPanel } from '../src/components/NotificationPanel.js';
import type { Platform } from '../src/platform/Platform.js';
import { PlatformProvider } from '../src/platform/PlatformContext.js';
import { fakeApi } from './helpers/fakeApi.js';
import { fakePlatform } from './helpers/fakePlatform.js';

afterEach(cleanup);

function open(notifications: NotificationMode, platform: Platform) {
  let current: UserSettingsDto = { notifications, heroImages: false, theme: 'system', claudeUsage: false, compactCards: false, hideNotNeeded: false, hideInPlanning: false, doneGrouping: 'time', doneArchiveDays: 0, ntfy: false, ntfyServer: 'https://ntfy.sh', ntfyTopic: '', ntfyToken: '', setupPrompts: { pluginDismissed: false, migrationDismissed: [] } };
  const saveUserSettings = vi.fn(async (patch: Partial<UserSettingsDto>) => (current = { ...current, ...patch }));
  render(
    <ApiProvider api={fakeApi({ getUserSettings: async () => current, saveUserSettings })}>
      <PlatformProvider value={platform}>
        <NotificationPanel />
      </PlatformProvider>
    </ApiProvider>,
  );
  return { saveUserSettings };
}

const app = () => fakePlatform({ notificationPermission: vi.fn(async () => 'granted' as const) });

describe('Benachrichtigungen in der Desktop-App', () => {
  // In der App zeigt immer das System die Meldung; eine Wahl zwischen Browser und Windows gibt es dort nicht
  it('bietet statt der Art nur einen Schalter an', async () => {
    open('windows', app());
    const toggle = (await screen.findByRole('checkbox', { name: 'Meldungen zeigen' })) as HTMLInputElement;
    await waitFor(() => expect(toggle.checked).toBe(true));
    expect(screen.queryByText('Art der Meldung')).toBeNull();
    expect(screen.queryByRole('radio')).toBeNull();
    expect(screen.getByRole('button', { name: 'Testmeldung senden' })).toBeTruthy();
  });

  it('schaltet die Meldungen aus und wieder ein', async () => {
    const { saveUserSettings } = open('windows', app());
    const toggle = (await screen.findByRole('checkbox', { name: 'Meldungen zeigen' })) as HTMLInputElement;
    await waitFor(() => expect(toggle.checked).toBe(true));
    await userEvent.click(toggle);
    await waitFor(() => expect(saveUserSettings).toHaveBeenLastCalledWith({ notifications: 'off' }));
    await waitFor(() => expect(toggle.checked).toBe(false));
    expect(screen.queryByRole('button', { name: 'Testmeldung senden' })).toBeNull();
    await userEvent.click(toggle);
    await waitFor(() => expect(saveUserSettings).toHaveBeenLastCalledWith({ notifications: 'windows' }));
  });

  // „browser“ aus dem Browser zeigt die App nicht (siehe SessionNotifier), der Schalter steht also auf aus
  it('zeigt den Modus „browser“ als ausgeschaltet', async () => {
    open('browser', app());
    const toggle = (await screen.findByRole('checkbox', { name: 'Meldungen zeigen' })) as HTMLInputElement;
    // Erst mit geladenen Einstellungen ist der Schalter bedienbar
    await waitFor(() => expect(toggle.disabled).toBe(false));
    expect(toggle.checked).toBe(false);
  });
});

describe('Benachrichtigungen im Browser', () => {
  it('bietet weiter die Art der Meldung an', async () => {
    open('windows', fakePlatform({ kind: 'browser' }));
    expect(await screen.findByText('Art der Meldung')).toBeTruthy();
    expect(screen.getAllByRole('radio')).toHaveLength(3);
    expect(await screen.findByText('Windows-Meldung')).toBeTruthy();
    expect(screen.queryByRole('checkbox', { name: 'Meldungen zeigen' })).toBeNull();
  });
});
