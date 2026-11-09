// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { ApiProvider } from '../src/api/ApiContext.js';
import { AppearancePanel } from '../src/components/AppearancePanel.js';
import { ThemeInstallDialog } from '../src/components/settings/ThemeInstallDialog.js';
import { fakeApi } from './helpers/fakeApi.js';
import { testManifest, themeFile } from './helpers/zip.js';
afterEach(cleanup);

const settings = { notifications: 'off', heroImages: false, theme: 'system', claudeUsage: false, compactCards: false, hideNotNeeded: false, hideInPlanning: false };
function open(installTheme = vi.fn(async () => testManifest())) {
  render(<ApiProvider api={fakeApi({ getUserSettings: async () => settings, saveUserSettings: async () => settings, listThemes: async () => [], installTheme } as never)}><AppearancePanel /></ApiProvider>);
  return installTheme;
}
async function choose(file: File) {
  await screen.findByRole('list', { name: 'Installierte Themes' }).catch(() => undefined);
  fireEvent.change(await screen.findByLabelText('Theme hochladen'), { target: { files: [file] } });
}

describe('ThemeInstallDialog', () => {
  it('zeigt Name, Autor, Version und hebt Code hervor', () => {
    render(<ThemeInstallDialog manifest={testManifest()} hasCode busy={false} onConfirm={() => {}} onCancel={() => {}} />);
    const dialog = screen.getByRole('alertdialog', { name: 'Theme installieren?' });
    expect(within(dialog).getByText('Ocean')).toBeTruthy();
    expect(within(dialog).getByText('von Ada')).toBeTruthy();
    expect(within(dialog).getByText('Version 1.2.0')).toBeTruthy();
    expect(within(dialog).getByText('Dieses Theme führt Code aus').closest('.warn')).toBeTruthy();
  });

  it('zeigt ohne main.js keinen Code-Hinweis und „Keine fremden Domains“', () => {
    render(<ThemeInstallDialog manifest={testManifest()} hasCode={false} busy={false} onConfirm={() => {}} onCancel={() => {}} />);
    expect(screen.queryByText('Dieses Theme führt Code aus')).toBeNull();
    expect(screen.getByText('Keine fremden Domains')).toBeTruthy();
  });

  it('listet die angefragten Domains', () => {
    render(<ThemeInstallDialog manifest={testManifest({ permissions: { network: ['fonts.example.com', '*.cdn.example'] } })} hasCode={false} busy={false} onConfirm={() => {}} onCancel={() => {}} />);
    const list = screen.getByRole('list', { name: 'Angefragte Domains' });
    expect(within(list).getAllByRole('listitem').map((li) => li.textContent)).toEqual(['fonts.example.com', '*.cdn.example']);
    expect(screen.queryByText('Keine fremden Domains')).toBeNull();
  });
});

describe('Installieren mit Rückfrage', () => {
  it('Abbrechen installiert nichts und schließt den Dialog', async () => {
    const installTheme = open();
    await choose(await themeFile(testManifest(), true));
    const dialog = await screen.findByRole('alertdialog', { name: 'Theme installieren?' });
    expect(within(dialog).getByText('Dieses Theme führt Code aus')).toBeTruthy();
    fireEvent.click(within(dialog).getByRole('button', { name: 'Abbrechen' }));
    expect(screen.queryByRole('alertdialog')).toBeNull();
    expect(installTheme).not.toHaveBeenCalled();
  });

  it('Installieren ruft installTheme genau einmal mit der Datei auf', async () => {
    const installTheme = open();
    const file = await themeFile(testManifest());
    await choose(file);
    fireEvent.click(await screen.findByRole('button', { name: 'Installieren' }));
    await waitFor(() => expect(installTheme).toHaveBeenCalledTimes(1));
    expect(installTheme).toHaveBeenCalledWith(file);
    await waitFor(() => expect(screen.queryByRole('alertdialog')).toBeNull());
  });

  it('meldet ein unlesbares Paket und installiert nichts', async () => {
    const installTheme = open();
    await choose(new File(['kein zip'], 'x.nexus'));
    expect((await screen.findByRole('alert')).textContent).toContain('kein ZIP-Archiv');
    expect(screen.queryByRole('alertdialog')).toBeNull();
    expect(installTheme).not.toHaveBeenCalled();
  });
});
