import { describe, expect, it } from 'vitest';
import type { DbCardDto } from '../src/api/dbTypes.js';
import { buildDoneLayout, formatDoneDate, type DoneLayout, type DoneLayoutOptions } from '../src/dbboard/doneSections.js';
import { dbCard } from './helpers/fakeApi.js';

// Mittwoch, 11. November 2026, 12:00 Uhr Ortszeit
const NOW = new Date(2026, 10, 11, 12, 0, 0);
const HOUR = 60 * 60 * 1000;
const DAY = 24 * HOUR;
const ago = (ms: number) => new Date(NOW.getTime() - ms).toISOString();

// Erledigtes Ticket mit Erledigt-Datum
const done = (id: string, doneAt: string | null, extra: Partial<DbCardDto> = {}) => dbCard(id, `Titel ${id}`, 'done', { doneAt, ...extra });
const layout = (cards: DbCardDto[], options: Partial<DoneLayoutOptions> = {}) =>
  buildDoneLayout(cards, { grouping: 'time', archiveDays: 0, showArchived: false, searching: false, now: NOW, ...options });
// Abschnitte als [Titel, IDs, eingeklappt]
const shape = (result: DoneLayout) => result.sections.map((section) => [section.title, section.cards.map((card) => card.id), section.collapsed]);

describe('buildDoneLayout – nach Zeit', () => {
  it('gliedert in Heute, Letzte 7 Tage und Älter, neueste zuerst, Älter eingeklappt', () => {
    const result = layout([
      done('ARC-0006', ago(40 * DAY)),
      done('ARC-0003', ago(2 * DAY)),
      done('ARC-0002', ago(5 * HOUR)),
      done('ARC-0005', ago(7 * DAY)),
      done('ARC-0001', ago(2 * HOUR)),
      done('ARC-0004', ago(7 * DAY - HOUR)),
    ]);
    expect(shape(result)).toEqual([
      ['Heute', ['ARC-0001', 'ARC-0002'], false],
      ['Letzte 7 Tage', ['ARC-0003', 'ARC-0004'], false],
      ['Älter', ['ARC-0005', 'ARC-0006'], true],
    ]);
    expect(result.shown).toBe(6);
    expect(result.archived).toBe(0);
  });

  it('zählt den Vortag kurz vor Mitternacht nicht zu Heute', () => {
    const result = layout([done('ARC-0001', new Date(2026, 10, 10, 23, 59).toISOString()), done('ARC-0002', new Date(2026, 10, 11, 0, 1).toISOString())]);
    expect(shape(result)).toEqual([
      ['Heute', ['ARC-0002'], false],
      ['Letzte 7 Tage', ['ARC-0001'], false],
    ]);
  });

  it('nimmt ohne Erledigt-Datum die letzte Änderung', () => {
    const result = layout([done('ARC-0001', null, { updatedAt: ago(HOUR) }), dbCard('ARC-0002', 'Ohne Feld', 'done', { updatedAt: ago(3 * DAY) })]);
    expect(shape(result)).toEqual([
      ['Heute', ['ARC-0001'], false],
      ['Letzte 7 Tage', ['ARC-0002'], false],
    ]);
  });

  it('lässt den ersten Abschnitt offen, auch wenn er „Älter“ ist', () => {
    expect(shape(layout([done('ARC-0001', ago(40 * DAY))]))).toEqual([['Älter', ['ARC-0001'], false]]);
  });

  it('ordnet bei gleichem Zeitpunkt die höhere Ticketnummer zuerst', () => {
    expect(shape(layout([done('ARC-0002', ago(HOUR)), done('ARC-0010', ago(HOUR))]))).toEqual([['Heute', ['ARC-0010', 'ARC-0002'], false]]);
  });

  it('stellt ein nicht lesbares Datum ans Ende von „Älter“', () => {
    expect(shape(layout([done('ARC-0001', 'kaputt'), done('ARC-0002', ago(30 * DAY))]))).toEqual([['Älter', ['ARC-0002', 'ARC-0001'], false]]);
  });

  it('verändert die übergebene Liste nicht', () => {
    const cards = [done('ARC-0001', ago(3 * DAY)), done('ARC-0002', ago(HOUR))];
    layout(cards);
    expect(cards.map((card) => card.id)).toEqual(['ARC-0001', 'ARC-0002']);
  });
});

describe('buildDoneLayout – Wartet auf Merge', () => {
  it('stellt Tickets mit Branch nach vorn, neueste zuerst und nie eingeklappt', () => {
    const result = layout([
      done('ARC-0001', ago(HOUR)),
      done('ARC-0007', ago(40 * DAY), { branch: 'ticket/ARC-0007-alt' }),
      done('ARC-0008', ago(2 * HOUR), { branch: 'ticket/ARC-0008-neu' }),
      done('ARC-0009', ago(50 * DAY)),
      done('ARC-0010', ago(HOUR), { branch: '  ' }),
    ]);
    expect(shape(result)).toEqual([
      ['Wartet auf Merge', ['ARC-0008', 'ARC-0007'], false],
      ['Heute', ['ARC-0010', 'ARC-0001'], false],
      ['Älter', ['ARC-0009'], true],
    ]);
  });
});

describe('buildDoneLayout – Archiv', () => {
  const cards = [
    done('ARC-0001', ago(DAY)),
    done('ARC-0002', ago(14 * DAY)),
    done('ARC-0003', ago(14 * DAY + HOUR)),
    done('ARC-0004', ago(60 * DAY), { branch: 'ticket/ARC-0004-offen' }),
    done('ARC-0005', 'kaputt'),
  ];

  it('blendet gemergte Tickets aus, die älter als die eingestellten Tage sind', () => {
    const result = layout(cards, { archiveDays: 14 });
    expect(shape(result)).toEqual([
      ['Wartet auf Merge', ['ARC-0004'], false],
      ['Letzte 7 Tage', ['ARC-0001'], false],
      ['Älter', ['ARC-0002', 'ARC-0005'], true],
    ]);
    expect(result.shown).toBe(4);
    expect(result.archived).toBe(1);
  });

  it('archiviert bei 0 Tagen nie', () => {
    const result = layout(cards, { archiveDays: 0 });
    expect(result.shown).toBe(5);
    expect(result.archived).toBe(0);
  });

  it('zeigt archivierte Tickets mit showArchived in ihrem Abschnitt', () => {
    const result = layout(cards, { archiveDays: 14, showArchived: true });
    expect(shape(result)).toEqual([
      ['Wartet auf Merge', ['ARC-0004'], false],
      ['Letzte 7 Tage', ['ARC-0001'], false],
      ['Älter', ['ARC-0002', 'ARC-0003', 'ARC-0005'], true],
    ]);
    expect(result.shown).toBe(5);
    expect(result.archived).toBe(0);
  });

  it('zeigt bei aktiver Suche archivierte Tickets und klappt nichts ein', () => {
    const result = layout(cards, { archiveDays: 14, searching: true });
    expect(result.sections.map((section) => section.collapsed)).toEqual([false, false, false]);
    expect(result.shown).toBe(5);
    expect(result.archived).toBe(0);
  });
});

describe('buildDoneLayout – nach Spec', () => {
  it('bildet je Spec eine Gruppe nach ihrem jüngsten Ticket, „Ohne Spec“ zuletzt', () => {
    const result = layout(
      [
        done('ARC-0001', ago(HOUR), { spec: 'theme-store' }),
        done('ARC-0002', ago(3 * DAY), { spec: 'theme-store' }),
        done('ARC-0003', ago(2 * DAY), { spec: ' login ' }),
        done('ARC-0004', ago(HOUR / 2)),
        done('ARC-0005', ago(10 * DAY), { spec: '  ' }),
        done('ARC-0006', ago(20 * DAY), { spec: 'alt' }),
      ],
      { grouping: 'spec' },
    );
    expect(shape(result)).toEqual([
      ['theme-store', ['ARC-0001', 'ARC-0002'], false],
      ['login', ['ARC-0003'], false],
      ['alt', ['ARC-0006'], true],
      ['Ohne Spec', ['ARC-0004', 'ARC-0005'], false],
    ]);
    expect(result.sections.map((section) => section.key)).toEqual(['spec:theme-store', 'spec:login', 'spec:alt', 'spec:']);
  });

  it('lässt „Wartet auf Merge“ auch bei der Gruppierung nach Spec vorn', () => {
    const result = layout([done('ARC-0001', ago(HOUR), { spec: 'login' }), done('ARC-0002', ago(DAY), { spec: 'login', branch: 'ticket/ARC-0002-x' })], { grouping: 'spec' });
    expect(shape(result)).toEqual([
      ['Wartet auf Merge', ['ARC-0002'], false],
      ['login', ['ARC-0001'], false],
    ]);
  });
});

describe('formatDoneDate', () => {
  it('zeigt heute die Uhrzeit, sonst Tag und Monat, in einem anderen Jahr mit Jahr', () => {
    expect(formatDoneDate(new Date(2026, 10, 11, 9, 5).toISOString(), NOW)).toBe('09:05');
    expect(formatDoneDate(new Date(2026, 10, 9, 15, 0).toISOString(), NOW)).toBe('09.11.');
    expect(formatDoneDate(new Date(2025, 11, 31, 15, 0).toISOString(), NOW)).toBe('31.12.25');
  });

  it('liefert für ein nicht lesbares Datum einen leeren Text', () => {
    expect(formatDoneDate('kaputt', NOW)).toBe('');
  });
});
