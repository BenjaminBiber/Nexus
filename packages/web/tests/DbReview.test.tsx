// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ApiProvider } from '../src/api/ApiContext.js';
import type { DispatchReviewDto } from '../src/api/dispatchTypes.js';
import { DbReview, showsReview } from '../src/components/db/DbReview.js';
import { DETAIL, openPanel } from './helpers/dbPanel.js';
import { dbCard, dbDetail, dispatchStatus, fakeApi } from './helpers/fakeApi.js';

afterEach(cleanup);

const TICKET = { ...dbDetail(dbCard('ARC-0007', 'Rabatt', 'blocked', { blockedReason: 'Human Interaction: Review vor dem Merge', boardId: 1 })), branch: 'ticket/ARC-0007-rabatt' };

const REVIEW: DispatchReviewDto = {
  ticketId: 'ARC-0007', branch: 'ticket/ARC-0007-rabatt', target: 'main', base: 'abc', testsAt: '2026-10-19T10:00:00.000Z', runLog: 'ARC-0007-x.log',
  available: true, reason: null,
  files: [
    { path: 'src/rabatt.ts', status: 'M', additions: 2, deletions: 1, binary: false },
    { path: 'neu.ts', oldPath: 'alt.ts', status: 'R', additions: 0, deletions: 0, binary: false },
  ],
  tests: [
    { command: 'npx vitest run', ok: true, output: 'Tests 3 passed' },
    { command: 'npm run lint', ok: false, output: '1 error' },
    { command: 'npm test', ok: null, output: '' },
  ],
};

// Rendert den Block mit gemockter API
function show(overrides: Parameters<typeof fakeApi>[0] = {}) {
  const onDone = vi.fn();
  const getDispatchReviewFile = vi.fn(async (_id: string, path: string) => ({ path, patch: '@@ -1 +1 @@\n-alt\n+neu\n', truncated: false }));
  const api = { getDispatchReview: async () => REVIEW, getDispatchReviewFile, ...overrides };
  render(<ApiProvider api={fakeApi(api)}><DbReview ticket={TICKET} onDone={onDone} /></ApiProvider>);
  return { onDone, getDispatchReviewFile };
}

describe('DbReview', () => {
  it('erscheint nur für Human Interaction mit Branch auf eigenem Board', () => {
    expect(showsReview(TICKET, true)).toBe(true);
    expect(showsReview(TICKET, false)).toBe(false);
    expect(showsReview({ ...TICKET, branch: null }, true)).toBe(false);
    expect(showsReview({ ...TICKET, blockedReason: 'Zugang fehlt' }, true)).toBe(false);
  });

  it('zeigt Ziel, Dateien und Testläufe; Aufklappen lädt den Patch genau einmal', async () => {
    const { getDispatchReviewFile } = show();
    expect(await screen.findByText('Wartet auf Freigabe: ticket/ARC-0007-rabatt → main')).toBeTruthy();
    expect(screen.getByText('2 Dateien geändert, +2 −1')).toBeTruthy();
    expect(screen.getByText('alt.ts → neu.ts')).toBeTruthy();
    expect(screen.getByText('grün')).toBeTruthy();
    expect(screen.getByText('rot')).toBeTruthy();
    expect(screen.getByText('ohne Ergebnis')).toBeTruthy();
    expect(screen.getByText('npm run lint')).toBeTruthy();
    expect(screen.getByText('src')).toBeTruthy();
    expect(screen.getByText('1 Datei · +2 −1')).toBeTruthy();
    const file = screen.getByText('rabatt.ts');
    await userEvent.click(file);
    await waitFor(() => expect(getDispatchReviewFile).toHaveBeenCalledWith('ARC-0007', 'src/rabatt.ts'));
    const added = await screen.findByText('+neu');
    expect(added.className).toBe('add');
    expect(screen.getByText('-alt').className).toBe('del');
    await userEvent.click(file);
    await userEvent.click(file);
    expect(getDispatchReviewFile).toHaveBeenCalledTimes(1);
  });

  it('„Übernehmen“ ruft die Abnahme und lädt neu', async () => {
    const accept = vi.fn(async () => ({ ...dispatchStatus(), result: { outcome: 'merged' as const, text: 'ARC-0007 gemergt in main (abc1234).' } }));
    const { onDone } = show({ acceptDispatchTicket: accept });
    await userEvent.click(await screen.findByRole('button', { name: 'Übernehmen' }));
    expect(accept).toHaveBeenCalledWith('ARC-0007', 1);
    expect(onDone).toHaveBeenCalled();
    expect(screen.getByRole('status').textContent).toContain('ARC-0007 gemergt in main');
  });

  it('„Zurück an den Agenten“ braucht eine Rückmeldung; Strg+Enter sendet', async () => {
    const back = vi.fn(async () => ({ ...dispatchStatus(), result: { outcome: 'returned' as const, text: 'ARC-0007 zurück an den Agenten' } }));
    const { onDone } = show({ returnDispatchTicket: back });
    await userEvent.click(await screen.findByRole('button', { name: 'Zurück an den Agenten …' }));
    const send = screen.getByRole('button', { name: 'Zurück an den Agenten' }) as HTMLButtonElement;
    expect(send.disabled).toBe(true);
    await userEvent.type(screen.getByLabelText('Rückmeldung an den Agenten'), 'Bitte auch den Fehlerfall testen');
    expect(send.disabled).toBe(false);
    await userEvent.keyboard('{Control>}{Enter}{/Control}');
    expect(back).toHaveBeenCalledWith('ARC-0007', 'Bitte auch den Fehlerfall testen', 1);
    expect(onDone).toHaveBeenCalled();
  });

  it('zeigt viele Dateien als zugeklappten Baum; Ordner zeigen Anzahl und Zeilen', async () => {
    const many = Array.from({ length: 8 }, (_, i) => ({ path: `packages/server/src/f${i}.ts`, status: 'M', additions: 1, deletions: 0, binary: false }));
    show({ getDispatchReview: async () => ({ ...REVIEW, files: [...many, { path: 'CHANGELOG.md', status: 'M', additions: 2, deletions: 0, binary: false }] }) });
    const dir = (await screen.findByText('packages/server/src')).closest('details')!;
    expect(dir.open).toBe(false);
    expect(screen.getByText('8 Dateien · +8 −0')).toBeTruthy();
    expect(screen.getByText('CHANGELOG.md')).toBeTruthy();
    expect(screen.getByRole('button', { name: /Freigabe/ }).textContent).toContain('9 Dateien · 1 Test rot');
  });

  it('zeigt den Hinweis statt des Diffs, wenn keiner verfügbar ist', async () => {
    show({ getDispatchReview: async () => ({ ...REVIEW, available: false, reason: 'Das Board hat keinen Repo-Pfad.', files: [], tests: [] }) });
    expect(await screen.findByText('Das Board hat keinen Repo-Pfad.')).toBeTruthy();
    expect(screen.queryByText(/Dateien geändert/)).toBeNull();
    expect(screen.getByText(/Keine Testläufe erkannt/)).toBeTruthy();
  });

  it('erscheint im Ticket-Panel statt „Abnehmen“', async () => {
    const detail = { ...DETAIL, status: 'blocked' as const, blockedReason: 'Human Interaction: Review vor dem Merge', branch: 'ticket/ARC-0001-login', boardId: 1 };
    openPanel({ getDispatchReview: async () => ({ ...REVIEW, ticketId: 'ARC-0001' }) }, detail, [], { isOwnBoard: () => true });
    expect(await screen.findByRole('region', { name: 'Freigabe' })).toBeTruthy();
    expect(screen.queryByRole('button', { name: 'Abnehmen' })).toBeNull();
    expect(screen.getByRole('button', { name: 'Übernehmen' })).toBeTruthy();
  });
});
