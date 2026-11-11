// @vitest-environment jsdom
// Rahmen im Stil GTA V: Szene, Kopf mit Wortmarke, Hinweisfeld und HUD, Reiterzeile, Inhalt und Fußleiste
import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, screen, within } from '@testing-library/react';
import { VShell } from '../src/v/VShell.js';
import { fakeGame, fakeHost, renderWithGame } from './helpers/fakes.js';

afterEach(cleanup);

const part = (name: string) => document.querySelector(`[data-gta-part="${name}"]`) as HTMLElement | null;
const content = <main className="shell-main">seiten-inhalt</main>;

describe('VShell', () => {
  it('trägt die Prüfmarke shell und zeigt Wortmarke, HUD, Reiter und Inhalt', () => {
    renderWithGame(<VShell content={content} />);
    const shell = part('shell')!;
    expect(shell.classList.contains('gta-v')).toBe(true);
    const header = shell.querySelector('header') as HTMLElement;
    expect(within(header).getByRole('img', { name: 'Nexus' }).getAttribute('data-wordmark')).toBe('nexus');
    expect(header.contains(part('hud'))).toBe(true);
    expect(screen.getAllByRole('navigation')).toHaveLength(1);
    // Der Kern liefert das <main> selbst; der Rahmen legt nur einen Scroll-Bereich darum
    expect(screen.getAllByRole('main')).toHaveLength(1);
    expect(shell.querySelector('.gta-v-main')?.textContent).toBe('seiten-inhalt');
  });

  it('legt die Szene als Schmuck hinter alles', () => {
    renderWithGame(<VShell content={content} />);
    const scene = part('shell')!.querySelector('.gta-v-scene') as HTMLElement;
    expect(scene.getAttribute('aria-hidden')).toBe('true');
    expect(scene.querySelector('svg')).not.toBeNull();
  });

  it('zeigt unten rechts Board und Seite in Schreibschrift, hervorgehoben nur nach einem Wechsel', () => {
    const view = renderWithGame(<VShell content={content} />);
    const zone = part('zone')!;
    expect(zone.querySelector('.gta-v-zone-board')?.textContent).toBe('Nova Shop');
    expect(zone.querySelector('.gta-v-zone-page')?.textContent).toBe('Board');
    expect(zone.hasAttribute('data-flash')).toBe(false);
    view.unmount();
    renderWithGame(<VShell content={content} />, fakeGame({ zone: 'Board' }));
    expect(part('zone')!.hasAttribute('data-flash')).toBe(true);
  });

  it('Tastenhinweise: Q und E für die Reiter, Strg K öffnet per Klick die Suche', () => {
    const host = fakeHost();
    renderWithGame(<VShell content={content} />, fakeGame(), host);
    const keys = part('shell')!.querySelector('.gta-v-keys') as HTMLElement;
    expect([...keys.querySelectorAll('kbd')].map((kbd) => kbd.textContent)).toEqual(['Q', 'E', 'Strg', 'K']);
    fireEvent.click(within(keys).getByRole('button', { name: /Suchen/ }));
    expect(host.openSearch).toHaveBeenCalledTimes(1);
  });

  it('der Stats-Knopf öffnet das Stats-Menü', () => {
    const game = fakeGame();
    renderWithGame(<VShell content={content} />, game);
    fireEvent.click(part('stats-button')!);
    expect(game.setStatsOpen).toHaveBeenCalledWith(true);
  });

  it('das Hinweisfeld erscheint nur bei Bedarf: Cheat-Meldung oder Board-Wechsel', () => {
    const view = renderWithGame(<VShell content={content} />);
    expect(part('cheat')).toBeNull();
    expect(part('station')).toBeNull();
    view.unmount();

    const cheats = [
      ['money', 'Cheat aktiviert – +$250.000'],
      ['clear', 'Cheat aktiviert – Fahndungslevel aufgehoben'],
      ['wanted', 'Cheat aktiviert – Fünf Sterne'],
    ] as const;
    for (const [cheat, text] of cheats) {
      const shown = renderWithGame(<VShell content={content} />, fakeGame({ cheat }));
      expect(part('cheat')?.textContent).toBe(text);
      shown.unmount();
    }

    renderWithGame(<VShell content={content} />, fakeGame({ station: 'Nova App' }));
    expect(part('station')?.textContent).toContain('Nova App');
  });
});
