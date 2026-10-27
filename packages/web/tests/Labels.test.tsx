// @vitest-environment jsdom
import { describe, it, expect, afterEach, vi } from 'vitest';
import { act, cleanup, render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { App } from '../src/App.js';
import { ApiProvider } from '../src/api/ApiContext.js';
import { FakeEventSource } from './helpers/FakeEventSource.js';
import { dbCard, dbView, fakeApi } from './helpers/fakeApi.js';

vi.mock('mermaid', () => ({ default: { initialize: vi.fn(), render: vi.fn(async () => ({ svg: '<svg></svg>' })) } }));

afterEach(cleanup);

// Rendert die App an einer Adresse mit Board-Karten, Sessions, Favoriten, Docs und Einstellungen und wartet auf einen Text
async function show(path: string, waitFor: string) {
  const api = fakeApi({
    getDbBoardView: async () => dbView([
      dbCard('ARC-0001', 'Eins', 'in_progress', { stepsDone: 1, stepsTotal: 2, sessions: ['aaaaaaaa-1'], waitingOnQuestions: [{ id: 'ARC-F0001', title: 'Offen?' }] }),
      dbCard('ARC-0002', 'Zwei', 'done'),
    ]),
    getSessions: async () => [
      {
        id: 'aaaaaaaa-1', projectId: 'p1', tickets: ['ARC-0001'], ticketInfo: [{ id: 'ARC-0001', title: 'Eins', status: 'in_progress' }], cwd: 'D:/wt',
        state: 'running', displayState: 'running', listening: true, startedAt: '2026-10-10T10:00:00.000Z', lastEventAt: new Date().toISOString(),
        endedAt: null, runtimeMs: 60000, title: null, model: null,
        subagents: [{ id: 'a', type: 'Explore', state: 'running', startedAt: '2026-10-10T10:00:00.000Z', endedAt: null }],
      },
      {
        id: 'bbbbbbbb-2', projectId: 'p1', tickets: [], ticketInfo: [], cwd: 'D:/x', state: 'waiting', displayState: 'stale', listening: false,
        startedAt: '2026-10-10T08:00:00.000Z', lastEventAt: '2026-10-10T08:00:00.000Z', endedAt: null, runtimeMs: 1000, title: null, model: null, subagents: [],
      },
    ],
    getFavorites: async () => [
      {
        sessionId: 'aaaaaaaa-1', note: 'Merken', tickets: ['ARC-0001'], ticketInfo: [{ id: 'ARC-0001', title: 'Eins', status: 'in_progress' }], cwd: 'D:/wt',
        cwdExists: false, title: null, markedAt: '2026-10-10T10:00:00.000Z', state: 'running', displayState: 'running', live: true,
        lastEventAt: new Date().toISOString(), resumedAs: ['cccccccc-3'], command: 'cd "D:/wt" && claude --resume aaaaaaaa-1',
      },
    ],
    getDocs: async () => ({ source: { root: 'C:/repo', boardId: 1, boardName: 'Sprint', reason: null }, docs: [{ path: 'GLOSSARY.md', title: 'Glossar', group: 'glossary' as const, kind: null, topic: null, date: null, mtimeMs: 1, exists: true }] }),
    getSettings: async () => ({ project: { id: 'p1', name: 'Demo', path: '/demo' }, token: 't' }),
  });
  render(
    <ApiProvider api={api}>
      <MemoryRouter initialEntries={[path]}>
        <App />
      </MemoryRouter>
    </ApiProvider>,
  );
  await screen.findAllByText(waitFor);
}

const PAGES: Array<[string, string, string]> = [
  ['Board', '/p/p1/board', 'Eins'],
  ['Docs', '/p/p1/docs', 'GLOSSARY.md'],
  ['Einstellungen', '/p/p1/settings', 'Einstellungen'],
  ['Anleitung', '/p/p1/guide', 'Anleitung'],
  ['Sessions', '/p/p1/sessions', 'Sessions'],
];

describe('Beschriftung (SC-008): Zustände nie nur durch Farbe', () => {
  it.each(PAGES)('%s: jede Marke trägt Text', async (_name, path, wait) => {
    await show(path, wait);
    for (const badge of Array.from(document.querySelectorAll('.badge'))) expect(badge.textContent?.trim().length, badge.outerHTML).toBeGreaterThan(0);
  });

  it.each(PAGES)('%s: jedes Eingabefeld hat einen zugänglichen Namen', async (_name, path, wait) => {
    await show(path, wait);
    for (const control of Array.from(document.querySelectorAll<HTMLElement>('input, select, textarea'))) {
      const labelled = (control as HTMLInputElement).labels?.length || control.getAttribute('aria-label');
      expect(Boolean(labelled), control.outerHTML).toBe(true);
    }
  });

  it.each(PAGES)('%s: jede Schaltfläche hat einen Namen', async (_name, path, wait) => {
    await show(path, wait);
    for (const button of Array.from(document.querySelectorAll('button'))) {
      const name = (button.getAttribute('aria-label') ?? button.textContent ?? '').trim();
      expect(name.length, button.outerHTML).toBeGreaterThan(0);
    }
  });

  it('Board: Kennzeichen der Karte stehen als Text da', async () => {
    await show('/p/p1/board', 'Eins');
    for (const text of ['1/2 Schritte', '1 Session', 'Frage']) {
      expect(screen.getAllByText(text).some((el) => el.tagName === 'SPAN'), text).toBe(true);
    }
    const bar = screen.getByRole('progressbar', { name: 'Erledigte Schritte' });
    expect(bar.getAttribute('aria-valuenow')).toBe('1');
    expect(bar.getAttribute('aria-valuemax')).toBe('2');
    expect((bar.firstElementChild as HTMLElement).style.width).toBe('50%');
  });

  it('Verbindungszustände haben Text', async () => {
    await show('/p/p1/board', 'Eins');
    const source = FakeEventSource.current();
    act(() => source.emit({ type: 'hello' }));
    expect(await screen.findByText('verbunden')).toBeTruthy();
    act(() => source.fail());
    expect(await screen.findByText('getrennt')).toBeTruthy();
  });
});

describe('Begriffe der Fragen (Glossar)', () => {
  // Verbotene Begriffe ohne Treffer in Klassen oder Bezeichnern wie `card-blocker`
  const FORBIDDEN = /(?<![\w-])r(ü|ue)ckfrage|(?<![\w-])(issues?|blockers?)(?![\w-])/i;

  // Quelltexte der Fragen-Oberfläche samt genutzter Hilfsdateien (Vite liest sie als Text)
  const sources = (): Array<[string, string]> => {
    const all = import.meta.glob(
      ['../src/components/*.tsx', '../src/components/db/*.tsx', '../src/pages/*.tsx', '../src/question/*.ts', '../src/api/questionTypes.ts'],
      { query: '?raw', import: 'default', eager: true },
    ) as Record<string, string>;
    const wanted = new RegExp(
      '/(components/(Question[^/]*|Option[^/]*|Research[^/]*|Decision[^/]*|QuickCapture|CopyCommand|EmptyState|ErrorState)\\.tsx' +
        '|components/db/(DbQuestion[^/]*|DbAskQuestionDialog|DbTicketQuestions)\\.tsx|pages/DbQuestion[^/]*\\.tsx|question/[^/]*\\.ts|api/questionTypes\\.ts)$',
    );
    return Object.entries(all).filter(([path]) => wanted.test(path));
  };

  it('verwendet in sichtbaren Texten keinen verbotenen Begriff', () => {
    const found = sources();
    expect(found.length).toBeGreaterThan(15);
    for (const [file, text] of found) {
      // Kommentare bleiben außer Acht, sie sind nicht sichtbar
      const code = text.replace(/\/\*[\s\S]*?\*\//g, '').replace(/^\s*\/\/.*$/gm, '').replace(/\s\/\/\s.*$/gm, '');
      expect(code, file).not.toMatch(FORBIDDEN);
    }
  });
});
