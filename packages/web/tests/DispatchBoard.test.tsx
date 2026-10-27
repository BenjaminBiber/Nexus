// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ApiProvider } from '../src/api/ApiContext.js';
import { MemoryRouter } from 'react-router-dom';
import { DispatchDialog } from '../src/components/db/DispatchDialog.js';
import { DbCard } from '../src/components/db/DbCard.js';
import { DispatchIndicator } from '../src/components/db/DispatchIndicator.js';
import { DbMergeRetry } from '../src/components/db/DbMergeRetry.js';
import { DbBoardPage } from '../src/pages/DbBoardPage.js';
import { FakeEventSource } from './helpers/FakeEventSource.js';
import { dbBoards, dbCard, dbDetail, dbView, dispatchStatus, fakeApi } from './helpers/fakeApi.js';
import { renderWithApp } from './helpers/renderWithApp.js';

beforeEach(() => FakeEventSource.reset());
afterEach(cleanup);

const RUNNING = dispatchStatus({
  enabled: true, state: 'running', message: 'läuft · 1/1', active: 1,
  runs: [{ id: 'ARC-0001-20261015-090000', kind: 'work', ticketId: 'ARC-0001', title: 'Login', profile: 'standard', model: 'opus', effort: 'high', startedAt: '2026-10-15T07:00:00.000Z', log: 'ARC-0001-20261015-090000.log' }],
  merges: [{ ticketId: 'ARC-0003', phase: 'waiting', branch: 'ticket/ARC-0003-x' }],
  tickets: [{ id: 'ARC-0002', title: 'Ohne Plan', reason: 'Kein Plan' }],
  lastErrors: ['ARC-0004: claude fehlt'],
});

describe('Dispatcher am Board', () => {
  it('zeigt den Zustand in der Kopfzeile und die Übersicht mit Steuerung und Protokoll', async () => {
    const run = vi.fn(async () => RUNNING);
    const pause = vi.fn(async () => ({ ...RUNNING, state: 'paused' as const, message: 'pausiert bis 11:00' }));
    const api = fakeApi({
      getDbBoardView: async () => dbView([dbCard('ARC-0001', 'Login', 'in_progress', { assignee: 'junior-ARC-0001' })]),
      getDispatchStatus: vi.fn(async () => RUNNING), runDispatch: run, pauseDispatch: pause,
      getDispatchLog: async () => 'Protokollinhalt',
    });
    renderWithApp(<DbBoardPage />, { api, path: '/p/p1/board', route: '/p/:pid/board/*' });
    await userEvent.click(await screen.findByRole('button', { name: 'Dispatcher: läuft · 1/1' }));
    const dialog = await screen.findByRole('dialog', { name: 'Dispatcher' });
    expect(within(dialog).getByText('ARC-0001')).toBeTruthy();
    expect(within(dialog).getByText('Umsetzung')).toBeTruthy();
    expect(within(dialog).getByTitle('Profil standard').textContent).toBe('opus/high');
    expect(within(dialog).getByText('wartet auf weitere Tickets')).toBeTruthy();
    expect(within(dialog).getByText('Kein Plan')).toBeTruthy();
    expect(within(dialog).getByText('ARC-0004: claude fehlt')).toBeTruthy();
    await userEvent.click(within(dialog).getByRole('button', { name: 'Jetzt prüfen' }));
    expect(run).toHaveBeenCalledWith(1);
    await userEvent.click(within(dialog).getByRole('button', { name: 'Pausieren (60 min)' }));
    expect(pause).toHaveBeenCalledWith(60, 1);
    expect(await within(dialog).findByText('pausiert bis 11:00')).toBeTruthy();
    await userEvent.click(within(dialog).getByRole('button', { name: 'Protokoll ARC-0001-20261015-090000.log' }));
    expect((await screen.findByRole('dialog', { name: 'Protokoll ARC-0001-20261015-090000.log' })).textContent).toContain('Protokollinhalt');
  });

  it('zeigt Recherche-Läufe mit Link zur Frage und Protokoll', async () => {
    const status = dispatchStatus({
      research: { auto: true, wip: 1, active: 1 },
      runs: [{ id: 'ARC-F0007-20261015-090000-research', kind: 'research', ticketId: 'ARC-F0007', questionId: 'ARC-F0007', title: 'Brutto?', profile: 'standard', model: null, effort: null, startedAt: '2026-10-15T07:00:00.000Z', log: 'ARC-F0007-20261015-090000-research.log' }],
    });
    render(
      <ApiProvider api={fakeApi({ getDispatchLog: async () => 'Recherche-Protokoll' })}>
        <MemoryRouter>
          <DispatchDialog boardId={1} status={status} error={null} pid="p1" onClose={() => undefined} onStatus={() => undefined} onReload={() => undefined} onOpenTicket={() => undefined} onChanged={() => undefined} />
        </MemoryRouter>
      </ApiProvider>,
    );
    const dialog = screen.getByRole('dialog', { name: 'Dispatcher' });
    expect(within(dialog).getByRole('link', { name: 'ARC-F0007' }).getAttribute('href')).toBe('/p/p1/questions/ARC-F0007');
    expect(within(dialog).getByText('Recherche')).toBeTruthy();
    expect(within(dialog).getByTitle('Profil standard').textContent).toBe('Standard/Standard');
    expect(within(dialog).getByText('Recherchen').textContent).toBe('Recherchen 1/1');
    expect(within(dialog).getByText('Automatik').textContent).toBe('Automatik an');
    await userEvent.click(within(dialog).getByRole('button', { name: 'Protokoll ARC-F0007-20261015-090000-research.log' }));
    expect((await screen.findByRole('dialog', { name: 'Protokoll ARC-F0007-20261015-090000-research.log' })).textContent).toContain('Recherche-Protokoll');
    cleanup();
    const failing = dispatchStatus({ research: { auto: true, wip: 1, active: 0, maxPerDay: 10, today: 2, retryAt: null, error: 'Git Bash nicht gefunden' } });
    render(
      <ApiProvider api={fakeApi()}>
        <MemoryRouter>
          <DispatchDialog boardId={1} status={failing} error={null} pid="p1" onClose={() => undefined} onStatus={() => undefined} onReload={() => undefined} onOpenTicket={() => undefined} onChanged={() => undefined} />
        </MemoryRouter>
      </ApiProvider>,
    );
    const again = screen.getByRole('dialog', { name: 'Dispatcher' });
    expect(within(again).getByText('Recherchen').textContent).toBe('Recherchen 0/1');
    expect(within(again).getByText('Heute automatisch').textContent).toBe('Heute automatisch 2/10');
    expect(within(again).getByText('Recherche-Automatik: Git Bash nicht gefunden')).toBeTruthy();
  });

  it('ordnet Geplant nach Abhängigkeiten in Schritte und schließt beim Klick auf den Hintergrund', async () => {
    const onClose = vi.fn();
    const status = dispatchStatus({
      runs: [{ ...RUNNING.runs[0]! }],
      tickets: [
        { id: 'ARC-0011', title: 'Danach', reason: 'Wartet auf ARC-0010' },
        { id: 'ARC-0010', title: 'Zuerst', reason: 'bereit' },
        { id: 'ARC-0012', title: 'Zuletzt', reason: 'wartet auf Merge von ARC-0011, ARC-0001' },
      ],
    });
    render(
      <ApiProvider api={fakeApi()}>
        <MemoryRouter>
          <DispatchDialog boardId={1} status={status} error={null} pid="p1" onClose={onClose} onStatus={() => undefined} onReload={() => undefined} onOpenTicket={() => undefined} onChanged={() => undefined} />
        </MemoryRouter>
      </ApiProvider>,
    );
    const dialog = screen.getByRole('dialog', { name: 'Dispatcher' });
    expect(within(dialog).getByText('3 Tickets in 3 Schritten')).toBeTruthy();
    expect(within(dialog).getByRole('button', { name: /Schritt 01 als Nächstes/ }).getAttribute('aria-expanded')).toBe('true');
    const third = within(dialog).getByRole('button', { name: /Schritt 03/ });
    expect(third.textContent).toContain('Zuletzt');
    expect(within(dialog).getByText('gibt 1 frei →')).toBeTruthy();
    await userEvent.click(within(dialog).getByText('Danach'));
    expect(onClose).not.toHaveBeenCalled();
    await userEvent.click(dialog.parentElement!);
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('nennt bei Merges den Zielbranch nach dem Branch; ohne Zielbranch nichts zusätzlich', () => {
    const status = dispatchStatus({
      merges: [
        { ticketId: 'ARC-0003', phase: 'pending', branch: 'ticket/ARC-0003-x', target: 'release/1' },
        { ticketId: 'ARC-0004', phase: 'waiting', branch: 'ticket/ARC-0004-y', target: null },
      ],
    });
    render(
      <ApiProvider api={fakeApi()}>
        <MemoryRouter>
          <DispatchDialog boardId={1} status={status} error={null} pid="p1" onClose={() => undefined} onStatus={() => undefined} onReload={() => undefined} onOpenTicket={() => undefined} onChanged={() => undefined} />
        </MemoryRouter>
      </ApiProvider>,
    );
    const dialog = screen.getByRole('dialog', { name: 'Dispatcher' });
    expect(within(dialog).getByText('ticket/ARC-0003-x → release/1')).toBeTruthy();
    expect(within(dialog).getByText('ticket/ARC-0004-y')).toBeTruthy();
  });

  it('Indikator zeigt „Recherche läuft“, wenn nur Recherchen laufen', () => {
    const only = dispatchStatus({ state: 'off', message: 'Dispatcher ist ausgeschaltet', active: 0, research: { auto: false, wip: 2, active: 2 } });
    const { rerender } = render(<DispatchIndicator status={only} error={null} onOpen={() => undefined} />);
    const button = screen.getByRole('button', { name: 'Dispatcher: Recherche läuft (2)' });
    expect(button.className).toContain('state-running');
    rerender(<DispatchIndicator status={{ ...only, research: { auto: false, wip: 2, active: 1 } }} error={null} onOpen={() => undefined} />);
    expect(screen.getByRole('button', { name: 'Dispatcher: Recherche läuft' })).toBeTruthy();
    rerender(<DispatchIndicator status={{ ...only, state: 'paused', message: 'pausiert bis 11:00' }} error={null} onOpen={() => undefined} />);
    expect(screen.getByRole('button', { name: 'Dispatcher: pausiert bis 11:00' })).toBeTruthy();
    rerender(<DispatchIndicator status={{ ...only, state: 'running', message: 'läuft · 1/1', active: 1 }} error={null} onOpen={() => undefined} />);
    expect(screen.getByRole('button', { name: 'Dispatcher: läuft · 1/1' })).toBeTruthy();
  });

  it('kennzeichnet Agent-Tickets auf der Karte', () => {
    render(<DbCard card={dbCard('ARC-0001', 'Login', 'in_progress', { assignee: 'junior-ARC-0001' })} agentPrefix="junior-" onOpen={() => undefined} onMove={() => undefined} />);
    expect(screen.getByText('Agent')).toBeTruthy();
  });

  it('zeigt bei laufendem Merge die Pille „Merge“, kompakt das animierte Merge-Symbol', () => {
    const card = dbCard('ARC-0001', 'Login', 'done', { assignee: 'junior-ARC-0001' });
    const { rerender } = render(<DbCard card={card} merge="session" onOpen={() => undefined} onMove={() => undefined} />);
    expect(screen.getByText('Merge').getAttribute('title')).toBe('Merge-Session läuft');
    rerender(<DbCard card={card} merge="session" compact onOpen={() => undefined} onMove={() => undefined} />);
    expect(screen.queryByText('Merge')).toBeNull();
    expect(screen.getByRole('img', { name: 'Merge' }).querySelector('.git-merge-icon')).toBeTruthy();
    rerender(<DbCard card={card} onOpen={() => undefined} onMove={() => undefined} />);
    expect(screen.queryByText('Merge')).toBeNull();
  });

  it('zeigt in der kompakten Ansicht statt der Pille „Agent“ nur das Claude-Logo', () => {
    render(<DbCard card={dbCard('ARC-0001', 'Login', 'in_progress', { assignee: 'junior-ARC-0001' })} agentPrefix="junior-" compact onOpen={() => undefined} onMove={() => undefined} />);
    expect(screen.queryByText('Agent')).toBeNull();
    const logo = screen.getByRole('img', { name: 'Agent' });
    expect(logo.getAttribute('title')).toBe('Dispatcher: junior-ARC-0001');
    expect(logo.classList.contains('active')).toBe(false);
  });

  it('lässt kompakt das Claude-Logo pulsieren, solange ein Agent-Lauf am Ticket arbeitet', () => {
    render(<DbCard card={dbCard('ARC-0001', 'Login', 'in_progress')} agentPrefix="junior-" compact active onOpen={() => undefined} onMove={() => undefined} />);
    expect(screen.getByRole('img', { name: 'Agent arbeitet' }).classList.contains('active')).toBe(true);
  });

  it('„Erneut mergen“ nur bei Blockiert mit Grund „Merge:“', async () => {
    const retry = vi.fn(async () => ({ ...dispatchStatus(), result: { outcome: 'blocked' as const, text: 'ARC-0003: Merge blockiert – lokale Änderungen im Hauptordner: a.txt' } }));
    const done = vi.fn();
    const blocked = dbDetail(dbCard('ARC-0003', 'X', 'blocked', { blockedReason: 'Merge: lokale Änderungen im Hauptordner: a.txt', boardId: 1 }));
    const { rerender } = render(<ApiProvider api={fakeApi({ retryDispatchMerge: retry })}><DbMergeRetry ticket={blocked} mine onDone={done} /></ApiProvider>);
    expect(screen.getByText('Der Dispatcher konnte nicht mergen: lokale Änderungen im Hauptordner: a.txt. Ursache beheben, dann erneut versuchen.')).toBeTruthy();
    await userEvent.click(screen.getByRole('button', { name: 'Erneut mergen' }));
    expect(retry).toHaveBeenCalledWith('ARC-0003', 1);
    expect(done).toHaveBeenCalled();
    expect(screen.getByRole('status').textContent).toContain('ARC-0003: Merge blockiert – lokale Änderungen im Hauptordner: a.txt');
    rerender(<ApiProvider api={fakeApi()}><DbMergeRetry ticket={{ ...blocked, blockedReason: 'DB fehlt' }} mine onDone={done} /></ApiProvider>);
    expect(screen.queryByRole('button', { name: 'Erneut mergen' })).toBeNull();
  });

  it('„Abnehmen“ bei Human Interaction ohne Branch oder auf fremden Boards; danach Ergebnis und Neuladen', async () => {
    const accept = vi.fn(async () => ({ ...dispatchStatus(), result: { outcome: 'merged' as const, text: 'ARC-0005 gemergt in main (abc1234).' } }));
    const retry = vi.fn();
    const done = vi.fn();
    const look = { ...dbDetail(dbCard('ARC-0005', 'Z', 'blocked', { blockedReason: 'Human Interaction: Oberfläche ansehen', boardId: 1 })), branch: 'ticket/ARC-0005-z' };
    const { rerender } = render(<ApiProvider api={fakeApi({ acceptDispatchTicket: accept, retryDispatchMerge: retry })}><DbMergeRetry ticket={{ ...look, branch: null }} mine onDone={done} /></ApiProvider>);
    expect(screen.getByText(/Wartet auf deine Abnahme: Oberfläche ansehen/)).toBeTruthy();
    expect(screen.queryByRole('button', { name: 'Mergen' })).toBeNull();
    await userEvent.click(screen.getByRole('button', { name: 'Abnehmen' }));
    expect(accept).toHaveBeenCalledWith('ARC-0005', 1);
    expect(retry).not.toHaveBeenCalled();
    expect(done).toHaveBeenCalled();
    expect(screen.getByRole('status').textContent).toContain('ARC-0005 gemergt in main (abc1234).');
    rerender(<ApiProvider api={fakeApi()}><DbMergeRetry ticket={look} mine={false} onDone={done} /></ApiProvider>);
    expect(screen.getByRole('button', { name: 'Abnehmen' })).toBeTruthy();
    expect(screen.getByText(/gemergt wird auf dem Board des Besitzers/)).toBeTruthy();
    // Eigenes Board mit Branch: das übernimmt der Review-Block (DbReview)
    rerender(<ApiProvider api={fakeApi()}><DbMergeRetry ticket={look} mine onDone={done} /></ApiProvider>);
    expect(screen.queryByRole('button', { name: 'Abnehmen' })).toBeNull();
    rerender(<ApiProvider api={fakeApi()}><DbMergeRetry ticket={{ ...look, blockedReason: 'Zugang fehlt' }} mine onDone={done} /></ApiProvider>);
    expect(screen.queryByRole('button', { name: 'Abnehmen' })).toBeNull();
  });

  it('„Mergen“ bei Erledigt mit noch gesetztem Branch', async () => {
    const retry = vi.fn(async () => dispatchStatus());
    const done = vi.fn();
    const finished = { ...dbDetail(dbCard('ARC-0004', 'Y', 'done', { boardId: 1 })), branch: 'ticket/ARC-0004-y' };
    const { rerender } = render(<ApiProvider api={fakeApi({ retryDispatchMerge: retry })}><DbMergeRetry ticket={finished} mine onDone={done} /></ApiProvider>);
    expect(screen.getByText('Der Branch ticket/ARC-0004-y ist noch nicht gemergt.')).toBeTruthy();
    expect(screen.queryByRole('button', { name: 'Erneut mergen' })).toBeNull();
    await userEvent.click(screen.getByRole('button', { name: 'Mergen' }));
    expect(retry).toHaveBeenCalledWith('ARC-0004', 1);
    expect(done).toHaveBeenCalled();
    rerender(<ApiProvider api={fakeApi()}><DbMergeRetry ticket={{ ...finished, branch: null }} mine onDone={done} /></ApiProvider>);
    expect(screen.queryByRole('button', { name: 'Mergen' })).toBeNull();
  });

  it('zeigt das Ergebnis „gemergt“ auch nach dem Neuladen ohne Branch; Ausblenden per Knopf', async () => {
    const retry = vi.fn(async () => ({ ...dispatchStatus(), result: { outcome: 'merged' as const, text: 'ARC-0004 gemergt in main (abc1234).' } }));
    const finished = { ...dbDetail(dbCard('ARC-0004', 'Y', 'done', { boardId: 1 })), branch: 'ticket/ARC-0004-y' };
    const { rerender } = render(<ApiProvider api={fakeApi({ retryDispatchMerge: retry })}><DbMergeRetry ticket={finished} mine onDone={() => undefined} /></ApiProvider>);
    await userEvent.click(screen.getByRole('button', { name: 'Mergen' }));
    rerender(<ApiProvider api={fakeApi()}><DbMergeRetry ticket={{ ...finished, branch: null }} mine onDone={() => undefined} /></ApiProvider>);
    expect(screen.getByRole('status').textContent).toContain('ARC-0004 gemergt in main (abc1234).');
    expect(screen.queryByRole('button', { name: 'Mergen' })).toBeNull();
    await userEvent.click(screen.getByRole('button', { name: 'Ausblenden' }));
    expect(screen.queryByRole('status')).toBeNull();
  });

  it('blendet das Ergebnis nach 4 s aus', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      const retry = vi.fn(async () => ({ ...dispatchStatus(), result: { outcome: 'merged' as const, text: 'ARC-0004 gemergt in main (abc1234).' } }));
      const finished = { ...dbDetail(dbCard('ARC-0004', 'Y', 'done', { boardId: 1 })), branch: 'ticket/ARC-0004-y' };
      render(<ApiProvider api={fakeApi({ retryDispatchMerge: retry })}><DbMergeRetry ticket={finished} mine onDone={() => undefined} /></ApiProvider>);
      await userEvent.click(screen.getByRole('button', { name: 'Mergen' }));
      expect(screen.getByRole('status')).toBeTruthy();
      await act(async () => { vi.advanceTimersByTime(4000); });
      expect(screen.queryByRole('status')).toBeNull();
    } finally {
      vi.useRealTimers();
    }
  });

  it('zeigt „Mergen“ im Panel nur auf eigenen Boards', async () => {
    const finished = { ...dbDetail(dbCard('ARC-0004', 'Y', 'done', { boardId: 1 })), branch: 'ticket/ARC-0004-y' };
    const view = async () => dbView([dbCard('ARC-0004', 'Y', 'done', { boardId: 1 })]);
    renderWithApp(<DbBoardPage />, { api: fakeApi({ getDbBoardView: view, getDbTicket: async () => finished }), path: '/p/p1/board/t/ARC-0004', route: '/p/:pid/board/*' });
    expect(await screen.findByRole('button', { name: 'Mergen' })).toBeTruthy();
    cleanup();

    const foreign = dbBoards.map((b) => (b.id === 1 ? { ...b, ownerId: 'u2', ownerName: 'Kollege' } : b));
    renderWithApp(<DbBoardPage />, {
      api: fakeApi({ getDbBoardView: view, getDbTicket: async () => finished, getDbBoards: async () => foreign }),
      path: '/p/p1/board/t/ARC-0004', route: '/p/:pid/board/*',
    });
    expect(await screen.findByText(/Der Branch ticket\/ARC-0004-y ist noch nicht gemergt\. Mergen kann nur der Besitzer des Boards\./)).toBeTruthy();
    expect(screen.queryByRole('button', { name: 'Mergen' })).toBeNull();
  });
});

describe('Dispatcher-Abruffehler', () => {
  const view = async () => dbView([dbCard('ARC-0001', 'Login', 'in_progress', {})]);

  it('zeigt den Fehler im Knopf und im Dialog mit „Erneut laden“', async () => {
    const get = vi.fn(async () => { throw new Error('Server weg'); });
    renderWithApp(<DbBoardPage />, { api: fakeApi({ getDbBoardView: view, getDispatchStatus: get }), path: '/p/p1/board', route: '/p/:pid/board/*' });
    await userEvent.click(await screen.findByRole('button', { name: 'Dispatcher: Fehler – Server weg' }));
    const dialog = await screen.findByRole('dialog', { name: 'Dispatcher' });
    expect(within(dialog).getByRole('alert').textContent).toContain('Server weg');
    const before = get.mock.calls.length;
    await userEvent.click(within(dialog).getByRole('button', { name: 'Erneut laden' }));
    expect(get.mock.calls.length).toBeGreaterThan(before);
  });

  it('weist bei vorhandenem Stand auf Veraltung hin', () => {
    render(<ApiProvider api={fakeApi()}><MemoryRouter><DispatchDialog boardId={1} status={RUNNING} error="Timeout" pid="p1" onClose={() => undefined} onStatus={() => undefined} onReload={() => undefined} onOpenTicket={() => undefined} onChanged={() => undefined} /></MemoryRouter></ApiProvider>);
    expect(screen.getByText('Stand evtl. veraltet: Timeout')).toBeTruthy();
  });
});
