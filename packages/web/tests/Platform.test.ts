// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { ApiClient } from '../src/api/ApiClient.js';
import { BrowserPlatform } from '../src/platform/BrowserPlatform.js';
import { createPlatform, isTauri } from '../src/platform/createPlatform.js';
import { TauriPlatform, joinDocPath, vscodeUrl } from '../src/platform/TauriPlatform.js';

// Die Plugin-Pakete gibt es im Test nicht als Fenster der App; ihre Funktionen sind deshalb gefälscht
const open = vi.fn();
const revealItemInDir = vi.fn();
const openUrl = vi.fn();
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: (...args: unknown[]) => open(...args) }));
vi.mock('@tauri-apps/plugin-opener', () => ({
  revealItemInDir: (...args: unknown[]) => revealItemInDir(...args),
  openUrl: (...args: unknown[]) => openUrl(...args),
}));

// Lässt die Oberfläche glauben, sie laufe im Fenster der Desktop-App
function asTauri() {
  (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {};
}

afterEach(() => {
  delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
  vi.clearAllMocks();
});

describe('isTauri', () => {
  it('erkennt den Browser ohne __TAURI_INTERNALS__', () => {
    expect(isTauri()).toBe(false);
  });

  it('erkennt die Desktop-App an __TAURI_INTERNALS__', () => {
    asTauri();
    expect(isTauri()).toBe(true);
  });
});

describe('createPlatform', () => {
  const api = {} as ApiClient;

  it('liefert im Browser die Plattform des Servers', () => {
    expect(createPlatform(api).kind).toBe('browser');
  });

  it('liefert in der Desktop-App die Plattform von Tauri', () => {
    asTauri();
    expect(createPlatform(api).kind).toBe('tauri');
  });
});

describe('BrowserPlatform', () => {
  it('holt den Ordner über den Server', async () => {
    const pickDbFolder = vi.fn(async () => ({ path: 'D:/Repos/Gewählt' }));
    const platform = new BrowserPlatform({ pickDbFolder } as unknown as ApiClient);
    await expect(platform.pickFolder('D:/Repos')).resolves.toBe('D:/Repos/Gewählt');
    expect(pickDbFolder).toHaveBeenCalledWith('D:/Repos');
  });

  it('gibt den Abbruch der Ordnerauswahl weiter', async () => {
    const platform = new BrowserPlatform({ pickDbFolder: async () => ({ path: null }) } as unknown as ApiClient);
    await expect(platform.pickFolder()).resolves.toBeNull();
  });

  it('öffnet das Dokument über den Server', async () => {
    const openDoc = vi.fn(async () => ({ ok: true as const }));
    const platform = new BrowserPlatform({ openDoc } as unknown as ApiClient);
    await platform.openDoc({ pid: 'p1', path: 'docs/a.md', root: '/repo', board: 3, target: 'vscode' });
    expect(openDoc).toHaveBeenCalledWith('p1', 'docs/a.md', 'vscode', 3);
  });

  it('öffnet eine fremde Adresse in einem neuen Tab', async () => {
    const spy = vi.spyOn(window, 'open').mockReturnValue(null);
    await new BrowserPlatform({} as ApiClient).openExternal('https://example.test/x');
    expect(spy).toHaveBeenCalledWith('https://example.test/x', '_blank', 'noopener');
    spy.mockRestore();
  });
});

describe('joinDocPath', () => {
  it('setzt unter macOS und Linux mit Schrägstrich zusammen', () => {
    expect(joinDocPath('/repo', 'docs/a.md')).toBe('/repo/docs/a.md');
  });

  it('setzt eine Windows-Wurzel mit Rückstrich zusammen', () => {
    expect(joinDocPath('C:\\repo', 'docs/a.md')).toBe('C:\\repo\\docs\\a.md');
  });

  it('stört sich nicht an einem Trenner zu viel', () => {
    expect(joinDocPath('/repo/', '/docs/a.md')).toBe('/repo/docs/a.md');
  });
});

describe('vscodeUrl', () => {
  it('bildet die Adresse eines Pfades mit Schrägstrichen', () => {
    expect(vscodeUrl('/repo/docs/a.md')).toBe('vscode://file/repo/docs/a.md');
  });

  it('bildet die Adresse eines Windows-Pfades mit Laufwerk', () => {
    expect(vscodeUrl('C:\\repo\\docs\\a.md')).toBe('vscode://file/C:/repo/docs/a.md');
  });

  it('kodiert Leerzeichen im Pfad', () => {
    expect(vscodeUrl('/repo/mein ordner/a.md')).toBe('vscode://file/repo/mein%20ordner/a.md');
  });
});

describe('TauriPlatform', () => {
  const platform = new TauriPlatform();

  it('öffnet den nativen Ordnerdialog beim bisherigen Pfad', async () => {
    open.mockResolvedValue('/repo/gewählt');
    await expect(platform.pickFolder('/repo')).resolves.toBe('/repo/gewählt');
    expect(open).toHaveBeenCalledWith({ directory: true, multiple: false, defaultPath: '/repo' });
  });

  it('liefert beim Abbruch des Dialogs null', async () => {
    open.mockResolvedValue(null);
    await expect(platform.pickFolder()).resolves.toBeNull();
  });

  it('zeigt die Datei im Dateimanager', async () => {
    revealItemInDir.mockResolvedValue(undefined);
    await platform.openDoc({ pid: 'p1', path: 'docs/a.md', root: '/repo', target: 'explorer' });
    expect(revealItemInDir).toHaveBeenCalledWith('/repo/docs/a.md');
    expect(openUrl).not.toHaveBeenCalled();
  });

  it('öffnet die Datei in VS Code', async () => {
    openUrl.mockResolvedValue(undefined);
    await platform.openDoc({ pid: 'p1', path: 'docs/a.md', root: 'C:\\repo', target: 'vscode' });
    expect(openUrl).toHaveBeenCalledWith('vscode://file/C:/repo/docs/a.md');
    expect(revealItemInDir).not.toHaveBeenCalled();
  });

  it('sagt es, wenn der Ordner des Dokuments unbekannt ist', async () => {
    await expect(platform.openDoc({ pid: 'p1', path: 'docs/a.md', root: null, target: 'explorer' }))
      .rejects.toThrow('Der Ordner des Dokuments ist unbekannt.');
    expect(revealItemInDir).not.toHaveBeenCalled();
  });

  it('macht aus einem Fehler des Openers einen verständlichen Text', async () => {
    openUrl.mockRejectedValue(new Error('not allowed'));
    await expect(platform.openDoc({ pid: 'p1', path: 'docs/a.md', root: '/repo', target: 'vscode' }))
      .rejects.toThrow('VS Code ließ sich nicht öffnen. (not allowed)');
  });

  it('öffnet eine fremde Adresse im Standardbrowser', async () => {
    openUrl.mockResolvedValue(undefined);
    await platform.openExternal('https://example.test/x');
    expect(openUrl).toHaveBeenCalledWith('https://example.test/x');
  });
});
