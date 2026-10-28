import { describe, expect, it } from 'vitest';
import { SseEvents } from '../lib/SseEvents.mjs';
import { SseParser } from '../lib/SseParser.mjs';
import { SseCloseStep, SseExpectStep, SseOpenStep } from '../lib/SseSteps.mjs';

// Stellt einen Strom nach, der seine Einträge schon gesammelt hat (statt einer echten Verbindung)
const stream = (entries) => ({
  closed: false,
  async expect(until) {
    const at = entries.findIndex((entry) => SseEvents.matches(entry, until));
    if (at < 0) throw new Error('kein Treffer');
    return entries.splice(0, at + 1);
  },
  close() {
    this.closed = true;
  },
});

// Kontext eines Schritts mit einem Strom unter dem Namen `s1`
const context = (entries) => ({
  base: 'http://127.0.0.1:1',
  variables: { apply: (value) => value },
  streams: new Map([['s1', stream(entries)]]),
});

// Erwartet eine Meldung beim Prüfen eines fehlerhaften Schritts
const failsWith = (kind, value, pattern) => {
  const fail = (message) => {
    throw new Error(message);
  };
  expect(() => kind.parse(value, 0, fail)).toThrow(pattern);
};

describe('SseParser', () => {
  it('liest mehrere Blöcke aus einem Teil des Stroms', () => {
    const parser = new SseParser();
    expect(parser.push('data: {"type":"hello"}\n\ndata: {"type":"sessions"}\n\n')).toEqual([{ type: 'hello' }, { type: 'sessions' }]);
  });

  it('setzt einen Block über zwei Teile hinweg zusammen', () => {
    const parser = new SseParser();
    expect(parser.push('data: {"type":"db')).toEqual([]);
    expect(parser.push('board","boardId":7}\n\n')).toEqual([{ type: 'dbboard', boardId: 7 }]);
  });

  it('gibt einen Block erst mit seiner leeren Zeile heraus', () => {
    const parser = new SseParser();
    expect(parser.push('data: {"type":"hello"}\n')).toEqual([]);
    expect(parser.push('\n')).toEqual([{ type: 'hello' }]);
  });

  it('erfasst Kommentarzeilen als Herzschlag', () => {
    const parser = new SseParser();
    expect(parser.push(': ping\n\n')).toEqual([{ comment: 'ping' }]);
  });

  it('liest Kommentar und Ereignis in der Reihenfolge des Stroms', () => {
    const parser = new SseParser();
    expect(parser.push(': ping\n\ndata: {"type":"notice"}\n\n')).toEqual([{ comment: 'ping' }, { type: 'notice' }]);
  });

  it('vereinheitlicht Zeilenenden mit Wagenrücklauf', () => {
    const parser = new SseParser();
    expect(parser.push('data: {"type":"hello"}\r\n\r\n')).toEqual([{ type: 'hello' }]);
  });

  it('fügt mehrere data-Zeilen eines Blocks zusammen', () => {
    const parser = new SseParser();
    expect(parser.push('data: {"type":\ndata: "hello"}\n\n')).toEqual([{ type: 'hello' }]);
  });

  it('lässt einen Inhalt, der kein JSON ist, als Text sichtbar', () => {
    const parser = new SseParser();
    expect(parser.push('data: kein json\n\n')).toEqual([{ data: 'kein json' }]);
  });

  it('überspringt Blöcke ohne data und ohne Kommentar', () => {
    const parser = new SseParser();
    expect(parser.push('event: x\nid: 1\n\ndata: {"type":"hello"}\n\n')).toEqual([{ type: 'hello' }]);
  });
});

describe('SseEvents', () => {
  it('entfernt Herzschläge, deren Anzahl am Takt hängt', () => {
    const entries = [{ comment: 'ping' }, { type: 'sessions' }, { comment: 'ping' }];
    expect(SseEvents.withoutComments(entries)).toEqual([{ type: 'sessions' }]);
  });

  it('fasst aufeinanderfolgende gleiche Einträge zu einem zusammen', () => {
    const entries = [{ type: 'dbboard', boardId: 1 }, { type: 'dbboard', boardId: 1 }, { type: 'dbboard', boardId: 1 }];
    expect(SseEvents.fold(entries)).toEqual([{ type: 'dbboard', boardId: 1 }]);
  });

  it('behält gleiche Einträge, zwischen denen ein anderer steht', () => {
    const entries = [{ type: 'dbboard', boardId: 1 }, { type: 'sessions' }, { type: 'dbboard', boardId: 1 }];
    expect(SseEvents.fold(entries)).toEqual(entries);
  });

  it('unterscheidet Einträge, die sich nur in einem Feld unterscheiden', () => {
    const entries = [{ type: 'dbboard', boardId: 1 }, { type: 'dbboard', boardId: 2 }];
    expect(SseEvents.fold(entries)).toEqual(entries);
  });

  it('erkennt einen Kommentar als Herzschlag', () => {
    expect(SseEvents.isComment({ comment: 'ping' })).toBe(true);
    expect(SseEvents.isComment({ type: 'hello' })).toBe(false);
  });

  it('vergleicht until als Teilobjekt', () => {
    expect(SseEvents.matches({ type: 'dbboard', boardId: 7 }, { type: 'dbboard' })).toBe(true);
    expect(SseEvents.matches({ type: 'dbboard', boardId: 7 }, { type: 'dbboard', boardId: 7 })).toBe(true);
    expect(SseEvents.matches({ type: 'dbboard', boardId: 7 }, { type: 'dbboard', boardId: 8 })).toBe(false);
    expect(SseEvents.matches({ type: 'hello' }, { type: 'hello', boardId: 1 })).toBe(false);
  });

  it('vergleicht verschachtelte Felder wieder als Teilobjekt', () => {
    expect(SseEvents.matches({ type: 'notice', data: { kind: 'wartet', tag: 'x' } }, { data: { kind: 'wartet' } })).toBe(true);
    expect(SseEvents.matches({ type: 'notice', data: { kind: 'fertig' } }, { data: { kind: 'wartet' } })).toBe(false);
  });

  it('passt nie auf einen Eintrag, der kein Objekt ist', () => {
    expect(SseEvents.matches(null, { type: 'hello' })).toBe(false);
    expect(SseEvents.matches([{ type: 'hello' }], { type: 'hello' })).toBe(false);
  });
});

describe('sseOpen', () => {
  const kind = new SseOpenStep();

  it('kennt nur seine eigenen Felder', () => {
    expect(kind.keys).toEqual(['path', 'as']);
  });

  it('übernimmt Pfad und Namen des Stroms', () => {
    expect(kind.parse({ path: '/api/projects/x/events', as: 's1' }, 0, () => {})).toEqual({ path: '/api/projects/x/events', as: 's1' });
  });

  it('meldet einen Pfad ohne führenden Schrägstrich', () => {
    failsWith(kind, { path: 'api/events', as: 's1' }, /steps\[0\]\.path muss mit \/ beginnen/);
  });

  it('meldet einen fehlenden Namen des Stroms', () => {
    failsWith(kind, { path: '/api/events' }, /steps\[0\]\.as muss ein Name des Stroms sein/);
  });
});

describe('sseExpect', () => {
  const kind = new SseExpectStep();

  it('setzt die Standardfrist', () => {
    expect(kind.parse({ stream: 's1', until: { type: 'hello' } }, 0, () => {})).toEqual({ stream: 's1', until: { type: 'hello' }, timeoutMs: 3000 });
  });

  it('meldet ein until ohne Feld', () => {
    failsWith(kind, { stream: 's1', until: {} }, /steps\[0\]\.until muss ein Objekt mit mindestens einem Feld sein/);
  });

  it('meldet eine Frist, die keine ganze Zahl ab 1 ist', () => {
    failsWith(kind, { stream: 's1', until: { type: 'hello' }, timeoutMs: 0 }, /steps\[0\]\.timeoutMs/);
  });

  it('zeichnet die Ereignisse bis zum Treffer ohne Herzschläge auf', async () => {
    const entries = [{ comment: 'ping' }, { type: 'sessions' }, { type: 'hello' }];
    const step = { stream: 's1', until: { type: 'sessions' }, timeoutMs: 100 };
    const result = await kind.run(step, context(entries));
    expect(result).toEqual({ request: { stream: 's1', until: { type: 'sessions' } }, response: { events: [{ type: 'sessions' }] } });
  });

  it('zeichnet Herzschläge auf, wenn gerade auf einen gewartet wird', async () => {
    const entries = [{ comment: 'ping' }, { comment: 'ping' }];
    const step = { stream: 's1', until: { comment: 'ping' }, timeoutMs: 100 };
    const result = await kind.run(step, context(entries));
    expect(result.response).toEqual({ events: [{ comment: 'ping' }] });
  });

  it('fasst gleiche Ereignisse vor dem Treffer zusammen', async () => {
    const entries = [{ type: 'dbboard', boardId: 1 }, { type: 'dbboard', boardId: 1 }, { type: 'sessions' }];
    const step = { stream: 's1', until: { type: 'sessions' }, timeoutMs: 100 };
    const result = await kind.run(step, context(entries));
    expect(result.response).toEqual({ events: [{ type: 'dbboard', boardId: 1 }, { type: 'sessions' }] });
  });

  it('meldet einen Strom, den es nicht gibt', async () => {
    const step = { stream: 'sX', until: { type: 'hello' }, timeoutMs: 100 };
    await expect(kind.run(step, context([]))).rejects.toThrow(/Es gibt keinen offenen Strom „sX“/);
  });
});

describe('sseClose', () => {
  const kind = new SseCloseStep();

  it('bricht die Verbindung ab und gibt den Namen frei', async () => {
    const ctx = context([]);
    const open = ctx.streams.get('s1');
    const result = await kind.run({ stream: 's1' }, ctx);
    expect(open.closed).toBe(true);
    expect(ctx.streams.has('s1')).toBe(false);
    expect(result).toEqual({ request: { stream: 's1' }, response: { closed: true } });
  });

  it('meldet einen zweiten Aufruf für denselben Strom', async () => {
    const ctx = context([]);
    await kind.run({ stream: 's1' }, ctx);
    await expect(kind.run({ stream: 's1' }, ctx)).rejects.toThrow(/sseClose kam schon/);
  });
});
