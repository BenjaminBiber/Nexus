// @vitest-environment jsdom
import { describe, it, expect, afterEach, vi } from 'vitest';
import { cleanup, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import { App } from '../src/App.js';
import { ApiProvider } from '../src/api/ApiContext.js';
import { boardPickerTrigger, pickBoard, pickProject } from './helpers/boardPicker.js';
import { dbActive, dbProjects, fakeApi, inboxItem, inboxList } from './helpers/fakeApi.js';

afterEach(cleanup);

// Rendert die ganze App unter einer Adresse
function openApp(path: string, overrides: Parameters<typeof fakeApi>[0] = {}) {
  return render(
    <ApiProvider api={fakeApi(overrides)}>
      <MemoryRouter initialEntries={[path]}>
        <App />
      </MemoryRouter>
    </ApiProvider>,
  );
}

describe('App-Rahmen', () => {
  it('zeigt Board, Meine Aufgaben, Dashboard, Sessions, Fragen, Docs, Anleitung und zuletzt Einstellungen – ohne alte Einträge', async () => {
    openApp('/p/p1/board');
    await screen.findByRole('heading', { name: 'Board', level: 1 });
    const nav = screen.getByRole('navigation', { name: 'Hauptnavigation' });
    const names = Array.from(nav.querySelectorAll('a')).map((a) => a.textContent?.trim() ?? '').filter(Boolean);
    expect(names.filter((n) => !n.startsWith('Fragen'))).toEqual(['Board', 'Meine Aufgaben', 'Dashboard', 'Sessions', 'Docs', 'Anleitung', 'Einstellungen']);
    expect(names.some((n) => n.startsWith('Fragen'))).toBe(true);
    expect(names.some((n) => n.includes('(alt)'))).toBe(false);
    expect(screen.getByRole('link', { name: 'Board' }).getAttribute('href')).toBe('/p/p1/board');
    expect(screen.getByRole('link', { name: 'Dashboard' }).getAttribute('href')).toBe('/p/p1/dashboard');
    expect(screen.getByRole('link', { name: 'Einstellungen' }).getAttribute('href')).toBe('/p/p1/settings');
  });

  it('bietet in der Seitenleiste Projekte und Boards an und aktiviert auf anderen Seiten das gewählte Board', async () => {
    let activeId = 1;
    const activateDbBoard = vi.fn(async (id: number) => dbActive((activeId = id)));
    openApp('/p/p1/sessions', { activateDbBoard, getDbActive: async () => dbActive(activeId) });
    const trigger = await screen.findByRole('button', { name: /^Board wählen/ });
    expect(trigger.getAttribute('aria-label')).toBe('Board wählen: Arches › Sprint');
    await userEvent.click(trigger);
    const projects = screen.getByRole('list', { name: 'Projekte' });
    expect(within(projects).getAllByRole('button').map((b) => b.textContent)).toEqual(['ARCArches2', 'OTHOther1']);
    await userEvent.keyboard('{Escape}');
    await pickBoard('Fremd', 'OTH');
    await waitFor(() => expect(activateDbBoard).toHaveBeenCalledWith(3));
    await waitFor(() => expect(boardPickerTrigger().getAttribute('aria-label')).toBe('Board wählen: Other › Fremd'));
  });

  it('bietet ohne Board-Projekt den Stift an, damit sich das erste Projekt anlegen lässt', async () => {
    openApp('/p/p1/sessions', { getDbProjects: async () => [], getDbBoards: async () => [], getDbActive: async () => dbActive(null) });
    const trigger = await screen.findByRole('button', { name: 'Board wählen: Noch kein Projekt' });
    expect((trigger as HTMLButtonElement).disabled).toBe(true);
    await userEvent.click(screen.getByRole('button', { name: 'Projekte und Boards verwalten' }));
    expect(screen.getByRole('dialog', { name: 'Verwaltung' })).toBeTruthy();
  });

  it('öffnet ein Projekt ohne Board aus der Seitenleiste auf dem Board', async () => {
    openApp('/p/p1/sessions', { getDbProjects: async () => [...dbProjects, { ...dbProjects[1], id: 9, key: 'NEU', name: 'Neu' }] });
    await screen.findByRole('button', { name: /^Board wählen/ });
    await pickProject('NEU');
    expect(await screen.findByText('Noch kein Board in NEU')).toBeTruthy();
  });

  it('zeigt „Meine Aufgaben“ direkt nach Board mit Zähler aller wartenden Einträge', async () => {
    openApp('/p/p1/board', { getInbox: async () => inboxList([inboxItem('ticket:ARC-0001', 'merge', 'Eins'), inboxItem('note:3', 'note', 'Zwei')]) });
    const link = await screen.findByRole('link', { name: 'Meine Aufgaben, 2 offen' });
    expect(link.getAttribute('href')).toBe('/p/p1/inbox');
    expect(link.querySelector('.nav-badge')?.textContent).toBe('2');
    const nav = screen.getByRole('navigation', { name: 'Hauptnavigation' });
    expect(Array.from(nav.querySelectorAll('a')).slice(0, 2).map((a) => a.getAttribute('href'))).toEqual(['/p/p1/board', '/p/p1/inbox']);
  });

  it('zeigt keine Auswahl der Harness-Projekte mehr', async () => {
    openApp('/p/p1/board');
    await screen.findByRole('heading', { name: 'Board', level: 1 });
    expect(screen.queryByLabelText('Projekt')).toBeNull();
  });

  it.each(['/p/p1/board-alt', '/p/p1/tickets/T-0042', '/p/p1/questions-alt', '/p/p1/gibt-es-nicht/x'])('leitet die alte Adresse %s zum Board weiter', async (path) => {
    openApp(path);
    expect(await screen.findByRole('heading', { name: 'Board', level: 1 })).toBeTruthy();
    expect(screen.getByRole('link', { name: 'Board' }).getAttribute('aria-current')).toBe('page');
  });

  it('leitet die Startadresse zum Board des ersten Projekts', async () => {
    openApp('/');
    expect(await screen.findByRole('heading', { name: 'Board', level: 1 })).toBeTruthy();
  });

  it('zeigt ohne angemeldete Projekte den Hinweis auf nexus board', async () => {
    openApp('/', { getProjects: async () => [] });
    expect(await screen.findByText('Noch kein Projekt angemeldet')).toBeTruthy();
    expect(screen.getByText('nexus board')).toBeTruthy();
  });
});
