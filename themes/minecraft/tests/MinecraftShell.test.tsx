// @vitest-environment jsdom
// Menü-Layout: Hotbar als Hauptnavigation (Klick, Tasten 1–9, Mausrad), Titelmenü mit Fokusführung, Seite als Untermenü, NavIcon
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { act, cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ThemeHostProvider, type NavItem, type ShellProps, type ThemeHost } from '@nexus/theme-sdk';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { MinecraftResources } from '../src/resources.js';

const resources = vi.hoisted(() => ({ current: null as unknown as MinecraftResources }));

vi.mock('../src/resources.js', async (importOriginal) => {
  const original = await importOriginal<typeof import('../src/resources.js')>();
  return { ...original, useMinecraftResources: () => resources.current };
});

// Die Spielschrift baut hier niemand: der Zustand kommt aus dem Stub (Browser-APIs prüft tests/useMinecraftFont.test.tsx)
const font = vi.hoisted(() => ({ state: 'ready' as 'idle' | 'building' | 'ready' | 'failed' }));

vi.mock('../src/font/useMinecraftFont.js', () => ({ useMinecraftFont: () => font.state }));

const { MINECRAFT_ENTRIES } = await import('../src/resources.js');
const { NAV_ITEM_TEXTURE } = await import('../src/navIcons.js');
const { default: theme } = await import('../src/main.js');
const { MinecraftShell } = await import('../src/MinecraftShell.js');
const { NavIcon } = await import('../src/NavIcon.js');
const { ResourcesProvider } = await import('../src/context.js');

const urls = Object.fromEntries(MINECRAFT_ENTRIES.map((e, i) => [e, `/api/themes/minecraft/fetch?i=${i}`]));
const soundUrls = { click: '/snd/click', menuOpen: '/snd/menu', hotbar: '/snd/pop' };
const ready: MinecraftResources = { phase: 'ready', done: 21, total: 21, urls, soundUrls, error: null, retry: () => {} };

const NAV: Array<[NavItem['id'], string]> = [
  ['board', 'Board'],
  ['dashboard', 'Dashboard'],
  ['inbox', 'Meine Aufgaben'],
  ['sessions', 'Sessions'],
  ['questions', 'Fragen'],
  ['docs', 'Dokumente'],
  ['search', 'Suche'],
  ['guide', 'Anleitung'],
  ['settings', 'Einstellungen'],
];

// Host mit 9 Zielen; `active` wählt das offene Ziel (null = Titelbildschirm)
function fakeHost(active: NavItem['id'] | null = 'board'): ThemeHost {
  return {
    nav: NAV.map(([id, label]) => ({ id, label, href: `/p/p1/${id}`, active: id === active })),
    boards: { current: null, projects: [], boards: [], loading: false, choose: vi.fn() },
    usage: { enabled: false, available: false, reason: null, fiveHour: null, sevenDay: null },
    settings: { values: null, loading: true, update: vi.fn(async () => {}) },
    themeSettings: { values: {}, loading: false, set: vi.fn(async () => {}) },
    colorScheme: { scheme: 'dark', themeId: 'minecraft', mode: 'minecraft' },
    navigate: vi.fn(),
    openSearch: vi.fn(),
    parts: {
      search: <button type="button">teil-suche</button>,
      boardSwitcher: <span>teil-board</span>,
      usage: <span>teil-verbrauch</span>,
      navItems: {},
      outlet: <p>teil-seite</p>,
      connection: <span>teil-banner</span>,
      status: <span>teil-status</span>,
    },
  };
}

function renderShell(host: ThemeHost) {
  const props: ShellProps = { pid: 'p1', nav: null, content: null, parts: host.parts };
  return render(
    <ThemeHostProvider value={host}>
      <MinecraftShell {...props} />
    </ThemeHostProvider>,
  );
}

// Abgespielte Töne als Liste der URLs; Audio gemockt, nie echter Ton
let played: string[];

beforeEach(() => {
  resources.current = ready;
  played = [];
  vi.stubGlobal(
    'Audio',
    vi.fn(function (this: { src: string; play: () => Promise<void> }, src: string) {
      this.src = src;
      this.play = () => {
        played.push(src);
        return Promise.resolve();
      };
    }),
  );
});

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

describe('Theme-Definition', () => {
  it('ersetzt die Slots Shell und NavIcon', () => {
    expect(Object.keys(theme.slots).sort()).toEqual(['NavIcon', 'Shell']);
  });

  it('manifest.json nennt die Slots Shell und NavIcon', () => {
    const manifest = JSON.parse(readFileSync(join(import.meta.dirname, '..', 'manifest.json'), 'utf8')) as { slots: string[] };
    expect(manifest.slots).toEqual(['Shell', 'NavIcon']);
  });
});

describe('Hotbar', () => {
  it('ist die Hauptnavigation mit 9 Feldern in der Reihenfolge des Hosts', () => {
    renderShell(fakeHost());
    const hotbar = screen.getByRole('navigation', { name: 'Hauptnavigation' });
    const links = within(hotbar).getAllByRole('link');
    expect(links).toHaveLength(9);
    expect(links.map((l) => l.getAttribute('href'))).toEqual(NAV.map(([id]) => `/p/p1/${id}`));
    expect(links[2].textContent).toContain('Meine Aufgaben');
  });

  it('das aktive Feld trägt aria-current=page und den Auswahl-Rahmen', () => {
    renderShell(fakeHost('docs'));
    const hotbar = screen.getByRole('navigation', { name: 'Hauptnavigation' });
    const active = within(hotbar).getByRole('link', { current: 'page' });
    expect(active.getAttribute('href')).toBe('/p/p1/docs');
    expect(active.classList.contains('mc-hotbar-slot--selected')).toBe(true);
    expect(hotbar.querySelectorAll('.mc-hotbar-slot--selected')).toHaveLength(1);
  });

  it('Klick auf ein Feld wechselt die Seite', () => {
    const host = fakeHost();
    renderShell(host);
    const hotbar = screen.getByRole('navigation', { name: 'Hauptnavigation' });
    fireEvent.click(within(hotbar).getByRole('link', { name: /Sessions/ }));
    expect(host.navigate).toHaveBeenCalledWith('/p/p1/sessions');
  });

  it('Tasten 1–9 wählen das Feld, nicht in Eingabefeldern und nicht mit Strg/Alt', () => {
    const host = fakeHost();
    const { container } = renderShell(host);
    fireEvent.keyDown(document.body, { key: '3' });
    expect(host.navigate).toHaveBeenLastCalledWith('/p/p1/inbox');
    fireEvent.keyDown(document.body, { key: '9' });
    expect(host.navigate).toHaveBeenLastCalledWith('/p/p1/settings');
    expect(host.navigate).toHaveBeenCalledTimes(2);

    fireEvent.keyDown(document.body, { key: '2', ctrlKey: true });
    fireEvent.keyDown(document.body, { key: '2', altKey: true });
    const input = document.createElement('input');
    container.appendChild(input);
    fireEvent.keyDown(input, { key: '2' });
    expect(host.navigate).toHaveBeenCalledTimes(2);
  });

  it('Mausrad über der Hotbar wechselt zum Nachbarn, am Rand rundherum', () => {
    const host = fakeHost('board');
    renderShell(host);
    const hotbar = screen.getByRole('navigation', { name: 'Hauptnavigation' });
    fireEvent.wheel(hotbar, { deltaY: 100 });
    expect(host.navigate).toHaveBeenLastCalledWith('/p/p1/dashboard');
    cleanup();

    const host2 = fakeHost('board');
    renderShell(host2);
    fireEvent.wheel(screen.getByRole('navigation', { name: 'Hauptnavigation' }), { deltaY: -100 });
    expect(host2.navigate).toHaveBeenLastCalledWith('/p/p1/settings');
  });

  it('zeigt die Item-Symbole über den Asset-Proxy', () => {
    renderShell(fakeHost());
    const hotbar = screen.getByRole('navigation', { name: 'Hauptnavigation' });
    const icons = hotbar.querySelectorAll('img');
    expect(icons).toHaveLength(9);
    expect(icons[0].getAttribute('src')).toBe(urls[NAV_ITEM_TEXTURE.board]);
  });
});

describe('Titelmenü', () => {
  it('Knopf „Menü“ öffnet das Hauptmenü mit allen Zielen und „Suchen …“, Fokus auf dem ersten Eintrag', async () => {
    const user = userEvent.setup();
    renderShell(fakeHost());
    expect(screen.queryByRole('menu', { name: 'Hauptmenü' })).toBeNull();
    const button = screen.getByRole('button', { name: 'Menü' });
    expect(button.getAttribute('aria-haspopup')).toBe('menu');
    await user.click(button);
    const menu = screen.getByRole('menu', { name: 'Hauptmenü' });
    const items = within(menu).getAllByRole('menuitem');
    expect(items).toHaveLength(10);
    expect(items[9].textContent).toContain('Suchen …');
    expect(document.activeElement).toBe(items[0]);
    expect(button.getAttribute('aria-expanded')).toBe('true');
  });

  it('Pfeiltasten wandern rundherum, Enter öffnet das Ziel', async () => {
    const user = userEvent.setup();
    const host = fakeHost();
    renderShell(host);
    await user.click(screen.getByRole('button', { name: 'Menü' }));
    const items = within(screen.getByRole('menu', { name: 'Hauptmenü' })).getAllByRole('menuitem');
    await user.keyboard('{ArrowDown}');
    expect(document.activeElement).toBe(items[1]);
    await user.keyboard('{ArrowUp}{ArrowUp}');
    expect(document.activeElement).toBe(items[9]);
    await user.keyboard('{ArrowUp}{Enter}');
    expect(host.navigate).toHaveBeenCalledWith('/p/p1/settings');
    expect(screen.queryByRole('menu', { name: 'Hauptmenü' })).toBeNull();
  });

  it('Esc schließt und gibt den Fokus an den Menü-Knopf zurück', async () => {
    const user = userEvent.setup();
    renderShell(fakeHost());
    const button = screen.getByRole('button', { name: 'Menü' });
    await user.click(button);
    await user.keyboard('{Escape}');
    expect(screen.queryByRole('menu', { name: 'Hauptmenü' })).toBeNull();
    expect(document.activeElement).toBe(button);
  });

  it('Esc außerhalb von Eingabefeldern öffnet das Menü', () => {
    renderShell(fakeHost());
    act(() => {
      fireEvent.keyDown(document.body, { key: 'Escape' });
    });
    expect(screen.getByRole('menu', { name: 'Hauptmenü' })).toBeTruthy();
  });

  it('„Suchen …“ öffnet die globale Suche', async () => {
    const user = userEvent.setup();
    const host = fakeHost();
    renderShell(host);
    await user.click(screen.getByRole('button', { name: 'Menü' }));
    await user.click(screen.getByRole('menuitem', { name: 'Suchen …' }));
    expect(host.openSearch).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole('menu', { name: 'Hauptmenü' })).toBeNull();
  });

  it('ohne offene Seite zeigt die Shell den Titelbildschirm', () => {
    renderShell(fakeHost(null));
    expect(screen.getByRole('menu', { name: 'Hauptmenü' })).toBeTruthy();
  });
});

describe('Untermenü', () => {
  it('hat Kopfzeile, Trennlinien, Listenfläche und Fuß', () => {
    const { container } = renderShell(fakeHost('docs'));
    const header = screen.getByRole('banner');
    expect(within(header).getByRole('button', { name: 'Menü' })).toBeTruthy();
    expect(within(header).getByText('Dokumente')).toBeTruthy();
    const main = screen.getByRole('main');
    expect(main.classList.contains('mc-list')).toBe(true);
    expect(within(main).getByText('teil-banner')).toBeTruthy();
    expect(within(main).getByText('teil-seite')).toBeTruthy();
    expect(container.querySelector('.mc-separator--top')).not.toBeNull();
    expect(container.querySelector('.mc-separator--bottom')).not.toBeNull();
    expect(container.querySelector('.mc-footer .mc-xp-slot')).not.toBeNull();
    expect(container.querySelector('.mc-panel')).toBeNull();
  });

  it('der Name des aktiven Felds steht über dem Platz der XP-Leiste', () => {
    const { container } = renderShell(fakeHost('docs'));
    const selected = container.querySelector('.mc-hotbar-slot--selected')!;
    expect(selected.querySelector('.mc-hotbar-label')!.textContent).toContain('Dokumente');
    const css = readFileSync(join(import.meta.dirname, '..', 'theme.css'), 'utf8');
    expect(css).toMatch(/\.mc-hotbar-label\s*\{[^}]*bottom:\s*calc\(100% \+ 28px\)/);
    expect(css).toMatch(/\.mc-xp-slot\s*\{[^}]*height:\s*10px/);
  });

  it('trägt den Zustand der Spielschrift an der Shell', () => {
    const { container } = renderShell(fakeHost('docs'));
    expect(container.querySelector('.mc-shell')!.getAttribute('data-mc-font')).toBe('ready');
    cleanup();
    font.state = 'failed';
    const second = renderShell(fakeHost('docs'));
    expect(second.container.querySelector('.mc-shell')!.getAttribute('data-mc-font')).toBe('failed');
    font.state = 'ready';
  });

  it('bei offenem Titelmenü bleibt die Seite eingehängt, aber verborgen', async () => {
    const user = userEvent.setup();
    const { container } = renderShell(fakeHost('docs'));
    await user.click(screen.getByRole('button', { name: 'Menü' }));
    expect(container.querySelector('.mc-screen')!.hasAttribute('hidden')).toBe(true);
    expect(screen.queryByRole('main')).toBeNull();
    expect(screen.getByText('teil-seite')).toBeTruthy();
  });
});

describe('Seite und Kopfzeile', () => {
  it('die Seite steht in der Listenfläche, die Bausteine des Hosts in der Kopfzeile', () => {
    renderShell(fakeHost('inbox'));
    const main = screen.getByRole('main');
    expect(within(main).getByText('teil-seite')).toBeTruthy();
    const header = screen.getByRole('banner');
    expect(within(header).getByText('Meine Aufgaben')).toBeTruthy();
    for (const part of ['teil-suche', 'teil-board', 'teil-verbrauch', 'teil-status']) {
      expect(within(header).getByText(part)).toBeTruthy();
    }
    expect(screen.getByText('teil-banner')).toBeTruthy();
  });

  it('Panorama mit 6 Würfelseiten, ohne Drehung bei reduzierter Bewegung', () => {
    const { container } = renderShell(fakeHost());
    const panorama = container.querySelector('.mc-panorama')!;
    expect(panorama.getAttribute('aria-hidden')).toBe('true');
    expect(panorama.querySelectorAll('.mc-panorama-face')).toHaveLength(6);
    const css = readFileSync(join(import.meta.dirname, '..', 'theme.css'), 'utf8');
    expect(css).toMatch(/prefers-reduced-motion:\s*reduce[\s\S]*\.mc-panorama-cube[\s\S]*animation:\s*none/);
  });

  it('ohne Ressourcen (offline) bleibt die Hotbar bedienbar, der Hinweis erscheint', () => {
    resources.current = {
      phase: 'error',
      done: 0,
      total: 21,
      urls: {},
      error: { status: null, code: 'network', message: 'x', offline: true },
      retry: () => {},
    };
    const host = fakeHost();
    renderShell(host);
    expect(screen.getByTestId('mc-error')).toBeTruthy();
    const hotbar = screen.getByRole('navigation', { name: 'Hauptnavigation' });
    expect(hotbar.querySelectorAll('img')).toHaveLength(0);
    fireEvent.click(within(hotbar).getByRole('link', { name: /Dashboard/ }));
    expect(host.navigate).toHaveBeenCalledWith('/p/p1/dashboard');
    fireEvent.keyDown(document.body, { key: '5' });
    expect(host.navigate).toHaveBeenLastCalledWith('/p/p1/questions');
  });
});

describe('Sounds', () => {
  it('Knopf „Menü“ klickt und öffnet das Menü mit Ton, Esc öffnet mit Ton', async () => {
    const user = userEvent.setup();
    renderShell(fakeHost());
    expect(played).toEqual([]);
    await user.click(screen.getByRole('button', { name: 'Menü' }));
    expect(played).toEqual(['/snd/click', '/snd/menu']);
    await user.keyboard('{Escape}');
    played = [];
    act(() => {
      fireEvent.keyDown(document.body, { key: 'Escape' });
    });
    expect(played).toEqual(['/snd/menu']);
  });

  it('Titelbildschirm ohne Seite bleibt beim ersten Rendern still', () => {
    renderShell(fakeHost(null));
    expect(played).toEqual([]);
  });

  it('Feldwechsel der Hotbar per Klick, Taste und Mausrad spielt den Ton, das aktive Feld nicht', () => {
    renderShell(fakeHost('board'));
    const hotbar = screen.getByRole('navigation', { name: 'Hauptnavigation' });
    fireEvent.click(within(hotbar).getByRole('link', { name: /Sessions/ }));
    fireEvent.keyDown(document.body, { key: '3' });
    fireEvent.wheel(hotbar, { deltaY: 100 });
    expect(played).toEqual(['/snd/pop', '/snd/pop', '/snd/pop']);
    fireEvent.click(within(hotbar).getByRole('link', { name: /Board/ }));
    fireEvent.keyDown(document.body, { key: '1' });
    expect(played).toHaveLength(3);
  });

  it('mit sounds: false bleibt alles still', async () => {
    const user = userEvent.setup();
    const host = fakeHost('board');
    host.themeSettings = { values: { sounds: false }, loading: false, set: vi.fn(async () => {}) };
    renderShell(host);
    const hotbar = screen.getByRole('navigation', { name: 'Hauptnavigation' });
    fireEvent.click(within(hotbar).getByRole('link', { name: /Sessions/ }));
    fireEvent.keyDown(document.body, { key: '3' });
    fireEvent.wheel(hotbar, { deltaY: 100 });
    await user.click(screen.getByRole('button', { name: 'Menü' }));
    expect(played).toEqual([]);
    expect(host.navigate).toHaveBeenCalled();
  });
});

describe('NavIcon', () => {
  it('rendert das Item-Symbol über den Asset-Proxy mit leerem alt', () => {
    const { container } = render(
      <ResourcesProvider urls={urls}>
        <NavIcon name="guide" />
      </ResourcesProvider>,
    );
    const img = container.querySelector('img')!;
    expect(img.getAttribute('src')).toBe(urls[NAV_ITEM_TEXTURE.guide]);
    expect(img.getAttribute('alt')).toBe('');
  });

  it('ohne Ressourcen ein Pixel-Feld mit dem Anfangsbuchstaben', () => {
    const { container } = render(<NavIcon name="settings" />);
    expect(container.querySelector('img')).toBeNull();
    const fallback = container.querySelector('.mc-nav-icon--fallback')!;
    expect(fallback.textContent).toBe('S');
    expect(fallback.getAttribute('aria-hidden')).toBe('true');
  });
});
