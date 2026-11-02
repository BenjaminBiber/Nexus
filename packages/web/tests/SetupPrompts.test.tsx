// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { useEffect } from 'react';
import { ApiProvider } from '../src/api/ApiContext.js';
import { BoardProjectProvider, useBoardProject } from '../src/dbboard/boardProject.js';
import { SetupPrompts } from '../src/components/setup/SetupPrompts.js';
import { fakeApi } from './helpers/fakeApi.js';

const settings = (setupPrompts = { pluginDismissed: false, migrationDismissed: [] as number[] }) => ({
  notifications: 'off', heroImages: false, theme: 'system', claudeUsage: false, compactCards: false, hideNotNeeded: false,
  hideInPlanning: false, ntfy: false, ntfyServer: 'https://ntfy.sh', ntfyTopic: '', ntfyToken: '', setupPrompts,
});
const missing = { bundled: true, claude: true, state: 'missing', installedVersion: null, bundledVersion: '0.1.2' };
const legacy = { needed: true, repoPath: 'C:/repo', reasons: ['legacy-rules'] };

// Setzt das angezeigte Board wie die Board-Seite
function ShowBoard({ board }: { board: number }) {
  const setShown = useBoardProject()?.setShown;
  useEffect(() => setShown?.({ project: 'ARC', board }), [setShown, board]);
  return null;
}

function open(overrides: Parameters<typeof fakeApi>[0], board = 1) {
  const api = fakeApi(overrides);
  const view = render(
    <ApiProvider api={api}>
      <BoardProjectProvider>
        <ShowBoard board={board} />
        <SetupPrompts />
      </BoardProjectProvider>
    </ApiProvider>,
  );
  return { api, view };
}

beforeEach(() => sessionStorage.clear());
afterEach(() => {
  cleanup();
  delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
});

const inApp = () => { (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {}; };

describe('SetupPrompts', () => {
  it('zeigt den Plugin-Dialog nur in der App', async () => {
    const getPluginSetup = vi.fn(async () => missing);
    open({ getPluginSetup });
    await waitFor(() => expect(getPluginSetup).toHaveBeenCalled());
    expect(screen.queryByText(/Claude-Code-Plugin installieren/)).toBeNull();
  });

  it('zeigt den Plugin-Dialog in der App bei fehlendem Plugin', async () => {
    inApp();
    open({ getPluginSetup: async () => missing });
    expect(await screen.findByText(/Claude-Code-Plugin installieren/)).toBeTruthy();
  });

  it('zeigt ihn nicht bei installiertem Plugin', async () => {
    inApp();
    const getBoardMigration = vi.fn(async () => ({ needed: false, repoPath: null, reasons: [] }));
    open({ getPluginSetup: async () => ({ ...missing, state: 'installed' }), getBoardMigration });
    await waitFor(() => expect(getBoardMigration).toHaveBeenCalled());
    expect(screen.queryByText(/Claude-Code-Plugin/)).toBeNull();
  });

  it('merkt Nicht jetzt für die Sitzung', async () => {
    inApp();
    open({ getPluginSetup: async () => missing });
    await userEvent.click(await screen.findByRole('button', { name: 'Nicht jetzt' }));
    expect(screen.queryByText(/Claude-Code-Plugin installieren/)).toBeNull();
    cleanup();
    open({ getPluginSetup: async () => missing });
    await new Promise((resolve) => setTimeout(resolve, 20));
    expect(screen.queryByText(/Claude-Code-Plugin installieren/)).toBeNull();
  });

  it('speichert Nicht mehr fragen in den Einstellungen', async () => {
    inApp();
    const saveUserSettings = vi.fn(async (value) => ({ ...settings(), ...value }));
    open({ getPluginSetup: async () => missing, getUserSettings: async () => settings(), saveUserSettings });
    await userEvent.click(await screen.findByRole('button', { name: 'Nicht mehr fragen' }));
    expect(saveUserSettings).toHaveBeenCalledWith({ setupPrompts: { pluginDismissed: true, migrationDismissed: [] } });
  });

  it('fragt nicht bei abgewiesenem Plugin', async () => {
    inApp();
    const getBoardMigration = vi.fn(async () => ({ needed: false, repoPath: null, reasons: [] }));
    open({
      getPluginSetup: async () => missing,
      getUserSettings: async () => settings({ pluginDismissed: true, migrationDismissed: [] }),
      getBoardMigration,
    });
    await waitFor(() => expect(getBoardMigration).toHaveBeenCalled());
    expect(screen.queryByText(/Claude-Code-Plugin installieren/)).toBeNull();
  });

  it('zeigt die Migration erst nach dem Plugin-Dialog', async () => {
    inApp();
    const getBoardMigration = vi.fn(async () => legacy);
    open({ getPluginSetup: async () => missing, getBoardMigration });
    await userEvent.click(await screen.findByRole('button', { name: 'Nicht jetzt' }));
    expect(await screen.findByText(/Projekt auf Nexus umstellen/)).toBeTruthy();
    expect(getBoardMigration).toHaveBeenCalledWith(1);
  });

  it('zeigt die Migration im Browser für das angezeigte Board', async () => {
    const getBoardMigration = vi.fn(async () => legacy);
    open({ getBoardMigration }, 2);
    expect(await screen.findByText(/Projekt auf Nexus umstellen/)).toBeTruthy();
    expect(getBoardMigration).toHaveBeenCalledWith(2);
  });

  it('fragt nicht für ein abgewiesenes Board', async () => {
    const getBoardMigration = vi.fn(async () => legacy);
    const getUserSettings = vi.fn(async () => settings({ pluginDismissed: false, migrationDismissed: [1] }));
    open({ getBoardMigration, getUserSettings });
    await waitFor(() => expect(getUserSettings).toHaveBeenCalled());
    await new Promise((resolve) => setTimeout(resolve, 20));
    expect(getBoardMigration).not.toHaveBeenCalled();
    expect(screen.queryByText(/Projekt auf Nexus umstellen/)).toBeNull();
  });

  it('speichert Für dieses Board nicht mehr fragen', async () => {
    const saveUserSettings = vi.fn(async (value) => ({ ...settings(), ...value }));
    open({ getBoardMigration: async () => legacy, getUserSettings: async () => settings({ pluginDismissed: false, migrationDismissed: [5] }), saveUserSettings });
    await userEvent.click(await screen.findByRole('button', { name: 'Für dieses Board nicht mehr fragen' }));
    expect(saveUserSettings).toHaveBeenCalledWith({ setupPrompts: { pluginDismissed: false, migrationDismissed: [5, 1] } });
  });
});
