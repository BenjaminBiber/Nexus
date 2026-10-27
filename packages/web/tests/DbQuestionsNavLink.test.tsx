// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { ApiProvider } from '../src/api/ApiContext.js';
import { ApiError } from '../src/api/ApiError.js';
import { DbQuestionsNavLink } from '../src/components/DbQuestionsNavLink.js';
import { notifyDbActiveChanged } from '../src/dbquestion/activeBoard.js';
import { LiveProvider } from '../src/live/LiveProvider.js';
import { FakeEventSource } from './helpers/FakeEventSource.js';
import { dbActive, dbQuestion, dbQuestionList, fakeApi } from './helpers/fakeApi.js';

beforeEach(() => FakeEventSource.reset());
afterEach(cleanup);

const THREE = dbQuestionList([
  dbQuestion('ARC-F0001', 'Eins'),
  dbQuestion('ARC-F0002', 'Zwei'),
  dbQuestion('ARC-F0003', 'Drei'),
  dbQuestion('ARC-F0004', 'Erledigt', { state: 'answered' }),
  dbQuestion('ARC-F0005', 'Betrifft mich nicht', { mine: false }),
]);

// Rendert nur den Navigationseintrag mit Live-Anbieter
function open(overrides: Parameters<typeof fakeApi>[0] = {}) {
  const api = fakeApi({ getDbQuestions: vi.fn(async () => THREE), openEvents: vi.fn(() => new FakeEventSource()), ...overrides });
  render(
    <ApiProvider api={api}>
      <LiveProvider pid="p1">
        <MemoryRouter initialEntries={['/p/p1/board']}>
          <nav>
            <DbQuestionsNavLink pid="p1" />
          </nav>
        </MemoryRouter>
      </LiveProvider>
    </ApiProvider>,
  );
  return api;
}

describe('Reiter „Fragen“ mit Zähler offener eigener DB-Fragen', () => {
  it('zeigt die Zahl offener eigener Fragen des Projekts des aktiven Boards (andere offene zählen nicht)', async () => {
    const api = open();
    const link = await screen.findByRole('link', { name: 'Fragen, 3 eigene offen' });
    expect(link.getAttribute('href')).toBe('/p/p1/questions');
    expect(link.querySelector('.nav-badge')?.textContent).toBe('3');
    expect(api.getDbQuestions).toHaveBeenCalledWith('ARC');
  });

  it('blendet den Zähler aus, wenn nur Fragen offen sind, die den Benutzer nicht betreffen', async () => {
    const api = open({ getDbQuestions: vi.fn(async () => dbQuestionList([dbQuestion('ARC-F0005', 'Fremd', { mine: false })])) });
    await waitFor(() => expect(api.getDbQuestions).toHaveBeenCalled());
    expect(screen.getByRole('link', { name: 'Fragen' }).querySelector('.nav-badge')).toBeNull();
  });

  it('blendet den Zähler bei 0 offenen Fragen aus', async () => {
    const api = open({ getDbQuestions: vi.fn(async () => dbQuestionList([dbQuestion('ARC-F0004', 'Erledigt', { state: 'answered' })])) });
    await waitFor(() => expect(api.getDbQuestions).toHaveBeenCalled());
    const link = screen.getByRole('link', { name: 'Fragen' });
    expect(link.querySelector('.nav-badge')).toBeNull();
  });

  it('aktualisiert den Zähler bei einer Live-Meldung zum Projekt', async () => {
    const getDbQuestions = vi.fn(async () => THREE);
    const api = open({ getDbQuestions });
    await screen.findByRole('link', { name: 'Fragen, 3 eigene offen' });
    await waitFor(() => expect(api.openEvents).toHaveBeenLastCalledWith('p1', null, 1));
    getDbQuestions.mockResolvedValue(dbQuestionList([dbQuestion('ARC-F0001', 'Eins')]));
    act(() => FakeEventSource.current().emit({ type: 'dbquestions', projectId: 1 }));
    expect(await screen.findByRole('link', { name: 'Fragen, 1 eigene offen' })).toBeTruthy();
  });

  it('folgt einem Wechsel des aktiven Boards', async () => {
    const getDbActive = vi.fn(async () => dbActive(1));
    const getDbQuestions = vi.fn(async (key: string) => (key === 'OTH' ? dbQuestionList([dbQuestion('OTH-F0001', 'Fremd')]) : THREE));
    open({ getDbActive, getDbQuestions });
    await screen.findByRole('link', { name: 'Fragen, 3 eigene offen' });
    getDbActive.mockResolvedValue(dbActive(3));
    act(() => notifyDbActiveChanged());
    expect(await screen.findByRole('link', { name: 'Fragen, 1 eigene offen' })).toBeTruthy();
    expect(getDbQuestions).toHaveBeenLastCalledWith('OTH');
  });

  it('blendet den Zähler aus, wenn die Board-Datenbank nicht verfügbar ist', async () => {
    const getDbActive = vi.fn(async () => { throw new ApiError('board-db-unreachable', 'Board-Datenbank ist nicht erreichbar', 503); });
    open({ getDbActive });
    await waitFor(() => expect(getDbActive).toHaveBeenCalled());
    const link = screen.getByRole('link', { name: 'Fragen' });
    expect(link.querySelector('.nav-badge')).toBeNull();
  });

  it('blendet den Zähler aus, wenn die Fragen mit 503 scheitern', async () => {
    const getDbQuestions = vi.fn(async () => { throw new ApiError('board-db-unreachable', 'Board-Datenbank ist nicht erreichbar', 503); });
    open({ getDbQuestions });
    await waitFor(() => expect(getDbQuestions).toHaveBeenCalled());
    await waitFor(() => expect(screen.getByRole('link', { name: 'Fragen' }).querySelector('.nav-badge')).toBeNull());
  });
});
