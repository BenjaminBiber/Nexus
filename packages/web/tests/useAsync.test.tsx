// @vitest-environment jsdom
import { describe, expect, it } from 'vitest';
import { act, renderHook } from '@testing-library/react';
import { useAsync } from '../src/hooks/useAsync.js';

describe('useAsync', () => {
  it('verwirft einen älteren Ladevorgang, der nach set() eintrifft', async () => {
    let resolve: (value: string) => void = () => undefined;
    const { result } = renderHook(() => useAsync(() => new Promise<string>((r) => { resolve = r; }), []));
    act(() => result.current.set('neu'));
    await act(async () => resolve('alt'));
    expect(result.current.state).toEqual({ status: 'ready', data: 'neu' });
  });
});
