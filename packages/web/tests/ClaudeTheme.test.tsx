// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { ApiProvider } from '../src/api/ApiContext.js';
import type { ClaudeThemeStatus, ThemeManifest } from '../src/api/generated/themes.js';
import { claudeThemeHint } from '../src/components/settings/ClaudeThemeSection.js';
import { ThemeInstallDialog } from '../src/components/settings/ThemeInstallDialog.js';
import { ThemesPanel } from '../src/components/settings/ThemesPanel.js';
import { fakeApi } from './helpers/fakeApi.js';
afterEach(cleanup);

const PATH = '/home/ada/.claude/skills/nexus-theme';
const manifest = (id: string, extra: Partial<ThemeManifest> = {}): ThemeManifest => ({
  id, name: `Theme ${id}`, version: '1.0.0', author: 'Ada', sdkVersion: '^1.3', colorScheme: 'dark', startColor: null, preview: null, slots: [], permissions: { network: [] }, ...extra,
});
const status = (extra: Partial<ClaudeThemeStatus> = {}): ClaudeThemeStatus => ({ enabled: true, state: 'none', path: PATH, ...extra });
const settings = (theme: string, claudeTheme?: boolean) => ({
  notifications: 'off', heroImages: false, theme, claudeUsage: false, compactCards: false, hideNotNeeded: false, hideInPlanning: false,
  ...(claudeTheme === undefined ? {} : { claudeTheme }),
});

function open(overrides: Record<string, unknown>, theme = 'system', claudeTheme?: boolean) {
  let current: Record<string, unknown> = settings(theme, claudeTheme);
  const saveUserSettings = vi.fn(async (patch: object) => (current = { ...current, ...patch }));
  render(<ApiProvider api={fakeApi({ getUserSettings: async () => current, saveUserSettings, ...overrides } as never)}><ThemesPanel /></ApiProvider>);
  return { saveUserSettings };
}

describe('claudeThemeHint', () => {
  it('beschreibt jeden Zustand', () => {
    expect(claudeThemeHint(status({ state: 'active', themeId: 'synthwave', themeName: 'Synthwave' }), true)).toBe('In Claude aktiv: Synthwave');
    expect(claudeThemeHint(status(), true)).toBe('Dieses Theme bringt kein Claude-Theme mit.');
    expect(claudeThemeHint(status({ state: 'foreign' }), true)).toBe(`${PATH} gehört nicht Nexus – umbenennen oder löschen.`);
    expect(claudeThemeHint(status({ state: 'error', message: 'Dateifehler im Claude-Theme: Zugriff verweigert' }), true)).toBe('Dateifehler im Claude-Theme: Zugriff verweigert');
    expect(claudeThemeHint(status({ state: 'active', themeName: 'Synthwave' }), false)).toBe('Aus: Claude Desktop behält sein eigenes Aussehen.');
    expect(claudeThemeHint(undefined, true)).toBe('Zustand wird geladen …');
  });
});

describe('Darstellung › Themes › Claude Desktop', () => {
  it('zeigt den Zustand und schaltet die Übernahme aus', async () => {
    const getClaudeTheme = vi
      .fn()
      .mockResolvedValueOnce(status({ state: 'active', themeId: 'neon', themeName: 'Neon' }))
      .mockResolvedValue(status({ enabled: false }));
    const { saveUserSettings } = open({ listThemes: async () => [manifest('neon', { claudeTheme: true })], getClaudeTheme }, 'neon');

    const toggle = await screen.findByRole('switch', { name: 'Claude übernimmt das Theme' });
    expect(await screen.findByText('In Claude aktiv: Neon')).toBeTruthy();
    await waitFor(() => expect(toggle).toHaveProperty('checked', true));

    fireEvent.click(toggle);
    await waitFor(() => expect(saveUserSettings).toHaveBeenCalledWith({ claudeTheme: false }));
    expect(await screen.findByText('Aus: Claude Desktop behält sein eigenes Aussehen.')).toBeTruthy();
    expect(getClaudeTheme).toHaveBeenCalledTimes(2);
    expect(toggle).toHaveProperty('checked', false);
  });

  it('lädt den Zustand nach einem Theme-Wechsel neu', async () => {
    const getClaudeTheme = vi
      .fn()
      .mockResolvedValueOnce(status())
      .mockResolvedValue(status({ state: 'active', themeId: 'neon', themeName: 'Neon' }));
    const { saveUserSettings } = open({ listThemes: async () => [manifest('neon', { claudeTheme: true })], getClaudeTheme });
    expect(await screen.findByText('Dieses Theme bringt kein Claude-Theme mit.')).toBeTruthy();

    fireEvent.click(await screen.findByRole('button', { name: 'Theme neon anwenden' }));

    await waitFor(() => expect(saveUserSettings).toHaveBeenCalledWith({ theme: 'neon' }));
    expect(await screen.findByText('In Claude aktiv: Neon')).toBeTruthy();
  });

  it('meldet einen fremden Ordner als Warnung', async () => {
    open({ getClaudeTheme: async () => status({ state: 'foreign', message: 'x' }) }, 'system', true);
    const hint = await screen.findByText(`${PATH} gehört nicht Nexus – umbenennen oder löschen.`);
    expect(hint.className).toBe('claude-theme-warn');
  });

  it('gilt als an, wenn die Einstellung fehlt', async () => {
    open({});
    const toggle = await screen.findByRole('switch', { name: 'Claude übernimmt das Theme' });
    await waitFor(() => expect(toggle).toHaveProperty('disabled', false));
    expect(toggle).toHaveProperty('checked', true);
  });

  it('markiert Kacheln mit Claude-Theme', async () => {
    open({ listThemes: async () => [manifest('neon', { claudeTheme: true }), manifest('plain')] });
    const list = await screen.findByRole('list', { name: 'Themes' });
    const neon = await within(list).findByRole('listitem', { name: 'Theme neon' });
    expect(within(neon).getByText('Claude')).toBeTruthy();
    const plain = within(list).getByRole('listitem', { name: 'Theme plain' });
    expect(within(plain).queryByText('Claude')).toBeNull();
  });

  it('markiert Store-Einträge mit Claude-Theme und weist im Installationsdialog darauf hin', async () => {
    const entry = { repoId: 'r1', repoUrl: 'https://x.example/i.json', id: 'neon', name: 'Neon', version: '1.0.0', sdkVersion: '^1.3', downloadUrl: 'x', sha256: 'y', compatible: true, claudeTheme: true };
    open({ getStoreIndex: async () => ({ entries: [entry], repos: [{ id: 'r1', url: entry.repoUrl, ok: true, entryCount: 1, skipped: [] }] }) });
    await screen.findByRole('list', { name: 'Themes' });
    fireEvent.click(screen.getByRole('button', { name: 'Theme installieren' }));
    const dialog = await screen.findByRole('dialog', { name: 'Theme installieren' });
    const card = (await within(dialog).findByText('Neon')).closest('li')!;
    expect(within(card).getByText('Claude')).toBeTruthy();

    fireEvent.click(within(card).getByRole('button', { name: 'Installieren' }));
    const confirm = await screen.findByRole('alertdialog', { name: 'Theme installieren?' });
    expect(within(confirm).getByText('Bringt ein Claude-Theme mit')).toBeTruthy();
  });
});

describe('ThemeInstallDialog', () => {
  it('weist nur bei einem Claude-Theme auf den Mod hin', () => {
    const props = { manifest: { name: 'Neon' }, hasCode: false, busy: false, onConfirm: () => {}, onCancel: () => {} };
    const { unmount } = render(<ThemeInstallDialog {...props} claudeTheme />);
    expect(screen.getByText('Bringt ein Claude-Theme mit')).toBeTruthy();
    unmount();
    render(<ThemeInstallDialog {...props} />);
    expect(screen.queryByText('Bringt ein Claude-Theme mit')).toBeNull();
  });
});
