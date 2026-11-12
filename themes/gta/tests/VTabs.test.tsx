// @vitest-environment jsdom
// Reiterzeile des Pausenmenüs: ein Link je Eintrag der Navigation, Zähler als rotes Kästchen, aktiver Reiter markiert
import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, screen, within } from '@testing-library/react';
import { VTabs } from '../src/v/VTabs.js';
import { fakeGame, fakeHost, fakeNav, renderWithGame } from './helpers/fakes.js';

afterEach(cleanup);

describe('VTabs', () => {
  it('ist eine Navigation mit einem Link je Eintrag in der Reihenfolge des Hosts', () => {
    renderWithGame(<VTabs />);
    const nav = screen.getByRole('navigation', { name: 'Hauptnavigation' });
    const links = within(nav).getAllByRole('link');
    expect(links.map((link) => link.getAttribute('href'))).toEqual(fakeNav().map((item) => item.href));
    expect(links.map((link) => link.querySelector('.gta-v-tab-label')?.textContent)).toEqual(fakeNav().map((item) => item.label));
    expect(links.every((link) => link.classList.contains('gta-v-tab'))).toBe(true);
  });

  it('zeigt Zähler nur, wo der Host einen hat', () => {
    renderWithGame(<VTabs />);
    const badges = [...document.querySelectorAll('.gta-v-tab')].map((tab) => tab.querySelector('.gta-v-badge')?.textContent ?? null);
    expect(badges).toEqual([null, null, '2', null, '1', null, null, null]);
  });

  it('markiert den aktiven Reiter als aktuelle Seite', () => {
    const host = fakeHost({ nav: fakeNav('docs') });
    renderWithGame(<VTabs />, fakeGame(), host);
    const current = screen.getAllByRole('link').filter((link) => link.getAttribute('aria-current') === 'page');
    expect(current).toHaveLength(1);
    expect(current[0]!.getAttribute('href')).toBe('/p/p1/docs');
  });

  it('holt den aktiven Reiter in den sichtbaren Bereich', () => {
    const scrolled: string[] = [];
    const original = HTMLElement.prototype.scrollIntoView;
    HTMLElement.prototype.scrollIntoView = function (this: HTMLElement) {
      scrolled.push(this.getAttribute('href') ?? '');
    };
    try {
      const view = renderWithGame(<VTabs />, fakeGame(), fakeHost({ nav: fakeNav('board') }));
      expect(scrolled).toEqual(['/p/p1/board']);
      view.unmount();
      renderWithGame(<VTabs />, fakeGame(), fakeHost({ nav: fakeNav('settings') }));
      expect(scrolled).toEqual(['/p/p1/board', '/p/p1/settings']);
    } finally {
      HTMLElement.prototype.scrollIntoView = original;
    }
  });

  it('ein Klick wechselt über den Host die Seite', () => {
    const host = fakeHost();
    renderWithGame(<VTabs />, fakeGame(), host);
    fireEvent.click(screen.getByRole('link', { name: /Sessions/ }));
    expect(host.navigate).toHaveBeenCalledWith('/p/p1/sessions');
  });
});
