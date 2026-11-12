// @vitest-environment jsdom
// Lade-Gate: Fortschritt, Shell mit CSS-Variablen bei ready, Hinweis ohne Netz – die Seite bleibt bedienbar
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { LoadingGate } from '../src/LoadingGate.js';
import { MINECRAFT_ENTRIES, type MinecraftResources } from '../src/resources.js';

afterEach(cleanup);

const base: MinecraftResources = { phase: 'entries', done: 3, total: 9, urls: {}, error: null, retry: () => {} };
const page = <button type="button">Seite</button>;

describe('LoadingGate', () => {
  it('zeigt während des Ladens den Fortschritt und die Seite', () => {
    render(<LoadingGate resources={base}>{page}</LoadingGate>);
    const loading = screen.getByTestId('mc-loading');
    expect(loading.textContent).toContain('Minecraft-Ressourcen werden geladen … 3/9');
    const bar = loading.querySelector('progress')!;
    expect(bar.value).toBe(3);
    expect(bar.max).toBe(9);
    expect(screen.getByRole('button', { name: 'Seite' })).toBeTruthy();
  });

  it('setzt bei ready die CSS-Variablen auf die Proxy-URLs', () => {
    const urls = Object.fromEntries(MINECRAFT_ENTRIES.map((e, i) => [e, `/api/themes/minecraft/fetch?i=${i}`]));
    render(<LoadingGate resources={{ ...base, phase: 'ready', done: 9, urls }}>{page}</LoadingGate>);
    expect(screen.queryByTestId('mc-loading')).toBeNull();
    const shell = screen.getByTestId('mc-shell');
    expect(shell.style.getPropertyValue('--mc-font-atlas')).toBe('url("/api/themes/minecraft/fetch?i=0")');
    expect(shell.style.getPropertyValue('--mc-button')).toBe('url("/api/themes/minecraft/fetch?i=1")');
    expect(shell.style.getPropertyValue('--mc-button-highlighted')).toBe('url("/api/themes/minecraft/fetch?i=2")');
    expect(shell.style.getPropertyValue('--mc-panorama-0')).toBe('url("/api/themes/minecraft/fetch?i=3")');
    expect(shell.style.getPropertyValue('--mc-panorama-5')).toBe('url("/api/themes/minecraft/fetch?i=8")');
    expect(shell.style.getPropertyValue('--mc-hotbar')).toBe('url("/api/themes/minecraft/fetch?i=9")');
    expect(shell.style.getPropertyValue('--mc-hotbar-selection')).toBe('url("/api/themes/minecraft/fetch?i=10")');
  });

  it('ohne Netz: Hinweis mit „Erneut versuchen“, Seite bleibt bedienbar', () => {
    const retry = vi.fn();
    const onPage = vi.fn();
    render(
      <LoadingGate resources={{ ...base, phase: 'error', retry, error: { status: null, code: 'network', message: 'x', offline: true } }}>
        <button type="button" onClick={onPage}>
          Seite
        </button>
      </LoadingGate>,
    );
    const error = screen.getByTestId('mc-error');
    expect(error.textContent).toContain(
      'Keine Verbindung zu Mojang. Die Ressourcen werden beim ersten Start einmal geladen. Bitte Internetverbindung prüfen und erneut versuchen.',
    );
    fireEvent.click(screen.getByRole('button', { name: 'Erneut versuchen' }));
    expect(retry).toHaveBeenCalledTimes(1);
    fireEvent.click(screen.getByRole('button', { name: 'Seite' }));
    expect(onPage).toHaveBeenCalledTimes(1);
  });

  it('bei 403 nennt der Hinweis die fehlende Domain-Freigabe', () => {
    const message = 'Fehlende Domain-Freigabe für Mojang im Manifest (permissions.network): piston-meta.mojang.com';
    render(
      <LoadingGate resources={{ ...base, phase: 'error', error: { status: 403, code: 'theme_fetch_forbidden', message, offline: false } }}>
        {page}
      </LoadingGate>,
    );
    expect(screen.getByTestId('mc-error').textContent).toContain(message);
  });
});
