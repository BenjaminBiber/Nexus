// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest';
import { act, cleanup, renderHook, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { ApiProvider } from '../src/api/ApiContext.js';
import { ApiError } from '../src/api/ApiError.js';
import { useQuestionResearch } from '../src/dbquestion/useQuestionResearch.js';
import { fakeApi } from './helpers/fakeApi.js';

afterEach(cleanup);

const RUN = {
  id: 'r1', kind: 'research' as const, ticketId: 'ARC-F0001', questionId: 'ARC-F0001', title: 'Eins', profile: 'standard', model: null, effort: null,
  startedAt: '2026-10-15T07:00:00.000Z', log: 'ARC-F0001-20261015-090000-research.log',
};

describe('useQuestionResearch', () => {
  it('setzt Lauf, Pause und Fehler beim Wechsel der Frage zurück und verwirft späte Antworten zur vorigen Frage', async () => {
    let releaseOld: () => void = () => undefined;
    const api = fakeApi({
      getQuestionResearch: async (id: string) => {
        if (id === 'ARC-F0001') return { questionId: id, run: RUN, pausedUntil: '2026-10-15T09:00:00.000Z', lastStartedAt: null, count: 1 };
        if (id === 'ARC-F0003') {
          await new Promise<void>((done) => { releaseOld = done; });
          return { questionId: id, run: { ...RUN, questionId: id }, pausedUntil: null, lastStartedAt: null, count: 1 };
        }
        return { questionId: id, run: null, pausedUntil: null, lastStartedAt: null, count: 0 };
      },
      startQuestionResearch: async () => { throw new ApiError('dispatch-paused', 'Der Dispatcher pausiert', 422); },
    });
    const wrapper = ({ children }: { children: ReactNode }) => <ApiProvider api={api}>{children}</ApiProvider>;
    const { result, rerender } = renderHook(({ id }) => useQuestionResearch(id, () => undefined), { wrapper, initialProps: { id: 'ARC-F0001' } });
    await waitFor(() => expect(result.current.run).not.toBeNull());
    expect(result.current.pausedUntil).toBe('2026-10-15T09:00:00.000Z');
    await act(async () => { await result.current.start(); });
    expect(result.current.error).toBe('Der Dispatcher pausiert');

    rerender({ id: 'ARC-F0002' });
    expect(result.current).toMatchObject({ run: null, pausedUntil: null, error: null, busy: false });

    // Antwort zu ARC-F0003 kommt erst nach dem Wechsel zu ARC-F0002 an und wird verworfen
    rerender({ id: 'ARC-F0003' });
    rerender({ id: 'ARC-F0002' });
    await act(async () => { releaseOld(); });
    await waitFor(() => expect(result.current.run).toBeNull());
  });
});
