// @vitest-environment jsdom
import { describe, it, expect, afterEach, vi } from 'vitest';
import { cleanup, renderHook, waitFor, act } from '@testing-library/react';
import type { ReactNode } from 'react';
import { ApiProvider } from '../src/api/ApiContext.js';
import type { DocCommentDto, NewDocCommentInput } from '../src/api/types.js';
import { useDocComments } from '../src/hooks/useDocs.js';
import { fakeApi } from './helpers/fakeApi.js';

afterEach(cleanup);

const comment: DocCommentDto = {
  id: 'c1', status: 'open', text: 'Bitte prüfen', createdAt: '2026-10-10T10:00:00.000Z',
  quote: 'Beispiel', heading: 'Titel', lines: [3, 3], resolution: null,
};
const input: NewDocCommentInput = { text: 'Neu', quote: 'Beispiel', heading: 'Titel', lines: [3, 3] };

// Rendert den Hook mit einer Fake-API
function setup(overrides: Record<string, unknown>) {
  const api = fakeApi(overrides);
  const wrapper = ({ children }: { children: ReactNode }) => <ApiProvider api={api}>{children}</ApiProvider>;
  return renderHook(() => useDocComments('p1', 'docs/a.md'), { wrapper });
}

describe('useDocComments', () => {
  it('lädt die Kommentare des Dokuments', async () => {
    const getDocComments = vi.fn(async () => [comment]);
    const { result } = setup({ getDocComments });
    await waitFor(() => expect(result.current.comments).toEqual([comment]));
    expect(getDocComments).toHaveBeenCalledWith('p1', 'docs/a.md', undefined);
  });

  it('add ruft addDocComment auf und lädt neu', async () => {
    const getDocComments = vi.fn(async () => [comment]);
    const addDocComment = vi.fn(async () => comment);
    const { result } = setup({ getDocComments, addDocComment });
    await waitFor(() => expect(result.current.comments).toHaveLength(1));
    await act(() => result.current.add(input));
    expect(addDocComment).toHaveBeenCalledWith('p1', 'docs/a.md', input, undefined);
    await waitFor(() => expect(getDocComments).toHaveBeenCalledTimes(2));
  });

  it('meldet einen Fehler als Text', async () => {
    const addDocComment = vi.fn(async () => {
      throw new Error('Speichern fehlgeschlagen');
    });
    const { result } = setup({ getDocComments: async () => [], addDocComment });
    await waitFor(() => expect(result.current.comments).toEqual([]));
    await act(() => result.current.add(input));
    expect(result.current.error).toBe('Speichern fehlgeschlagen');
  });

  it('update und remove rufen die API auf und laden neu', async () => {
    const getDocComments = vi.fn(async () => [comment]);
    const patchDocComment = vi.fn(async () => comment);
    const deleteDocComment = vi.fn(async () => undefined);
    const { result } = setup({ getDocComments, patchDocComment, deleteDocComment });
    await waitFor(() => expect(result.current.comments).toHaveLength(1));
    await act(() => result.current.update('c1', { status: 'resolved' }));
    expect(patchDocComment).toHaveBeenCalledWith('p1', 'docs/a.md', 'c1', { status: 'resolved' }, undefined);
    await act(() => result.current.remove('c1'));
    expect(deleteDocComment).toHaveBeenCalledWith('p1', 'docs/a.md', 'c1', undefined);
    await waitFor(() => expect(getDocComments).toHaveBeenCalledTimes(3));
  });

  it('setzt den Fehler nach einer erfolgreichen Änderung zurück', async () => {
    const addDocComment = vi.fn()
      .mockRejectedValueOnce(new Error('Speichern fehlgeschlagen'))
      .mockResolvedValueOnce(comment);
    const { result } = setup({ getDocComments: async () => [], addDocComment });
    await waitFor(() => expect(result.current.comments).toEqual([]));
    await act(() => result.current.add(input));
    expect(result.current.error).toBe('Speichern fehlgeschlagen');
    await act(() => result.current.add(input));
    expect(result.current.error).toBeNull();
  });

  it('meldet einen Ladefehler als Text', async () => {
    const { result } = setup({
      getDocComments: async () => {
        throw new Error('Laden fehlgeschlagen');
      },
    });
    await waitFor(() => expect(result.current.error).toBe('Laden fehlgeschlagen'));
  });

  it('verwirft den Änderungsfehler beim Wechsel des Dokuments', async () => {
    const api = fakeApi({
      getDocComments: async () => [],
      addDocComment: vi.fn(async () => {
        throw new Error('Speichern fehlgeschlagen');
      }),
    });
    const wrapper = ({ children }: { children: ReactNode }) => <ApiProvider api={api}>{children}</ApiProvider>;
    const { result, rerender } = renderHook(({ path }) => useDocComments('p1', path), {
      wrapper,
      initialProps: { path: 'docs/a.md' },
    });
    await waitFor(() => expect(result.current.comments).toEqual([]));
    await act(() => result.current.add(input));
    expect(result.current.error).toBe('Speichern fehlgeschlagen');
    rerender({ path: 'docs/b.md' });
    await waitFor(() => expect(result.current.error).toBeNull());
  });
});
