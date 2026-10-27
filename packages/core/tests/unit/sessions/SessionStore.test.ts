import { describe, it, expect } from 'vitest';
import { SessionStore, STALE_MS, RETENTION_MS } from '../../../src/sessions/SessionStore.js';
import type { HookEvent } from '../../../src/sessions/Session.js';

const P = 'proj1';

// Baut ein Ereignis; nicht genannte Felder haben Standardwerte
function ev(kind: HookEvent['kind'], extra: Partial<HookEvent> = {}): HookEvent {
  return { kind, sessionId: 's1', cwd: 'D:/wt/T-0042', ...extra };
}

// Ein Speicher mit steuerbarer Uhr
function clock(start = '2026-10-10T10:00:00.000Z') {
  let now = new Date(start).getTime();
  const store = new SessionStore([], () => new Date(now));
  return { store, advance: (ms: number) => (now += ms) };
}

describe('SessionStore: Zustände', () => {
  it('legt bei session-start eine untätige Session mit Tickets und Verzeichnis an', () => {
    const { store } = clock();
    const s = store.apply(P, ev('session-start', { tickets: ['T-0042'], title: 'Login', model: 'm' }));
    expect(s).toMatchObject({ id: 's1', projectId: P, tickets: ['T-0042'], cwd: 'D:/wt/T-0042', state: 'idle', title: 'Login', model: 'm' });
    expect(s.startedAt).toBe('2026-10-10T10:00:00.000Z');
  });

  it('wechselt: Prompt → läuft, Benachrichtigung → wartet, Stop → untätig, Ende → beendet', () => {
    const { store, advance } = clock();
    store.apply(P, ev('session-start'));
    advance(1000);
    expect(store.apply(P, ev('user-prompt')).state).toBe('running');
    expect(store.apply(P, ev('notification', { notificationType: 'permission_prompt' })).state).toBe('waiting');
    expect(store.apply(P, ev('user-prompt')).state).toBe('running');
    expect(store.apply(P, ev('notification', { notificationType: 'idle_prompt' })).state).toBe('waiting');
    expect(store.apply(P, ev('stop')).state).toBe('idle');
    advance(5000);
    const ended = store.apply(P, ev('session-end'));
    expect(ended.state).toBe('ended');
    expect(ended.endedAt).toBe('2026-10-10T10:00:06.000Z');
  });

  it('lässt den Zustand bei anderen Benachrichtigungen unverändert, aktualisiert aber die letzte Meldung', () => {
    const { store, advance } = clock();
    store.apply(P, ev('user-prompt'));
    advance(60000);
    const s = store.apply(P, ev('notification', { notificationType: 'auth_success' }));
    expect(s.state).toBe('running');
    expect(s.lastEventAt).toBe('2026-10-10T10:01:00.000Z');
  });

  it('legt eine unbekannte Session bei jedem Ereignis an', () => {
    const { store } = clock();
    expect(store.apply(P, ev('user-prompt', { sessionId: 'neu' })).state).toBe('running');
    expect(store.apply(P, ev('stop', { sessionId: 'noch-neu' })).state).toBe('idle');
  });

  it('setzt beim erneuten session-start (Fortsetzen) zurück, ohne den Beginn zu verlieren', () => {
    const { store, advance } = clock();
    store.apply(P, ev('session-start'));
    store.apply(P, ev('session-end'));
    advance(10000);
    const s = store.apply(P, ev('session-start'));
    expect(s.state).toBe('idle');
    expect(s.endedAt).toBeUndefined();
    expect(s.startedAt).toBe('2026-10-10T10:00:00.000Z');
  });

  it('vereinigt Tickets ohne Doppelungen', () => {
    const { store } = clock();
    store.apply(P, ev('session-start', { tickets: ['T-0001'] }));
    const s = store.apply(P, ev('user-prompt', { tickets: ['T-0001', 'T-0002'] }));
    expect(s.tickets).toEqual(['T-0001', 'T-0002']);
  });

  it('hält Projekte getrennt', () => {
    const { store } = clock();
    store.apply('a', ev('session-start'));
    store.apply('b', ev('session-start', { sessionId: 's9' }));
    expect(store.list('a').map((s) => s.id)).toEqual(['s1']);
    expect(store.list('b').map((s) => s.id)).toEqual(['s9']);
    expect(store.list()).toHaveLength(2);
  });
});

describe('SessionStore: Subagenten', () => {
  it('führt Subagenten mit Start und Ende und setzt die Session auf „läuft“', () => {
    const { store, advance } = clock();
    store.apply(P, ev('session-start'));
    advance(1000);
    let s = store.apply(P, ev('subagent-start', { agentId: 'a1', agentType: 'general-purpose' }));
    expect(s.state).toBe('running');
    expect(s.subagents).toEqual([{ id: 'a1', type: 'general-purpose', state: 'running', startedAt: '2026-10-10T10:00:01.000Z' }]);
    advance(4000);
    s = store.apply(P, ev('subagent-stop', { agentId: 'a1', agentType: 'general-purpose' }));
    expect(s.subagents[0]).toMatchObject({ state: 'ended', endedAt: '2026-10-10T10:00:05.000Z' });
  });

  it('führt ein Subagenten-Ende ohne Start als beendeten Subagenten', () => {
    const { store } = clock();
    const s = store.apply(P, ev('subagent-stop', { agentId: 'x', agentType: 'Explore' }));
    expect(s.subagents).toHaveLength(1);
    expect(s.subagents[0].state).toBe('ended');
  });

  it('bleibt bei „läuft“, wenn ein Leerlauf-Hinweis kommt, während Subagenten arbeiten', () => {
    const { store } = clock();
    store.apply(P, ev('subagent-start', { agentId: 'a1', agentType: 't' }));
    expect(store.apply(P, ev('notification', { notificationType: 'idle_prompt' })).state).toBe('running');
    // Eine Rückfrage (Berechtigung) bleibt ein echtes Warten
    expect(store.apply(P, ev('notification', { notificationType: 'permission_prompt' })).state).toBe('waiting');
    // Ohne aktive Subagenten gilt der Leerlauf-Hinweis wie bisher
    store.apply(P, ev('subagent-stop', { agentId: 'a1', agentType: 't' }));
    expect(store.apply(P, ev('notification', { notificationType: 'idle_prompt' })).state).toBe('waiting');
  });

  it('beendet laufende Subagenten bei stop und session-end', () => {
    const { store } = clock();
    store.apply(P, ev('subagent-start', { agentId: 'a1', agentType: 't' }));
    expect(store.apply(P, ev('stop')).subagents[0].state).toBe('ended');
    store.apply(P, ev('subagent-start', { agentId: 'a2', agentType: 't' }));
    expect(store.apply(P, ev('session-end')).subagents.every((a) => a.state === 'ended')).toBe(true);
  });
});

describe('SessionStore: Anzeige, Lauschen und Aufbewahrung', () => {
  it('berechnet „veraltet“ nach 30 Minuten ohne Meldung (nicht für beendete)', () => {
    const { store, advance } = clock();
    store.apply(P, ev('user-prompt'));
    store.apply(P, ev('session-start', { sessionId: 'fertig' }));
    store.apply(P, ev('session-end', { sessionId: 'fertig' }));
    advance(STALE_MS + 1000);
    const byId = Object.fromEntries(store.list(P).map((s) => [s.id, s]));
    expect(byId.s1.displayState).toBe('stale');
    expect(byId.s1.state).toBe('running');
    expect(byId.fertig.displayState).toBe('ended');
  });

  it('meldet „listening“ für untätige, laufende und wartende Sessions, nicht für beendete oder veraltete', () => {
    const { store, advance } = clock();
    for (const [id, kind] of [['i', 'session-start'], ['r', 'user-prompt'], ['w', 'notification'], ['e', 'session-end']] as const) {
      store.apply(P, ev(kind, { sessionId: id, notificationType: 'permission_prompt' }));
    }
    const listening = (id: string) => store.list(P).find((s) => s.id === id)!.listening;
    expect([listening('i'), listening('r'), listening('w'), listening('e')]).toEqual([true, true, true, false]);
    advance(STALE_MS + 1000);
    expect(listening('r')).toBe(false);
  });

  it('berechnet die Laufzeit bis zum Ende oder bis jetzt', () => {
    const { store, advance } = clock();
    store.apply(P, ev('session-start'));
    advance(90000);
    expect(store.list(P)[0].runtimeMs).toBe(90000);
    store.apply(P, ev('session-end'));
    advance(60000);
    expect(store.list(P)[0].runtimeMs).toBe(90000);
  });

  it('sortiert nach letzter Aktivität, neueste zuerst', () => {
    const { store, advance } = clock();
    store.apply(P, ev('session-start', { sessionId: 'alt' }));
    advance(1000);
    store.apply(P, ev('session-start', { sessionId: 'neu' }));
    expect(store.list(P).map((s) => s.id)).toEqual(['neu', 'alt']);
  });

  it('liefert Sessions zu einem Ticket', () => {
    const { store } = clock();
    store.apply(P, ev('session-start', { tickets: ['T-0001'] }));
    store.apply(P, ev('session-start', { sessionId: 's2', tickets: ['T-0002'] }));
    expect(store.forTicket(P, 'T-0002').map((s) => s.id)).toEqual(['s2']);
    expect(store.forTicket(P, 'T-0099')).toEqual([]);
  });

  it('entfernt Sessions nach 7 Tagen (beendete und stumme), behält jüngere', () => {
    const { store, advance } = clock();
    store.apply(P, ev('session-start', { sessionId: 'beendet' }));
    store.apply(P, ev('session-end', { sessionId: 'beendet' }));
    store.apply(P, ev('session-start', { sessionId: 'stumm' }));
    advance(RETENTION_MS - 1000);
    store.apply(P, ev('session-start', { sessionId: 'frisch' }));
    expect(store.prune()).toBe(0);
    advance(2000);
    expect(store.prune()).toBe(2);
    expect(store.list(P).map((s) => s.id)).toEqual(['frisch']);
  });

  it('bereinigt automatisch bei jedem Ereignis', () => {
    const { store, advance } = clock();
    store.apply(P, ev('session-start', { sessionId: 'alt' }));
    advance(RETENTION_MS + 1000);
    store.apply(P, ev('session-start', { sessionId: 'neu' }));
    expect(store.toJSON().map((s) => s.id)).toEqual(['neu']);
  });

  it('übernimmt gespeicherte Sessions und gibt sie unverändert zurück', () => {
    const { store } = clock();
    store.apply(P, ev('session-start', { tickets: ['T-0001'] }));
    const copy = new SessionStore(store.toJSON(), () => new Date('2026-10-10T10:05:00.000Z'));
    expect(copy.list(P)[0]).toMatchObject({ id: 's1', tickets: ['T-0001'] });
  });
});

describe('SessionStore: claim', () => {
  it('ordnet einer bekannten Session ein Ticket zu', () => {
    const { store } = clock();
    store.apply(P, ev('session-start'));
    expect(store.claim(P, 's1', 'T-0042').tickets).toEqual(['T-0042']);
    expect(store.claim(P, 's1', 'T-0042').tickets).toEqual(['T-0042']);
  });

  it('meldet eine unbekannte Session verständlich', () => {
    const { store } = clock();
    expect(() => store.claim(P, 'gibtsnicht', 'T-1')).toThrow(/Session gibtsnicht/);
  });
});
