// @vitest-environment jsdom
// Lade-Gate: Fortschritt, Shell mit CSS-Variablen bei ready, Hinweis ohne Netz – die Seite bleibt bedienbar
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { LoadingGate } from '../src/LoadingGate.js';
import { INDEX_ENTRIES, MINECRAFT_ENTRIES, type MinecraftResources } from '../src/resources.js';

// Alle Einträge, die im Theme eine URL haben: die aus dem JAR und die aus dem Asset-Index
const ALL_ENTRIES = [...MINECRAFT_ENTRIES, ...INDEX_ENTRIES];

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
    // Ohne gemeldeten Zustand gilt die Spielschrift als noch nicht gebaut
    expect(screen.getByTestId('mc-shell').getAttribute('data-mc-font')).toBe('idle');
  });

  it('setzt bei ready die CSS-Variablen auf die Proxy-URLs', () => {
    const urls = Object.fromEntries(ALL_ENTRIES.map((e, i) => [e, `/api/themes/minecraft/fetch?i=${i}`]));
    const at = (entry: string) => `url("/api/themes/minecraft/fetch?i=${ALL_ENTRIES.indexOf(entry)}")`;
    render(<LoadingGate resources={{ ...base, phase: 'ready', done: 9, urls }}>{page}</LoadingGate>);
    expect(screen.queryByTestId('mc-loading')).toBeNull();
    const shell = screen.getByTestId('mc-shell');
    expect(shell.style.getPropertyValue('--mc-button')).toBe(at('assets/minecraft/textures/gui/sprites/widget/button.png'));
    expect(shell.style.getPropertyValue('--mc-button-highlighted')).toBe(at('assets/minecraft/textures/gui/sprites/widget/button_highlighted.png'));
    expect(shell.style.getPropertyValue('--mc-button-disabled')).toBe(at('assets/minecraft/textures/gui/sprites/widget/button_disabled.png'));
    expect(shell.style.getPropertyValue('--mc-text-field')).toBe(at('assets/minecraft/textures/gui/sprites/widget/text_field.png'));
    expect(shell.style.getPropertyValue('--mc-text-field-highlighted')).toBe(
      at('assets/minecraft/textures/gui/sprites/widget/text_field_highlighted.png'),
    );
    expect(shell.style.getPropertyValue('--mc-panorama-0')).toBe(at('minecraft/textures/gui/title/background/panorama_0.png'));
    expect(shell.style.getPropertyValue('--mc-panorama-5')).toBe(at('minecraft/textures/gui/title/background/panorama_5.png'));
    expect(shell.style.getPropertyValue('--mc-hotbar')).toBe(at('assets/minecraft/textures/gui/sprites/hud/hotbar.png'));
    expect(shell.style.getPropertyValue('--mc-hotbar-selection')).toBe(at('assets/minecraft/textures/gui/sprites/hud/hotbar_selection.png'));
    expect(shell.style.getPropertyValue('--mc-menu-bg')).toBe(at('assets/minecraft/textures/gui/menu_background.png'));
    expect(shell.style.getPropertyValue('--mc-list-bg')).toBe(at('assets/minecraft/textures/gui/menu_list_background.png'));
    expect(shell.style.getPropertyValue('--mc-sep-top')).toBe(at('assets/minecraft/textures/gui/header_separator.png'));
    expect(shell.style.getPropertyValue('--mc-sep-bottom')).toBe(at('assets/minecraft/textures/gui/footer_separator.png'));
    expect(shell.style.getPropertyValue('--mc-popup')).toBe(at('assets/minecraft/textures/gui/sprites/popup/background.png'));
    expect(shell.style.getPropertyValue('--mc-xp-bg')).toBe(at('assets/minecraft/textures/gui/sprites/hud/experience_bar_background.png'));
    expect(shell.style.getPropertyValue('--mc-xp-fill')).toBe(at('assets/minecraft/textures/gui/sprites/hud/experience_bar_progress.png'));
    expect(shell.style.getPropertyValue('--mc-scroller')).toBe(at('assets/minecraft/textures/gui/sprites/widget/scroller.png'));
    expect(shell.style.getPropertyValue('--mc-scroller-bg')).toBe(at('assets/minecraft/textures/gui/sprites/widget/scroller_background.png'));
    expect(shell.style.getPropertyValue('--mc-checkbox')).toBe(at('assets/minecraft/textures/gui/sprites/widget/checkbox.png'));
    expect(shell.style.getPropertyValue('--mc-checkbox-highlighted')).toBe(
      at('assets/minecraft/textures/gui/sprites/widget/checkbox_highlighted.png'),
    );
    expect(shell.style.getPropertyValue('--mc-checkbox-selected')).toBe(
      at('assets/minecraft/textures/gui/sprites/widget/checkbox_selected.png'),
    );
    expect(shell.style.getPropertyValue('--mc-checkbox-selected-highlighted')).toBe(
      at('assets/minecraft/textures/gui/sprites/widget/checkbox_selected_highlighted.png'),
    );
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
