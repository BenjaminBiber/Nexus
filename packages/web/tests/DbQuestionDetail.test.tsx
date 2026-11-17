// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { ApiProvider } from '../src/api/ApiContext.js';
import { ApiError } from '../src/api/ApiError.js';
import type { DbDecisionInput, DbNewQuestionInput, DbQuestionDetailDto } from '../src/api/dbTypes.js';
import { BoardProjectProvider } from '../src/dbboard/boardProject.js';
import { LiveProvider } from '../src/live/LiveProvider.js';
import { DbQuestionsPage } from '../src/pages/DbQuestionsPage.js';
import { FakeEventSource } from './helpers/FakeEventSource.js';
import { dbQuestion, dbQuestionDetail, dbQuestionList, fakeApi } from './helpers/fakeApi.js';

vi.mock('mermaid', () => ({ default: { initialize: vi.fn(), render: vi.fn(async () => ({ svg: '<svg></svg>' })) } }));

beforeEach(() => {
  FakeEventSource.reset();
  window.localStorage.clear();
});
afterEach(cleanup);

const SUMMARY = dbQuestion('ARC-F0001', 'Welcher Speicher?', {
  rev: 3,
  description: '## Worum es geht\nDer Export braucht einen Speicher.\n\n## Zu entscheiden\nWohin mit den Daten?',
  tags: ['speicher'], originSession: 's-1', occasion: 'Planung ARC-0007',
  ticketIds: ['ARC-0007'], tickets: [{ id: 'ARC-0007', title: 'Export', status: 'backlog', boardId: 1 }],
  dependsOn: ['ARC-F0002'], waitingOn: [{ id: 'ARC-F0002', state: 'open' }], ready: false, researched: true, optionCount: 2,
});

const CONTEXT = [
  '## Heute',
  'Alles liegt in einer Datei.',
  '',
  '## Fundstellen',
  '- `src/store.ts:12` – schreibt die Datei',
  '',
  '## Weitere Frage nötig',
  'Soll der Export verschlüsselt werden? Betrifft auch das Backup.',
].join('\n');

const DETAIL: DbQuestionDetailDto = dbQuestionDetail(SUMMARY, {
  research: [{
    id: 1, questionId: 'ARC-F0001', at: '2026-10-14T10:00:00.000Z', byId: 'u1', byName: 'claude', kind: 'session', context: CONTEXT,
    options: [
      { id: 'O1', title: 'Datei', text: 'JSON-Datei', pros: ['einfach'], cons: [], recommended: false },
      { id: 'O2', title: 'DB', text: 'SQL', pros: ['geteilt'], cons: ['Schema'], recommended: true },
    ],
  }],
  history: [{ questionId: 'ARC-F0001', at: '2026-10-14T09:00:00.000Z', byId: 'u1', byName: 'claude', byKind: 'session', type: 'created', text: 'Frage angelegt' }],
  comments: [{ id: 1, questionId: 'ARC-F0001', authorId: 'u1', authorName: 'Florian', authorKind: 'human', body: 'Bitte bis Freitag', createdAt: '2026-10-14T11:00:00.000Z' }],
});

const SECOND: DbQuestionDetailDto = { ...DETAIL, id: 'ARC-F0002', number: 2, title: 'Zweite Frage', dependsOn: [], waitingOn: [], ready: true };

// Liste daneben: die gewählte Frage wartet auf die zweite
const LIST = dbQuestionList([SUMMARY, dbQuestion('ARC-F0002', 'Zweite Frage')]);

// Entscheidung zu einer Detailfassung
const decided = (base: DbQuestionDetailDto, rev: number, extra: Partial<DbQuestionDetailDto['decision'] & object> = {}): DbQuestionDetailDto => ({
  ...base, state: 'answered', rev,
  decision: { outcome: 'answered', option: 'O2', text: null, reason: 'geteilt', byId: 'u1', byName: 'Florian', at: '2026-10-14T12:00:00.000Z', ...extra },
});

// Rendert die Fragenseite mit gewählter Frage ARC-F0001
function open(overrides: Parameters<typeof fakeApi>[0] = {}, path = '/p/p1/questions/ARC-F0001') {
  const api = fakeApi({
    getDbQuestions: vi.fn(async () => LIST),
    getDbQuestion: vi.fn(async (id: string) => (id === 'ARC-F0002' ? SECOND : DETAIL)),
    openEvents: vi.fn(() => new FakeEventSource()),
    ...overrides,
  });
  render(
    <ApiProvider api={api}>
      <LiveProvider pid="p1">
        <MemoryRouter initialEntries={[path]}>
          <BoardProjectProvider>
            <Routes>
              <Route path="/p/:pid/questions/:id?" element={<DbQuestionsPage />} />
            </Routes>
          </BoardProjectProvider>
        </MemoryRouter>
      </LiveProvider>
    </ApiProvider>,
  );
  return api;
}

const loaded = (title = 'Welcher Speicher?') => screen.findByRole('heading', { name: title, level: 2 });
const card = () => screen.getByRole('region', { name: 'Entscheiden' });
const rail = () => screen.getByRole('complementary', { name: 'Bezüge und Details' });
const status = () => within(card()).getByRole('status').textContent;
const reasonField = () => screen.getByLabelText(/^Begründung/) as HTMLTextAreaElement;

// Wählt Option O2 und gibt eine Begründung ein
async function pickO2(reason = 'geteilt') {
  await userEvent.click(within(card()).getByRole('radio', { name: 'DB (empfohlen)' }));
  await userEvent.type(reasonField(), reason);
}

// Wechselt den Inhaltsreiter
async function showTab(name: RegExp | string) {
  await userEvent.click(within(screen.getByRole('tablist', { name: 'Inhalt der Frage' })).getByRole('tab', { name }));
  return screen.getByRole('tabpanel', { name });
}

describe('Fragen – offene Frage', () => {
  it('zeigt Titel, Entscheidungskarte, Bezüge und markiert die Zeile in der Liste', async () => {
    open();
    await loaded();
    expect(within(card()).getByText('Wohin mit den Daten?')).toBeTruthy();
    expect(within(card()).getByText('Der Export braucht einen Speicher.')).toBeTruthy();
    const options = within(card()).getByRole('radiogroup', { name: 'Optionen' });
    expect(within(options).getAllByRole('radio').map((r) => r.getAttribute('aria-checked'))).toEqual(['false', 'false']);
    expect(within(options).getByRole('radio', { name: 'Datei' })).toBeTruthy();
    expect(within(options).getByText('einfach')).toBeTruthy();
    expect(within(options).getByText('Schema')).toBeTruthy();
    expect(status()).toBe('Noch nichts gewählt');
    expect((screen.getByRole('button', { name: 'Entscheiden' }) as HTMLButtonElement).disabled).toBe(true);
    expect((await screen.findByRole('link', { name: 'Welcher Speicher?' })).getAttribute('aria-current')).toBe('page');

    const tickets = within(rail()).getByRole('region', { name: 'Wartende Tickets' });
    expect(within(tickets).getByRole('link', { name: /ARC-0007/ }).getAttribute('href')).toBe('/p/p1/board/t/ARC-0007');
    expect(within(tickets).getByText('wartet')).toBeTruthy();
    const deps = within(rail()).getByRole('region', { name: 'Abhängig von' });
    expect(within(deps).getByRole('link', { name: 'ARC-F0002' }).getAttribute('href')).toBe('/p/p1/questions/ARC-F0002');
    expect(within(deps).getByText('noch offen')).toBeTruthy();
    expect(within(rail()).getByText('/nexus:board-question ARC-F0001')).toBeTruthy();
    expect(within(rail()).getByText('claude · Session s-1')).toBeTruthy();
    expect(within(rail()).getByText('Planung ARC-0007')).toBeTruthy();
    const followUp = within(rail()).getByRole('region', { name: 'Folgefrage aus der Recherche' });
    expect(within(followUp).getByText('Die Recherche sieht eine weitere Frage')).toBeTruthy();
  });

  it('klappt den Stand heute aus der Recherche auf', async () => {
    open();
    await loaded();
    const toggle = within(card()).getByRole('button', { name: 'So ist es heute (Kontext der Recherche)' });
    expect(toggle.getAttribute('aria-expanded')).toBe('false');
    expect(screen.getByText('Alles liegt in einer Datei.').closest('[hidden]')).not.toBeNull();
    await userEvent.click(toggle);
    expect(toggle.getAttribute('aria-expanded')).toBe('true');
    expect(screen.getByText('Alles liegt in einer Datei.').closest('[hidden]')).toBeNull();
  });

  it('zeigt Beschreibung, Recherche mit Fundstellen und Verlauf in Reitern', async () => {
    open();
    await loaded();
    const tabs = within(screen.getByRole('tablist', { name: 'Inhalt der Frage' })).getAllByRole('tab');
    expect(tabs.map((t) => t.textContent)).toEqual(['Beschreibung', 'Recherche 1', 'Verlauf 2', 'Läufe']);
    expect(within(screen.getByRole('region', { name: 'Inhalt' })).getByRole('tabpanel').textContent).toContain('Alles Weitere steht oben in der Entscheidung.');

    const research = await showTab('Recherche 1');
    expect(within(research).getByText('neueste')).toBeTruthy();
    expect(within(research).getByText('Fundstellen im Code · 1')).toBeTruthy();
    expect(within(research).getByText('src/store.ts:12')).toBeTruthy();
    expect(within(research).getByText('schreibt die Datei')).toBeTruthy();
    // „Heute“ steht schon in der Entscheidungskarte, die Folgefrage in der rechten Leiste
    expect(within(research).queryByText('Alles liegt in einer Datei.')).toBeNull();
    expect(within(research).queryByText(/verschlüsselt/)).toBeNull();

    const history = await showTab('Verlauf 2');
    expect(within(history).getByText('Frage angelegt')).toBeTruthy();
    expect(within(history).getByText('Bitte bis Freitag')).toBeTruthy();

    tabs[2].focus();
    await userEvent.keyboard('{ArrowRight}{ArrowRight}');
    expect(document.activeElement).toBe(tabs[0]);
    expect(tabs[0].getAttribute('aria-selected')).toBe('true');
  });

  it('entscheidet mit einer Option der neuesten Recherche und Begründung', async () => {
    const decideDbQuestion = vi.fn(async (_id: string, input: DbDecisionInput) => decided(DETAIL, 4, { option: input.option ?? null, reason: input.reason }));
    const api = open({ decideDbQuestion });
    await loaded();
    await waitFor(() => expect(api.getDbQuestions).toHaveBeenCalledTimes(1));
    await pickO2();
    expect(status()).toBe('Gewählt: O2');
    await userEvent.click(screen.getByRole('button', { name: 'Entscheiden: O2' }));
    expect(decideDbQuestion).toHaveBeenCalledWith('ARC-F0001', { rev: 3, option: 'O2', reason: 'geteilt' });
    const box = await screen.findByRole('region', { name: 'Entscheidung' });
    expect(within(box).getByText('Option O2')).toBeTruthy();
    expect(within(box).getByText('DB')).toBeTruthy();
    expect(within(box).getByText('geteilt')).toBeTruthy();
    await waitFor(() => expect(api.getDbQuestions).toHaveBeenCalledTimes(2));
  });

  it('wählt mit Ziffern und entscheidet mit Strg+Enter; in Eingabefeldern zählen Ziffern als Text', async () => {
    const decideDbQuestion = vi.fn(async (_id: string, input: DbDecisionInput) => decided(DETAIL, 4, { option: input.option ?? null, reason: input.reason }));
    open({ decideDbQuestion });
    await loaded();
    await userEvent.keyboard('2');
    const db = within(card()).getByRole('radio', { name: 'DB (empfohlen)' });
    expect(db.getAttribute('aria-checked')).toBe('true');
    expect(document.activeElement).toBe(db);
    await userEvent.keyboard('{ArrowDown}');
    expect(within(card()).getByRole('radio', { name: 'Datei' }).getAttribute('aria-checked')).toBe('true');
    await userEvent.keyboard('3');
    expect(document.activeElement).toBe(screen.getByLabelText('Eigene Antwort'));
    await userEvent.click(reasonField());
    await userEvent.keyboard('2');
    expect(reasonField().value).toBe('2');
    expect(status()).toBe('Gewählt: O1');
    await userEvent.keyboard('{Control>}{Enter}{/Control}');
    expect(decideDbQuestion).toHaveBeenCalledWith('ARC-F0001', { rev: 3, option: 'O1', reason: '2' });
  });

  it('entscheidet mit eigener Antwort; eine Option ersetzt sie', async () => {
    const decideDbQuestion = vi.fn(async (_id: string, input: DbDecisionInput) => decided(DETAIL, 4, { option: null, text: input.text ?? null, reason: '' }));
    open({ decideDbQuestion });
    await loaded();
    const custom = screen.getByLabelText('Eigene Antwort') as HTMLInputElement;
    await userEvent.type(custom, 'Redis');
    expect(status()).toBe('Eigene Antwort');
    await userEvent.click(within(card()).getByRole('radio', { name: 'Datei' }));
    expect(custom.value).toBe('');
    await userEvent.type(custom, 'Redis');
    expect(within(card()).getByRole('radio', { name: 'Datei' }).getAttribute('aria-checked')).toBe('false');
    await userEvent.click(screen.getByRole('button', { name: 'Entscheiden' }));
    expect(decideDbQuestion).toHaveBeenCalledWith('ARC-F0001', { rev: 3, text: 'Redis', reason: '' });
    const box = await screen.findByRole('region', { name: 'Entscheidung' });
    expect(within(box).getByText('eigene Antwort')).toBeTruthy();
    expect(within(box).getByText('Redis')).toBeTruthy();
  });

  it('verlangt beim Verwerfen eine Begründung und öffnet wieder', async () => {
    const discardDbQuestion = vi.fn(async () => ({ ...decided(DETAIL, 4, { outcome: 'discarded', option: null, reason: 'doppelt' }), state: 'discarded' as const }));
    const reopenDbQuestion = vi.fn(async () => ({ ...DETAIL, rev: 5 }));
    open({ discardDbQuestion, reopenDbQuestion });
    await loaded();
    await userEvent.click(screen.getByRole('button', { name: 'Verwerfen' }));
    expect(await screen.findByText('Bitte eine Begründung angeben.')).toBeTruthy();
    expect(document.activeElement).toBe(reasonField());
    expect(discardDbQuestion).not.toHaveBeenCalled();
    await userEvent.type(reasonField(), 'doppelt');
    await userEvent.click(screen.getByRole('button', { name: 'Verwerfen' }));
    expect(discardDbQuestion).toHaveBeenCalledWith('ARC-F0001', 3, 'doppelt');
    const box = await screen.findByRole('region', { name: 'Entscheidung' });
    expect(within(box).getByText('Verworfen')).toBeTruthy();
    await userEvent.click(within(box).getByRole('button', { name: 'Wieder öffnen' }));
    expect(reopenDbQuestion).toHaveBeenCalledWith('ARC-F0001', 4, undefined);
    expect(await screen.findByRole('region', { name: 'Entscheiden' })).toBeTruthy();
  });

  it('springt mit „Überspringen“ zur nächsten Frage der Liste', async () => {
    open();
    await loaded();
    await waitFor(() => expect((screen.getByRole('button', { name: 'Überspringen' }) as HTMLButtonElement).disabled).toBe(false));
    await userEvent.click(screen.getByRole('button', { name: 'Überspringen' }));
    expect(await loaded('Zweite Frage')).toBeTruthy();
  });

  it('bietet bei einem Konflikt „Meine Fassung behalten“ und entscheidet dann mit der neuen Version', async () => {
    const fresh: DbQuestionDetailDto = { ...DETAIL, rev: 4, title: 'Neuer Titel' };
    const decideDbQuestion = vi.fn()
      .mockRejectedValueOnce(new ApiError('conflict', 'Frage ARC-F0001 wurde inzwischen geändert.', 409, fresh))
      .mockResolvedValueOnce(decided(fresh, 5));
    open({ decideDbQuestion });
    await loaded();
    await pickO2();
    await userEvent.click(screen.getByRole('button', { name: 'Entscheiden: O2' }));
    const dialog = await screen.findByRole('alertdialog', { name: 'Speicherkonflikt' });
    await userEvent.click(within(dialog).getByRole('button', { name: 'Meine Fassung behalten' }));
    expect(decideDbQuestion).toHaveBeenLastCalledWith('ARC-F0001', { rev: 4, option: 'O2', reason: 'geteilt' });
    expect(await screen.findByRole('region', { name: 'Entscheidung' })).toBeTruthy();
    expect(screen.queryByRole('alertdialog')).toBeNull();
  });

  it('ändert wartende Tickets und Abhängigkeiten und zeigt Kreise als Fehler', async () => {
    const putDbQuestionTickets = vi.fn(async (_id: string, rev: number, ticketIds: string[]) => ({
      ...DETAIL, rev: rev + 1, ticketIds, tickets: ticketIds.map((id) => ({ id, title: id, status: 'backlog' as const, boardId: 1 })),
    }));
    const putDbQuestionDepends = vi.fn(async () => { throw new ApiError('dependency-cycle', 'Abhängigkeiten liefen im Kreis: ARC-F0001 -> ARC-F0003 -> ARC-F0001.', 422); });
    open({ putDbQuestionTickets, putDbQuestionDepends });
    await loaded();
    const tickets = () => within(rail()).getByRole('region', { name: 'Wartende Tickets' });
    await userEvent.type(within(tickets()).getByLabelText('Ticket hinzufügen'), 'arc-0008');
    await userEvent.click(within(tickets()).getByRole('button', { name: 'Ticket eintragen' }));
    expect(putDbQuestionTickets).toHaveBeenCalledWith('ARC-F0001', 3, ['ARC-0007', 'ARC-0008']);
    await waitFor(() => expect((within(tickets()).getByLabelText('Ticket hinzufügen') as HTMLInputElement).value).toBe(''));
    await userEvent.click(await within(tickets()).findByRole('button', { name: 'ARC-0007 entfernen' }));
    expect(putDbQuestionTickets).toHaveBeenLastCalledWith('ARC-F0001', 4, ['ARC-0008']);
    const deps = within(rail()).getByRole('region', { name: 'Abhängig von' });
    await userEvent.type(within(deps).getByLabelText('Abhängigkeit hinzufügen'), 'ARC-F0003');
    await userEvent.click(within(deps).getByRole('button', { name: 'Abhängigkeit eintragen' }));
    expect(putDbQuestionDepends).toHaveBeenCalledWith('ARC-F0001', 5, ['ARC-F0002', 'ARC-F0003']);
    expect((await within(deps).findByRole('alert')).textContent).toContain('Kreis');
  });

  it('schreibt Kommentare und lädt bei einer Live-Meldung des Projekts nach', async () => {
    const addDbQuestionComment = vi.fn(async (_id: string, body: string) => ({
      ...DETAIL, rev: 4,
      comments: [...DETAIL.comments, { id: 2, questionId: 'ARC-F0001', authorId: 'u1', authorName: 'Florian', authorKind: 'human' as const, body, createdAt: '2026-10-14T12:00:00.000Z' }],
    }));
    const api = open({ addDbQuestionComment });
    await loaded();
    const history = await showTab('Verlauf 2');
    expect(history.textContent).toContain('Sessions sehen Kommentare mit nexus board question show');
    await userEvent.type(within(history).getByLabelText('Neuer Kommentar'), 'Noch eine Idee');
    await userEvent.click(within(history).getByRole('button', { name: 'Kommentar senden' }));
    expect(addDbQuestionComment).toHaveBeenCalledWith('ARC-F0001', 'Noch eine Idee');
    expect(await screen.findByText('Noch eine Idee')).toBeTruthy();
    await waitFor(() => expect(api.openEvents).toHaveBeenLastCalledWith('p1', null, 1));
    act(() => FakeEventSource.current().emit({ type: 'dbquestions', projectId: 1 }));
    await waitFor(() => expect(api.getDbQuestion).toHaveBeenCalledTimes(2));
  });

  it('setzt beim Wechsel zu einer anderen Frage Auswahl, Begründung und Eingaben zurück', async () => {
    open();
    await loaded();
    await pickO2('mein Grund');
    await userEvent.type(screen.getByLabelText('Ticket hinzufügen'), 'ARC-0009');
    await userEvent.click(within(within(rail()).getByRole('region', { name: 'Abhängig von' })).getByRole('link', { name: 'ARC-F0002' }));
    expect(await loaded('Zweite Frage')).toBeTruthy();
    expect(within(card()).getAllByRole('radio').map((r) => r.getAttribute('aria-checked'))).toEqual(['false', 'false']);
    expect(status()).toBe('Noch nichts gewählt');
    expect(reasonField().value).toBe('');
    expect((screen.getByLabelText('Ticket hinzufügen') as HTMLInputElement).value).toBe('');
    expect(screen.getByRole('link', { name: 'Zweite Frage' }).getAttribute('aria-current')).toBe('page');
  });

  it('hebt die Auswahl auf, wenn eine neue Recherche die gewählte Option nicht mehr enthält', async () => {
    const getDbQuestion = vi.fn(async () => DETAIL);
    open({ getDbQuestion });
    await loaded();
    await pickO2();
    expect(status()).toBe('Gewählt: O2');
    const newer: DbQuestionDetailDto = {
      ...DETAIL, rev: 4,
      research: [...DETAIL.research, {
        id: 2, questionId: 'ARC-F0001', at: '2026-10-15T10:00:00.000Z', byId: 'u1', byName: 'claude', kind: 'session', context: 'Neu',
        options: [{ id: 'O1', title: 'Cache', text: 'Redis', pros: [], cons: [], recommended: true }],
      }],
    };
    getDbQuestion.mockResolvedValue(newer);
    await waitFor(() => expect(FakeEventSource.current()).toBeTruthy());
    act(() => FakeEventSource.current().emit({ type: 'dbquestions', projectId: 1 }));
    expect(await within(card()).findByRole('radio', { name: 'Cache (empfohlen)' })).toBeTruthy();
    expect(status()).toBe('Noch nichts gewählt');
  });

  it('legt die Folgefrage aus der Recherche an und öffnet sie', async () => {
    const createDbQuestion = vi.fn(async (_key: string, input: DbNewQuestionInput) => dbQuestionDetail(dbQuestion('ARC-F0005', input.title)));
    open({ createDbQuestion });
    await loaded();
    await userEvent.click(within(rail()).getByRole('button', { name: 'Als neue Frage anlegen' }));
    const dialog = screen.getByRole('dialog', { name: 'Folgefrage zu ARC-F0001' });
    expect((within(dialog).getByLabelText('Titel') as HTMLInputElement).value).toBe('Soll der Export verschlüsselt werden?');
    // Ziffern im Dialog wählen keine Option
    await userEvent.type(within(dialog).getByLabelText('Tags'), '1');
    expect(within(card()).getByRole('radio', { name: 'Datei' }).getAttribute('aria-checked')).toBe('false');
    await userEvent.clear(within(dialog).getByLabelText('Tags'));
    await userEvent.click(within(dialog).getByRole('button', { name: 'Frage stellen' }));
    expect(createDbQuestion).toHaveBeenCalledWith('ARC', {
      title: 'Soll der Export verschlüsselt werden?',
      description: 'Soll der Export verschlüsselt werden? Betrifft auch das Backup.',
      occasion: 'Folgefrage zu ARC-F0001',
      tags: [],
      ticketIds: [],
      dependsOn: ['ARC-F0001'],
    });
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
  });

  it('verweist auf eine schon angelegte Folgefrage', async () => {
    open({ getDbQuestions: vi.fn(async () => dbQuestionList([...LIST.questions, dbQuestion('ARC-F0005', 'Verschlüsseln?', { occasion: 'Folgefrage zu ARC-F0001' })])) });
    await loaded();
    const followUp = within(rail()).getByRole('region', { name: 'Folgefrage aus der Recherche' });
    expect((await within(followUp).findByRole('link', { name: 'ARC-F0005' })).getAttribute('href')).toBe('/p/p1/questions/ARC-F0005');
    expect(within(followUp).queryByRole('button', { name: 'Als neue Frage anlegen' })).toBeNull();
  });

  it('zeigt den Hinweis bei nicht verfügbarer Board-Datenbank', async () => {
    open({ getDbQuestion: async () => { throw new ApiError('board-schema-outdated', 'Schema-Version 1, erwartet 2', 503); } });
    expect(await screen.findByText('Board-Datenbank nicht verfügbar')).toBeTruthy();
  });
});

describe('Fragen – Recherche', () => {
  it('startet die Recherche, zeigt den Hinweis und lädt die Frage nach, wenn sie endet', async () => {
    const run = { id: 'r1', kind: 'research' as const, ticketId: 'ARC-F0001', questionId: 'ARC-F0001', title: 'Welcher Speicher?', profile: 'standard', model: null, effort: null, startedAt: '2026-10-15T07:00:00.000Z', log: 'ARC-F0001-20261015-090000-research.log' };
    let running: typeof run | null = null;
    const getQuestionResearch = vi.fn(async () => ({ questionId: 'ARC-F0001', run: running, pausedUntil: null, lastStartedAt: null, count: 0 }));
    const startQuestionResearch = vi.fn(async () => {
      running = run;
      return { started: run };
    });
    const getDbQuestion = vi.fn(async () => DETAIL);
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      open({ getQuestionResearch, startQuestionResearch, getDbQuestion });
      const section = () => within(rail()).getByRole('region', { name: 'Recherche' });
      const button = await within(await screen.findByRole('complementary', { name: 'Bezüge und Details' })).findByRole('button', { name: 'Neue Recherche starten' });
      await userEvent.click(button);
      expect(startQuestionResearch).toHaveBeenCalledWith('ARC-F0001');
      expect((await within(section()).findByRole('status')).textContent).toContain('Recherche läuft – das Ergebnis erscheint hier automatisch');
      expect((within(section()).getByRole('button', { name: 'Neue Recherche starten' }) as HTMLButtonElement).disabled).toBe(true);
      const loads = getDbQuestion.mock.calls.length;
      running = null;
      await act(async () => { await vi.advanceTimersByTimeAsync(10_000); });
      await waitFor(() => expect(getDbQuestion.mock.calls.length).toBeGreaterThan(loads));
      await waitFor(() => expect((within(section()).getByRole('button', { name: 'Neue Recherche starten' }) as HTMLButtonElement).disabled).toBe(false));
    } finally {
      vi.useRealTimers();
    }
  });

  it('zeigt Fehler beim Start sichtbar an', async () => {
    open({
      startQuestionResearch: async () => { throw new ApiError('research-no-repo', 'Für Frage ARC-F0001 ist kein eigenes Board mit Repo-Pfad bekannt', 422); },
    });
    await loaded();
    await userEvent.click(within(rail()).getByRole('button', { name: 'Neue Recherche starten' }));
    const section = within(rail()).getByRole('region', { name: 'Recherche' });
    expect((await within(section).findByRole('alert')).textContent).toContain('kein eigenes Board mit Repo-Pfad');
  });

  it('zeigt, dass der Dispatcher pausiert', async () => {
    const until = new Date(2026, 9, 15, 11, 30).toISOString();
    open({ getQuestionResearch: async () => ({ questionId: 'ARC-F0001', run: null, pausedUntil: until, lastStartedAt: null, count: 0 }) });
    await loaded();
    expect(await within(rail()).findByText('Der Dispatcher pausiert bis 11:30 (z. B. Nutzungslimit) – eine Recherche startet erst danach.')).toBeTruthy();
  });

  it('zeigt die Recherche-Läufe mit Protokoll und Arbeitsanweisung im Reiter „Läufe“', async () => {
    const getDispatchRuns = vi.fn(async () => [{ name: 'ARC-F0001-20261016-120000-research.log', kind: 'research' as const, started: '2026-10-16T10:00:00.000Z', running: false }]);
    const getDispatchRunLog = vi.fn(async (name: string) => ({ name, header: '[dispatch] Start', prompt: 'Recherchiere ARC-F0001.', body: '12:00:01  Claude startet (opus)\n' }));
    open({ getDispatchRuns, getDispatchRunLog });
    await loaded();
    const runs = await showTab('Läufe');
    expect(await within(runs).findByText(/Claude startet \(opus\)/)).toBeTruthy();
    expect(within(runs).getByText(/Recherche vom/)).toBeTruthy();
    expect(within(runs).getByText('Arbeitsanweisung an den Agent')).toBeTruthy();
    expect(getDispatchRuns).toHaveBeenCalledWith('ARC-F0001');
  });

  it('meldet im Reiter „Läufe“, wenn es noch keinen gab', async () => {
    open();
    await loaded();
    const runs = await showTab('Läufe');
    expect(await within(runs).findByText('Zu dieser Frage gab es noch keinen Recherche-Lauf.')).toBeTruthy();
  });
});

describe('Fragen – entschiedene Frage', () => {
  it('zeigt die Entscheidung, betroffene Tickets und die Recherche nur lesbar', async () => {
    open({
      getDbQuestion: vi.fn(async () => decided(DETAIL, 4)),
      getDbQuestions: vi.fn(async () => dbQuestionList([{ ...SUMMARY, state: 'answered', ready: false }])),
    });
    await loaded();
    expect(screen.getByText(/· claude \(Session\) · angelegt 14\.10\.2026/)).toBeTruthy();
    const box = screen.getByRole('region', { name: 'Entscheidung' });
    expect(within(box).getByText('DB')).toBeTruthy();
    expect(within(box).getByText(/^Florian · /)).toBeTruthy();
    expect(within(box).queryByText('vom Autopilot entschieden')).toBeNull();
    expect(screen.queryByRole('region', { name: 'Entscheiden' })).toBeNull();
    await waitFor(() => expect(screen.getByRole('tab', { name: 'Erledigt (1)' }).getAttribute('aria-selected')).toBe('true'));

    // Folgefrage in der Hauptspalte, nicht in der Leiste
    const followUps = screen.getAllByRole('region', { name: 'Folgefrage aus der Recherche' });
    expect(followUps).toHaveLength(1);
    expect(rail().contains(followUps[0])).toBe(false);
    expect(within(followUps[0]).getByText('Die Recherche sah eine weitere Frage')).toBeTruthy();

    const tickets = within(rail()).getByRole('region', { name: 'Betroffene Tickets' });
    expect(within(tickets).getByText('Freigegeben – sie warten nicht mehr.')).toBeTruthy();
    expect(within(rail()).getByText('Nur offene Fragen lassen sich recherchieren. „Wieder öffnen“ macht es möglich.')).toBeTruthy();

    const research = await showTab('Recherche 1');
    expect(within(research).getByText('Kontext · Heute')).toBeTruthy();
    expect(within(research).getByText('Alles liegt in einer Datei.')).toBeTruthy();
    expect(within(research).queryByRole('radio')).toBeNull();
    const chosen = within(research).getByText('gewählt').closest('details')!;
    expect(within(chosen).getByText('DB')).toBeTruthy();
  });

  it('kennzeichnet eine vom Autopilot entschiedene Frage', async () => {
    open({ getDbQuestion: vi.fn(async () => decided(DETAIL, 4, { reason: 'Autopilot: Empfehlung der Recherche' })) });
    const box = await screen.findByRole('region', { name: 'Entscheidung' });
    expect(within(box).getByText('vom Autopilot entschieden')).toBeTruthy();
  });

  it('nennt eine Folgefrage, die nach der Entscheidung recherchiert wurde, als solche', async () => {
    open({ getDbQuestion: vi.fn(async () => decided(DETAIL, 4, { at: '2026-10-14T09:30:00.000Z' })) });
    await screen.findByRole('region', { name: 'Entscheidung' });
    expect(screen.getByText('Die Recherche kam nach der Entscheidung und schlägt eine Folgefrage vor')).toBeTruthy();
  });
});
