// @vitest-environment jsdom
import { act, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { loadActiveThemeModule, type ThemeModuleManifest } from '../src/theme/loadThemeModule.js';
import { getTheme, unregisterTheme } from '../src/theme/registry.js';
import { ThemeNotice } from '../src/theme/ThemeNotice.js';
import { getThemeNotice, setThemeNotice } from '../src/theme/themeNoticeStore.js';
import { THEME_CACHE_KEY } from '../src/theme/theme.js';
import { useSlot } from '../src/theme/useSlot.js';

const manifest: ThemeModuleManifest = { id: 'ocean', sdkVersion: '^1.0', slots: ['Progress'] };

function Progress() {
  return <span>Ozean-Fortschritt</span>;
}

// Zeigt die Komponente des angezeigten Themes für den Slot Progress oder „Standard“
function ProgressSlot() {
  const Slot = useSlot('Progress');
  return Slot ? <Slot done={1} total={2} label="Fortschritt" caption="1/2" /> : <span>Standard</span>;
}

function deps(over: Partial<Parameters<typeof loadActiveThemeModule>[0]> = {}) {
  return {
    importModule: vi.fn(async () => ({ default: { id: 'ocean', slots: { Progress } } })),
    fetchManifest: vi.fn(async () => manifest),
    sdkVersion: '1.1.0',
    search: '',
    timeoutMs: 50,
    ...over,
  };
}

beforeEach(() => {
  localStorage.setItem(THEME_CACHE_KEY, 'ocean');
  vi.spyOn(console, 'warn').mockImplementation(() => {});
});

afterEach(() => {
  unregisterTheme('ocean');
  localStorage.clear();
  setThemeNotice(undefined);
  delete document.documentElement.dataset.theme;
  vi.restoreAllMocks();
});

describe('main.js des aktiven Themes', () => {
  it('importiert main.js, meldet den default-Export an und der Slot greift', async () => {
    const d = deps();
    const result = await loadActiveThemeModule(d);
    expect(result.status).toBe('loaded');
    expect(d.importModule).toHaveBeenCalledWith('/themes/ocean/main.js');
    expect(getTheme('ocean')?.slots.Progress).toBe(Progress);
    document.documentElement.dataset.theme = 'ocean';
    render(<ProgressSlot />);
    expect(screen.getByText('Ozean-Fortschritt')).toBeTruthy();
  });

  it('importiert in der App über die Server-Adresse, im Browser relativ', async () => {
    const app = deps();
    await loadActiveThemeModule({ ...app, base: 'http://127.0.0.1:4710' });
    expect(app.importModule).toHaveBeenCalledWith('http://127.0.0.1:4710/themes/ocean/main.js');
    unregisterTheme('ocean');
    const browser = deps();
    await loadActiveThemeModule(browser);
    expect(browser.importModule).toHaveBeenCalledWith('/themes/ocean/main.js');
  });

  it('lädt bei unpassender sdkVersion nichts und nennt verlangte und vorhandene Version', async () => {
    const d = deps({ fetchManifest: vi.fn(async () => ({ ...manifest, sdkVersion: '^9' })) });
    const result = await loadActiveThemeModule(d);
    expect(result.status).toBe('incompatible');
    expect(d.importModule).not.toHaveBeenCalled();
    expect(getTheme('ocean')).toBeUndefined();
    expect(getThemeNotice()).toContain('SDK 1.1.0, verlangt ^9');
  });

  it('fällt bei Ladefehler auf den Standard zurück', async () => {
    const result = await loadActiveThemeModule(deps({ importModule: vi.fn(async () => { throw new Error('kaputt'); }) }));
    expect(result.status).toBe('failed');
    expect(getTheme('ocean')).toBeUndefined();
    expect(getThemeNotice()).toContain('konnte nicht geladen werden');
  });

  it('fällt bei fehlendem Manifest auf den Standard zurück', async () => {
    const result = await loadActiveThemeModule(deps({ fetchManifest: vi.fn(async () => { throw new Error('HTTP 404'); }) }));
    expect(result.status).toBe('failed');
    expect(getThemeNotice()).toContain('Manifest nicht lesbar: HTTP 404');
  });

  it('nennt es beim Namen, wenn statt des Manifests kein JSON kommt (etwa die index.html)', async () => {
    const fetchSpy = vi.spyOn(globalThis, 'fetch').mockResolvedValue(new Response('<!doctype html>', { headers: { 'content-type': 'text/html' } }));
    try {
      const { fetchManifest: _ignored, ...rest } = deps();
      const result = await loadActiveThemeModule(rest);
      expect(result.status).toBe('failed');
      expect(getThemeNotice()).toContain('Manifest nicht lesbar: keine JSON-Antwort (text/html)');
    } finally {
      fetchSpy.mockRestore();
    }
  });

  it('bricht nach dem Zeitlimit ab und registriert ein spätes Modul nicht', async () => {
    let resolve!: (m: unknown) => void;
    const late = new Promise((r) => { resolve = r; });
    const result = await loadActiveThemeModule(deps({ importModule: vi.fn(() => late), timeoutMs: 20 }));
    expect(result.status).toBe('timeout');
    expect(getThemeNotice()).toContain('nicht rechtzeitig');
    resolve({ default: { id: 'ocean', slots: { Progress } } });
    await late;
    expect(getTheme('ocean')).toBeUndefined();
  });

  it.each([
    ['ohne slots', { default: { id: 'ocean' } }],
    ['mit falscher id', { default: { id: 'andere', slots: {} } }],
    ['ohne default', {}],
  ])('lehnt einen Export %s ab', async (_name, mod) => {
    const result = await loadActiveThemeModule(deps({ importModule: vi.fn(async () => mod) }));
    expect(result.status).toBe('invalid');
    expect(getTheme('ocean')).toBeUndefined();
    expect(getThemeNotice()).toContain('keine gültige Theme-Definition');
  });

  it.each(['system', 'light', 'dark'])('lädt beim eingebauten Theme %s keinen Code', async (mode) => {
    localStorage.setItem(THEME_CACHE_KEY, mode);
    const d = deps();
    expect((await loadActiveThemeModule(d)).status).toBe('skipped');
    expect(d.fetchManifest).not.toHaveBeenCalled();
    expect(getThemeNotice()).toBeUndefined();
  });

  it('lädt main.js des Pakets „Windows 95“ wie bei jedem installierten Code-Theme', async () => {
    localStorage.setItem(THEME_CACHE_KEY, 'win95');
    const d = deps({
      fetchManifest: vi.fn(async () => ({ id: 'win95', sdkVersion: '^1.1', slots: ['Shell', 'NavIcon', 'Progress'] })),
      importModule: vi.fn(async () => ({ default: { id: 'win95', slots: { Progress } } })),
    });
    try {
      expect((await loadActiveThemeModule(d)).status).toBe('loaded');
      expect(d.importModule).toHaveBeenCalledWith('/themes/win95/main.js');
      expect(getTheme('win95')?.slots.Progress).toBe(Progress);
    } finally {
      unregisterTheme('win95');
    }
  });

  it('lädt bei einem Theme ohne Slots keinen Code', async () => {
    const d = deps({ fetchManifest: vi.fn(async () => ({ ...manifest, slots: [] })) });
    expect((await loadActiveThemeModule(d)).status).toBe('skipped');
    expect(d.importModule).not.toHaveBeenCalled();
    expect(getThemeNotice()).toBeUndefined();
  });

  it('lädt im abgesicherten Modus keinen Code und meldet das', async () => {
    const d = deps({ search: '?safe-theme' });
    expect((await loadActiveThemeModule(d)).status).toBe('skipped');
    expect(d.importModule).not.toHaveBeenCalled();
    expect(getThemeNotice()).toContain('Abgesicherter Modus');
  });
});

describe('Hinweis zum Theme', () => {
  it('ist sichtbar und lässt sich schließen', () => {
    render(<ThemeNotice />);
    expect(screen.queryByRole('status')).toBeNull();
    act(() => setThemeNotice('Theme kaputt'));
    expect(screen.getByRole('status').textContent).toContain('Theme kaputt');
    fireEvent.click(screen.getByRole('button', { name: 'Hinweis schließen' }));
    expect(screen.queryByRole('status')).toBeNull();
  });
});
