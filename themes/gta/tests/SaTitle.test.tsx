// @vitest-environment jsdom
// Kopf des Inhalts im Stil San Andreas: gotischer Seitentitel, Unterzeile aus echten Daten und Hilfekasten
import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, screen } from '@testing-library/react';
import { SaTitle } from '../src/sa/SaTitle.js';
import { fakeGame, fakeNav, renderWithGame } from './helpers/fakes.js';

afterEach(cleanup);

const info = () => document.querySelector('.gta-sa-title-info')?.textContent;
const onPage = (id: 'board' | 'docs') => {
  const nav = fakeNav(id);
  return { nav, active: nav.find((item) => item.active) };
};

describe('SaTitle', () => {
  it('zeigt den Namen der aktiven Seite groß, ohne zweites h1 neben dem der Seite', () => {
    renderWithGame(<SaTitle />, fakeGame(onPage('docs')));
    expect(document.querySelector('.gta-sa-title-page')?.textContent).toBe('Docs');
    expect(screen.queryByRole('heading')).toBeNull();
  });

  it('auf dem Board: offene Aufträge und blockierte', () => {
    renderWithGame(<SaTitle />, fakeGame({ ...onPage('board'), total: 7, open: 5, blocked: 1 }));
    expect(info()).toBe('5 Aufträge, 1 blockiert');
    cleanup();
    renderWithGame(<SaTitle />, fakeGame({ ...onPage('board'), open: 1, blocked: 0 }));
    expect(info()).toBe('1 Auftrag, 0 blockiert');
  });

  it('auf anderen Seiten: offene Tickets und Fragen', () => {
    renderWithGame(<SaTitle />, fakeGame({ ...onPage('docs'), open: 5, questions: 2 }));
    expect(info()).toBe('5 offen · 2 Fragen');
    cleanup();
    renderWithGame(<SaTitle />, fakeGame({ ...onPage('docs'), open: 5, questions: 1 }));
    expect(info()).toBe('5 offen · 1 Frage');
  });

  it('ohne Board-Stand nennt die Unterzeile nur, was bekannt ist', () => {
    renderWithGame(<SaTitle />, fakeGame({ ...onPage('docs'), loaded: false, open: 0, questions: 2 }));
    expect(info()).toBe('2 Fragen');
    cleanup();
    renderWithGame(<SaTitle />, fakeGame({ ...onPage('board'), loaded: false, open: 0, blocked: 0, questions: 1 }));
    expect(info()).toBe('1 Frage');
  });

  it('der Hilfekasten erscheint nur nach einem Board-Wechsel; die Statushülle bleibt stehen', () => {
    const view = renderWithGame(<SaTitle />);
    expect(screen.getByRole('status').textContent).toBe('');
    expect(document.querySelector('[data-gta-part="station"]')).toBeNull();
    view.unmount();
    renderWithGame(<SaTitle />, fakeGame({ station: 'Nova App' }));
    const station = document.querySelector('[data-gta-part="station"]') as HTMLElement;
    expect(screen.getByRole('status').contains(station)).toBe(true);
    expect(station.textContent).toContain('Nova App');
  });
});
