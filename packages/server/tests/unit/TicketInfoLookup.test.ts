import { describe, expect, it } from 'vitest';
import { TicketInfoLookup } from '../../src/sessions/TicketInfoLookup.js';
import type { BoardRuntimeHolder } from '../../src/dbboard/BoardRuntimeHolder.js';

type Run = BoardRuntimeHolder['run'];

describe('TicketInfoLookup', () => {
  it('liefert die Angaben aus der Board-DB', async () => {
    const run = (async (fn: (b: unknown) => unknown) =>
      fn({ services: { tickets: { summaries: async () => [{ id: 'ARC-0001', title: 'Login', status: 'planned' }] } } })) as unknown as Run;
    const infos = await new TicketInfoLookup({ run }).infos(['ARC-0001']);
    expect(infos.get('ARC-0001')).toEqual({ id: 'ARC-0001', title: 'Login', status: 'planned' });
  });

  it('wartet nicht ewig auf eine hängende Board-DB und antwortet wie ohne DB', async () => {
    const run = (() => new Promise(() => {})) as unknown as Run;
    const started = Date.now();
    const infos = await new TicketInfoLookup({ run }, 50).infos(['ARC-0001']);
    expect(infos.size).toBe(0);
    expect(Date.now() - started).toBeLessThan(1000);
  });

  it('begrenzt die Wartezeit standardmäßig auf etwa eine Sekunde', async () => {
    const run = (() => new Promise(() => {})) as unknown as Run;
    const started = Date.now();
    expect((await new TicketInfoLookup({ run }).infos(['ARC-0001'])).size).toBe(0);
    expect(Date.now() - started).toBeLessThan(3000);
  });
});
