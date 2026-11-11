// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ApiProvider } from '../src/api/ApiContext.js';
import { ApiError } from '../src/api/ApiError.js';
import type { StoreIndexDto, StoreIndexEntryDto, StoreUpdatesDto } from '../src/api/types.js';
import type { ThemeManifest } from '../src/api/generated/themes.js';
import { ThemeStore } from '../src/components/settings/ThemeStore.js';
import { fakeApi } from './helpers/fakeApi.js';
import { testManifest } from './helpers/zip.js';

afterEach(cleanup);

const entry = (extra: Partial<StoreIndexEntryDto> = {}): StoreIndexEntryDto => ({
  repoId: 'r1', repoUrl: 'https://example.com/nexus-themes.json', id: 'ocean', name: 'Ocean', version: '1.0.0', sdkVersion: '^1.0',
  downloadUrl: 'https://example.com/ocean.nexus', sha256: 'x', description: 'Blaues Meer', compatible: true, ...extra,
});
const repoOk = { id: 'r1', url: 'https://example.com/nexus-themes.json', ok: true, entryCount: 2, skipped: [] };

function setup(opts: { index?: StoreIndexDto; updates?: StoreUpdatesDto; installed?: ThemeManifest[]; extra?: Record<string, unknown> } = {}) {
  const index = opts.index ?? { entries: [entry(), entry({ id: 'forest', name: 'Forest', description: 'Grüner Wald', preview: 'https://example.com/f.png' })], repos: [repoOk] };
  const api = {
    getStoreIndex: vi.fn(async () => index),
    getStoreUpdates: vi.fn(async () => opts.updates ?? { updates: [], errors: [] }),
    installFromStore: vi.fn(async () => ({ manifest: testManifest(), verified: true, repoId: 'r1', sha256: 'x' })),
    updateFromStore: vi.fn(async () => ({ manifest: testManifest(), verified: false, repoId: 'r1', sha256: 'x', previousVersion: '1.0.0' })),
    ...opts.extra,
  };
  const onInstalled = vi.fn();
  render(<ApiProvider api={fakeApi(api as never)}><ThemeStore installed={opts.installed ?? []} onInstalled={onInstalled} /></ApiProvider>);
  return { api, onInstalled };
}

describe('Store: Durchsuchen', () => {
  it('zeigt Name, Version, Beschreibung, Vorschau und Store', async () => {
    setup();
    const card = (await screen.findByText('Forest')).closest('li')!;
    expect(within(card).getByText('v1.0.0')).toBeTruthy();
    expect(within(card).getByText('Grüner Wald')).toBeTruthy();
    // Die Vorschau kommt über den Server: die CSP der Oberfläche lässt keine Bilder fremder Hosts zu
    expect(within(card).getByAltText('Vorschau: Forest').getAttribute('src')).toBe('/api/themes/store/preview?repoId=r1&id=forest&v=1.0.0');
    expect(within(card).getByText('example.com')).toBeTruthy();
    const ocean = screen.getByText('Ocean').closest('li')!;
    expect(within(ocean).queryByRole('img')).toBeNull();
    expect(within(ocean).getByText('Keine Vorschau')).toBeTruthy();
    expect(screen.getByText('2 Themes')).toBeTruthy();
  });

  it('filtert nach Name, id und Beschreibung', async () => {
    setup();
    const search = await screen.findByLabelText('Store durchsuchen');
    await userEvent.type(search, 'WALD');
    expect(screen.queryByText('Ocean')).toBeNull();
    expect(screen.getByText('Forest')).toBeTruthy();
    await userEvent.clear(search);
    await userEvent.type(search, 'ocea');
    expect(screen.getByText('Ocean')).toBeTruthy();
    expect(screen.getByText('1 Theme')).toBeTruthy();
    await userEvent.clear(search);
    await userEvent.type(search, 'xyz');
    expect(screen.getByText('Keine Themes gefunden')).toBeTruthy();
    expect(screen.getByText('Kein Theme passt zu „xyz“.')).toBeTruthy();
  });

  it('weist ohne Repos auf die Repo-Verwaltung hin', async () => {
    setup({ index: { entries: [], repos: [] } });
    expect(await screen.findByText(/Noch kein Theme-Store eingetragen/)).toBeTruthy();
  });

  it('markiert nicht kompatible Einträge und zeigt Installiertes', async () => {
    setup({ index: { entries: [entry(), entry({ id: 'old', name: 'Old', compatible: false })], repos: [repoOk] }, installed: [testManifest({ version: '1.0.0' })] });
    const old = (await screen.findByText('Old')).closest('li')!;
    expect(within(old).getByText('Nicht kompatibel')).toBeTruthy();
    expect((within(old).getByRole('button', { name: 'Installieren' }) as HTMLButtonElement).disabled).toBe(true);
    const ocean = screen.getByText('Ocean').closest('li')!;
    expect(within(ocean).getByText('Installiert')).toBeTruthy();
    expect(within(ocean).queryByRole('button', { name: 'Installieren' })).toBeNull();
  });

  it('zeigt die Meldung eines fehlerhaften Repos und die übrigen Einträge', async () => {
    setup({ index: { entries: [entry()], repos: [repoOk, { id: 'r2', url: 'https://bad.example/x.json', ok: false, entryCount: 0, skipped: [], error: { code: 'store-index-http', message: 'Repo antwortet mit 404' } }] } });
    await userEvent.click(await screen.findByRole('button', { name: /Theme-Stores/ }));
    expect(screen.getByText(/Repo antwortet mit 404/)).toBeTruthy();
    expect(screen.getByText('HTTP 404')).toBeTruthy();
    expect(screen.getByText('Ocean')).toBeTruthy();
  });

  it('fasst die Stores zusammen und klappt die Details auf', async () => {
    setup();
    const head = await screen.findByRole('button', { name: /Theme-Stores/ });
    expect(head.textContent).toContain('1 von 1 erreichbar');
    expect(head.getAttribute('aria-expanded')).toBe('false');
    expect(screen.queryByRole('list', { name: 'Theme-Stores' })).toBeNull();
    await userEvent.click(head);
    const list = screen.getByRole('list', { name: 'Theme-Stores' });
    expect(within(list).getByText('Erreichbar')).toBeTruthy();
    expect(within(list).getByText('2 Themes')).toBeTruthy();
  });

  it('startet auch bei einem nicht erreichbaren GitHub-Store eingeklappt, verlinkt die Releases und prüft erneut', async () => {
    const url = 'https://github.com/acme/themes/releases/latest/download/nexus-themes.json';
    const bad = { id: 'r2', url, ok: false, entryCount: 0, skipped: [], error: { code: 'store-index-http', message: 'Das Repo antwortet mit HTTP 404.' } };
    const getStoreIndex = vi.fn(async () => ({ entries: [entry()], repos: [repoOk, bad] }));
    setup({ extra: { getStoreIndex } });
    const head = await screen.findByRole('button', { name: /Theme-Stores/ });
    expect(head.getAttribute('aria-expanded')).toBe('false');
    expect(head.textContent).toContain('1 Store nicht erreichbar');
    await userEvent.click(head);
    const list = screen.getByRole('list', { name: 'Theme-Stores' });
    expect(within(list).getByText('github.com/acme/themes')).toBeTruthy();
    expect(within(list).getByText(/Meist fehlt im neuesten Release die Datei nexus-themes.json/)).toBeTruthy();
    expect(within(list).getByRole('link', { name: /Releases öffnen/ }).getAttribute('href')).toBe('https://github.com/acme/themes/releases');
    await userEvent.click(within(list).getByRole('button', { name: /Erneut prüfen/ }));
    await waitFor(() => expect(getStoreIndex).toHaveBeenCalledTimes(2));
    expect(await within(list).findByText('Zuletzt geprüft: gerade eben')).toBeTruthy();
    expect(screen.getByText('Ocean')).toBeTruthy();
  });
});

describe('Store: Installieren', () => {
  const install = async () => userEvent.click(within((await screen.findByText('Ocean')).closest('li')!).getByRole('button', { name: 'Installieren' }));

  it('Abbrechen installiert nichts', async () => {
    const { api } = setup();
    await install();
    const dialog = screen.getByRole('alertdialog', { name: 'Theme installieren?' });
    expect(within(dialog).getByText(/Themes aus dem Store können Code ausführen/)).toBeTruthy();
    await userEvent.click(within(dialog).getByRole('button', { name: 'Abbrechen' }));
    expect(screen.queryByRole('alertdialog')).toBeNull();
    expect(api.installFromStore).not.toHaveBeenCalled();
  });

  it('Bestätigen installiert genau einmal und lädt die installierten Themes neu', async () => {
    const { api, onInstalled } = setup();
    await install();
    await userEvent.click(within(screen.getByRole('alertdialog')).getByRole('button', { name: 'Installieren' }));
    expect(await screen.findByText(/Signatur geprüft/)).toBeTruthy();
    expect(api.installFromStore).toHaveBeenCalledTimes(1);
    expect(api.installFromStore).toHaveBeenCalledWith('r1', 'ocean');
    expect(onInstalled).toHaveBeenCalled();
    expect(screen.queryByRole('alertdialog')).toBeNull();
  });

  it('zeigt die Meldung des Servers', async () => {
    setup({ extra: { installFromStore: vi.fn(async () => { throw new ApiError('store-checksum-mismatch', 'Prüfsumme passt nicht', 422); }) } });
    await install();
    await userEvent.click(within(screen.getByRole('alertdialog')).getByRole('button', { name: 'Installieren' }));
    expect((await screen.findByRole('alert')).textContent).toContain('Prüfsumme passt nicht');
  });
});

describe('Store: Updates', () => {
  const updates: StoreUpdatesDto = { updates: [{ id: 'ocean', name: 'Ocean', repoId: 'r1', installedVersion: '1.0.0', availableVersion: '1.1.0' }], errors: [] };
  const update = async () => userEvent.click(within(await screen.findByRole('list', { name: 'Updates' })).getByRole('button', { name: 'Aktualisieren' }));

  it('listet alte → neue Version und installiert reine CSS-Updates direkt', async () => {
    const { api, onInstalled } = setup({ updates });
    expect(await screen.findByText('1.0.0 → 1.1.0')).toBeTruthy();
    await update();
    expect(api.updateFromStore).toHaveBeenCalledWith('r1', 'ocean');
    await waitFor(() => expect(screen.queryByText('1.0.0 → 1.1.0')).toBeNull());
    expect(onInstalled).toHaveBeenCalled();
  });

  it('fragt bei Code oder neuen Domains nach und installiert erst nach Bestätigung', async () => {
    const updateFromStore = vi.fn(async (_r: string, _i: string, confirm?: boolean) => {
      if (!confirm) throw new ApiError('store-update-confirmation-required', 'Bestätigung nötig', 409, undefined, { hasCode: true, newDomains: ['new.example'] });
      return { manifest: testManifest(), verified: false, repoId: 'r1', sha256: 'x', previousVersion: '1.0.0' };
    });
    setup({ updates, extra: { updateFromStore } });
    await update();
    const dialog = await screen.findByRole('alertdialog', { name: 'Update installieren?' });
    expect(within(dialog).getByText('Dieses Update führt Code aus')).toBeTruthy();
    expect(within(dialog).getByText('new.example')).toBeTruthy();
    await userEvent.click(within(dialog).getByRole('button', { name: 'Installieren' }));
    expect(updateFromStore).toHaveBeenCalledTimes(2);
    expect(updateFromStore).toHaveBeenLastCalledWith('r1', 'ocean', true);
  });

  it('Abbrechen ändert nichts', async () => {
    const updateFromStore = vi.fn(async () => { throw new ApiError('store-update-confirmation-required', 'Bestätigung nötig', 409, undefined, { hasCode: false, newDomains: ['new.example'] }); });
    setup({ updates, extra: { updateFromStore } });
    await update();
    await userEvent.click(within(await screen.findByRole('alertdialog')).getByRole('button', { name: 'Abbrechen' }));
    expect(updateFromStore).toHaveBeenCalledTimes(1);
    expect(screen.getByText('1.0.0 → 1.1.0')).toBeTruthy();
  });
});
