import { afterEach, describe, expect, it, vi } from 'vitest';
import { DispatchScheduler } from '../../../src/dispatch/DispatchScheduler.js';
import { ToastTexts } from '../../../src/notifications/ToastTexts.js';

describe('DispatchScheduler', () => {
  afterEach(() => vi.useRealTimers());

  it('tickt im Takt und hört nach stop auf; 0 heißt aus', async () => {
    vi.useFakeTimers();
    const tick = vi.fn(async () => undefined);
    const scheduler = new DispatchScheduler({ tick }, 1000);
    scheduler.start();
    await vi.advanceTimersByTimeAsync(3000);
    expect(tick).toHaveBeenCalledTimes(3);
    scheduler.stop();
    await vi.advanceTimersByTimeAsync(3000);
    expect(tick).toHaveBeenCalledTimes(3);
    const off = vi.fn(async () => undefined);
    new DispatchScheduler({ tick: off }, 0).start();
    await vi.advanceTimersByTimeAsync(3000);
    expect(off).not.toHaveBeenCalled();
  });

  it('stop wartet auf einen laufenden Tick, höchstens bis zur Wartezeit', async () => {
    vi.useFakeTimers();
    let finish: () => void = () => undefined;
    const tick = vi.fn(() => new Promise<void>((resolve) => { finish = resolve; }));
    const scheduler = new DispatchScheduler({ tick }, 1000, () => undefined, 30_000);
    scheduler.start();
    await vi.advanceTimersByTimeAsync(1000);
    let stopped = false;
    const stopping = scheduler.stop().then(() => { stopped = true; });
    await vi.advanceTimersByTimeAsync(5000);
    expect(stopped).toBe(false);
    finish();
    await stopping;
    expect(stopped).toBe(true);
    expect(tick).toHaveBeenCalledTimes(1);

    // Endet der Tick nie, gibt stop nach der Wartezeit auf
    const log = vi.fn();
    const hanging = new DispatchScheduler({ tick: () => new Promise<void>(() => undefined) }, 1000, log, 30_000);
    hanging.start();
    await vi.advanceTimersByTimeAsync(1000);
    let gaveUp = false;
    const giving = hanging.stop().then(() => { gaveUp = true; });
    await vi.advanceTimersByTimeAsync(29_000);
    expect(gaveUp).toBe(false);
    await vi.advanceTimersByTimeAsync(1000);
    await giving;
    expect(gaveUp).toBe(true);
    expect(log).toHaveBeenCalledWith('Dispatcher: laufender Durchlauf nach 30 s nicht beendet – Server schließt trotzdem');
    // Ohne laufenden Tick kehrt stop sofort zurück
    await new DispatchScheduler({ tick }, 1000).stop();
  });

  it('protokolliert Fehler eines Ticks statt abzustürzen', async () => {
    vi.useFakeTimers();
    const log = vi.fn();
    const scheduler = new DispatchScheduler({ tick: async () => { throw new Error('kaputt'); } }, 1000, log);
    scheduler.start();
    await vi.advanceTimersByTimeAsync(1000);
    scheduler.stop();
    expect(log).toHaveBeenCalledWith('Dispatcher: kaputt');
  });
});

describe('ToastTexts.dispatch', () => {
  it('formuliert Meldungen des Dispatchers', () => {
    const texts = new ToastTexts();
    expect(texts.dispatch({ notice: { kind: 'merged', ticketId: 'ARC-0009', title: 'Rabatte', text: 'in main gemergt (abc1234)', repoPath: null }, openUrl: 'u' }))
      .toEqual({ kind: 'bereit', title: 'Dispatcher: gemergt', line1: 'ARC-0009 Rabatte', line2: 'in main gemergt (abc1234)', openUrl: 'u' });
    expect(texts.dispatch({ notice: { kind: 'blocked', ticketId: 'ARC-0009', title: 'Rabatte', text: 'Merge: x', repoPath: null }, openUrl: 'u' }))
      .toMatchObject({ kind: 'wartet', title: 'Dispatcher: blockiert' });
    expect(texts.dispatch({ notice: { kind: 'question', ticketId: 'ARC-0009', title: 'R', text: 'wartet auf ARC-F0001', repoPath: null }, openUrl: 'u' }).title).toBe('Dispatcher: Frage vom Agent');
    expect(texts.dispatch({ notice: { kind: 'paused', ticketId: null, title: 'R', text: 'Nutzungslimit', repoPath: null }, openUrl: 'u' }).title).toBe('Dispatcher pausiert');
  });
});
