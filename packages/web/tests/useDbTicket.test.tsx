// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, renderHook, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { ApiProvider } from '../src/api/ApiContext.js';
import type { DbTicketPatch } from '../src/api/dbTypes.js';
import { useDbTicket } from '../src/dbboard/useDbTicket.js';
import { DETAIL } from './helpers/dbPanel.js';
import { fakeApi } from './helpers/fakeApi.js';

afterEach(cleanup);

describe('useDbTicket – Änderung aus dem Stand beim Senden', () => {
  it('berechnet eine wartende Änderung aus dem Stand, den der vorige Speichervorgang hinterließ', async () => {
    let current = { ...DETAIL, dependsOn: ['ARC-0002'] };
    const patchDbTicket = vi.fn(async (_id: string, patch: DbTicketPatch) => (current = { ...current, ...patch, rev: current.rev + 1 }));
    const wrapper = ({ children }: { children: ReactNode }) => <ApiProvider api={fakeApi({ getDbTicket: async () => current, patchDbTicket })}>{children}</ApiProvider>;
    const { result } = renderHook(() => useDbTicket('ARC-0001', () => {}), { wrapper });
    await waitFor(() => expect(result.current.ticket).not.toBeNull());
    await act(async () => {
      // Die erste Änderung setzt die Liste anderweitig, die zweite hängt an den dann bekannten Stand an
      const first = result.current.save({ dependsOn: ['ARC-0002', 'ARC-0009'] });
      const second = result.current.save((base) => ({ dependsOn: [...base.dependsOn, 'ARC-0003'] }));
      await Promise.all([first, second]);
    });
    expect(patchDbTicket.mock.calls[1][1]).toEqual({ dependsOn: ['ARC-0002', 'ARC-0009', 'ARC-0003'], rev: 4 });
  });
});
