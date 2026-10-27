// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { ApiProvider } from '../src/api/ApiContext.js';
import type { DbTicketTimelineDto } from '../src/api/dbTypes.js';
import { DbTicketTimeline } from '../src/components/db/DbTicketTimeline.js';
import { fakeApi } from './helpers/fakeApi.js';

afterEach(cleanup);
beforeEach(() => localStorage.clear());

const TIMELINE: DbTicketTimelineDto = {
  ticketId: 'ARC-0001',
  events: [
    { at: '2026-10-13T10:00:00.000Z', kind: 'status', type: 'field:status', actor: { name: 'Florian', type: 'person' }, field: 'status', from: null, to: 'backlog', note: 'angelegt' },
    { at: '2026-10-13T11:00:00.000Z', kind: 'run', type: 'dispatch:start', actor: { name: 'dispatcher', type: 'agent' }, text: '[dispatch] Start: an junior-ARC-0001 übergeben', ref: { profile: 'standard' } },
    { at: '2026-10-13T12:00:00.000Z', kind: 'question', type: 'question:decided', actor: { name: 'Florian', type: 'person' }, text: 'ARC-F0001 „Speicher?“: SQL', ref: { questionId: 'ARC-F0001' } },
    { at: '2026-10-13T13:00:00.000Z', kind: 'git', type: 'commit', actor: { name: 'Tester', type: 'person' }, text: 'Login-Formular', ref: { commit: 'abcdef1234567890' } },
    { at: '2026-10-13T14:00:00.000Z', kind: 'comment', type: 'comment', actor: { name: 'junior-ARC-0001', type: 'agent' }, text: 'Schritt **2** fertig' },
  ],
  sources: { runs: 'ok', git: 'ok' },
};

function renderTimeline(getDbTicketTimeline = vi.fn(async () => TIMELINE), rev = 1, composer?: { onOpen: () => void; busy: boolean }) {
  const api = fakeApi({ getDbTicketTimeline });
  const view = (r: number) => (
    <ApiProvider api={api}>
      <MemoryRouter initialEntries={['/p/p1/board/t/ARC-0001']}>
        <Routes>
          <Route path="/p/:pid/board/t/:id" element={<DbTicketTimeline ticketId="ARC-0001" rev={r} composer={composer} />} />
          <Route path="/p/:pid/questions/:qid" element={<p>Fragenseite</p>} />
        </Routes>
      </MemoryRouter>
    </ApiProvider>
  );
  const result = render(view(rev));
  return { getDbTicketTimeline, rerender: (r: number) => result.rerender(view(r)) };
}

// Texte der Einträge in angezeigter Reihenfolge
function entries(): string[] {
  return screen.queryAllByRole('listitem').map((li) => li.textContent ?? '');
}

describe('DbTicketTimeline', () => {
  it('zeigt Ereignisse neueste zuerst nach Tag gruppiert mit Uhrzeit, Person oder Agent und Hervorhebung', async () => {
    renderTimeline();
    expect(await screen.findByText('Login-Formular', { exact: false })).toBeTruthy();
    const list = entries();
    expect(list).toHaveLength(5);
    expect(screen.getByRole('region', { name: /13. Oktober/ })).toBeTruthy();
    expect(list[0]).toContain('junior-ARC-0001:');
    expect(list[0]).toContain('Agent');
    expect(list[4]).toContain('Angelegt in Backlog');
    expect(list[4]).toContain('Florian · Person');
    expect(list[3]).toContain('Lauf gestartet – Profil standard');
    expect(list[1]).toContain('Login-Formular');
    expect(list[1]).toContain('Commit abcdef123');
    expect(screen.getByText('2', { selector: 'strong' })).toBeTruthy();
    const items = screen.getAllByRole('listitem');
    expect(items[3].querySelector('.tl-body.highlight')).toBeTruthy();
    expect(items[4].querySelector('.tl-body.highlight')).toBeNull();
    expect(items[2].querySelector('.tl-dot.tone-ok')).toBeTruthy();
  });

  it('filtert nach Art, dreht die Reihenfolge und merkt sich beides', async () => {
    renderTimeline();
    await screen.findByText('Login-Formular', { exact: false });
    await userEvent.click(screen.getByRole('button', { name: 'Kommentare (1)' }));
    expect(screen.getByRole('button', { name: 'Kommentare (1)' }).getAttribute('aria-pressed')).toBe('false');
    expect(entries()).toHaveLength(4);
    await userEvent.click(screen.getByRole('button', { name: /Reihenfolge: neueste zuerst/ }));
    expect(entries()[0]).toContain('Angelegt in Backlog');
    cleanup();
    renderTimeline();
    await screen.findByText('Login-Formular', { exact: false });
    expect(entries()).toHaveLength(4);
    expect(entries()[0]).toContain('Angelegt in Backlog');
  });

  it('verlinkt Fragen', async () => {
    renderTimeline();
    await userEvent.click(await screen.findByRole('link', { name: 'Frage ARC-F0001 ansehen' }));
    expect(screen.getByText('Fragenseite')).toBeTruthy();
  });

  it('weist auf nicht lesbare lokale Quellen hin', async () => {
    renderTimeline(vi.fn(async () => ({ ...TIMELINE, sources: { runs: 'unavailable' as const, git: 'unavailable' as const, gitMessage: 'Kein Repo-Pfad auf diesem Rechner.' } })));
    expect(await screen.findByText(/Lauf-Protokolle gibt es nur auf dem Rechner des Dispatchers/)).toBeTruthy();
    expect(screen.getByText(/Kein Repo-Pfad auf diesem Rechner/)).toBeTruthy();
  });

  it('lädt bei neuer Version neu und meldet Fehler', async () => {
    const { getDbTicketTimeline, rerender } = renderTimeline();
    await screen.findByText('Login-Formular', { exact: false });
    rerender(2);
    await vi.waitFor(() => expect(getDbTicketTimeline).toHaveBeenCalledTimes(2));
    cleanup();
    renderTimeline(vi.fn(async () => { throw new Error('kaputt'); }));
    expect(within(await screen.findByRole('alert')).getByText(/kaputt/)).toBeTruthy();
  });
});
describe('DbTicketTimeline – Kommentarknopf', () => {
  it('zeigt ohne composer keinen Knopf und ruft sonst onOpen', async () => {
    renderTimeline();
    expect(await screen.findByRole('group', { name: 'Arten filtern' })).toBeTruthy();
    expect(screen.queryByRole('button', { name: 'Kommentar hinzufügen' })).toBeNull();
    cleanup();
    const onOpen = vi.fn();
    renderTimeline(vi.fn(async () => TIMELINE), 1, { onOpen, busy: false });
    await userEvent.click(await screen.findByRole('button', { name: 'Kommentar hinzufügen' }));
    expect(onOpen).toHaveBeenCalledTimes(1);
  });
});

