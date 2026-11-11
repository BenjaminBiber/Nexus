import type { DbCardDto } from '../api/dbTypes.js';
import type { DoneGrouping } from '../api/types.js';

const DAY_MS = 24 * 60 * 60 * 1000;
// Bis zu diesem Alter gilt ein erledigtes Ticket als frisch („Letzte 7 Tage“)
const RECENT_MS = 7 * DAY_MS;

/** Abschnitt der Spalte „Erledigt“; `collapsed` ist der Ausgangswert, solange der Nutzer nichts umschaltet */
export interface DoneSection {
  key: string;
  title: string;
  cards: DbCardDto[];
  collapsed: boolean;
}

/** Gliederung der Spalte „Erledigt“: Abschnitte, Zahl der gezeigten und der archivierten Tickets */
export interface DoneLayout {
  sections: DoneSection[];
  shown: number;
  archived: number;
}

export interface DoneLayoutOptions {
  grouping: DoneGrouping;
  /** Gemergte Tickets, die älter als so viele Tage sind, gelten als archiviert (0 = nie) */
  archiveDays: number;
  /** Archivierte Tickets trotzdem zeigen */
  showArchived: boolean;
  /** Die Suche im Board ist aktiv: archivierte Treffer zeigen, nichts einklappen */
  searching: boolean;
  now: Date;
}

// Zeitpunkt, zu dem das Ticket erledigt wurde (ohne Angabe die letzte Änderung); NaN, wenn nicht lesbar
export function doneTime(card: DbCardDto): number {
  return Date.parse(card.doneAt ?? card.updatedAt);
}

// Neueste zuerst; nicht lesbare Zeitpunkte zuletzt, bei Gleichstand die höhere Ticketnummer zuerst
function byNewest(a: DbCardDto, b: DbCardDto): number {
  const ta = doneTime(a);
  const tb = doneTime(b);
  if (Number.isNaN(ta) !== Number.isNaN(tb)) return Number.isNaN(ta) ? 1 : -1;
  if (!Number.isNaN(ta) && ta !== tb) return tb - ta;
  return b.number - a.number;
}

// Erledigt, aber noch nicht gemergt: Der Branch des Tickets besteht noch
function hasBranch(card: DbCardDto): boolean {
  return Boolean(card.branch?.trim());
}

// Archiviert sind nur gemergte Tickets mit lesbarem Datum, die älter als die eingestellten Tage sind
function isArchived(card: DbCardDto, options: DoneLayoutOptions): boolean {
  if (options.archiveDays <= 0 || hasBranch(card)) return false;
  const time = doneTime(card);
  return !Number.isNaN(time) && options.now.getTime() - time > options.archiveDays * DAY_MS;
}

// Derselbe Kalendertag in Ortszeit
function sameDay(a: Date, b: Date): boolean {
  return a.getFullYear() === b.getFullYear() && a.getMonth() === b.getMonth() && a.getDate() === b.getDate();
}

type TimeKey = 'today' | 'week' | 'older';

const TIME_SECTIONS: { key: TimeKey; title: string }[] = [
  { key: 'today', title: 'Heute' },
  { key: 'week', title: 'Letzte 7 Tage' },
  { key: 'older', title: 'Älter' },
];

function timeKey(card: DbCardDto, now: Date): TimeKey {
  const time = doneTime(card);
  if (Number.isNaN(time)) return 'older';
  if (sameDay(new Date(time), now)) return 'today';
  return now.getTime() - time < RECENT_MS ? 'week' : 'older';
}

// Abschnitte nach Zeit; leere fehlen, „Älter“ ist im Ausgangswert eingeklappt
function timeSections(cards: DbCardDto[], now: Date): DoneSection[] {
  return TIME_SECTIONS.map(({ key, title }) => ({ key, title, cards: cards.filter((card) => timeKey(card, now) === key), collapsed: key === 'older' })).filter(
    (section) => section.cards.length > 0,
  );
}

// Abschnitte nach Spec. Die Karten sind schon nach Datum sortiert, die Gruppen folgen damit ihrem jüngsten Ticket;
// „Ohne Spec“ steht zuletzt. Eine Gruppe ist im Ausgangswert eingeklappt, wenn ihr jüngstes Ticket 7 Tage oder älter ist.
function specSections(cards: DbCardDto[], now: Date): DoneSection[] {
  const groups = new Map<string, DbCardDto[]>();
  for (const card of cards) {
    const spec = card.spec?.trim() ?? '';
    groups.set(spec, [...(groups.get(spec) ?? []), card]);
  }
  const entries = Array.from(groups.entries());
  const ordered = [...entries.filter(([spec]) => spec !== ''), ...entries.filter(([spec]) => spec === '')];
  return ordered.map(([spec, group]) => {
    const newest = group.map(doneTime).find((time) => !Number.isNaN(time));
    return { key: `spec:${spec}`, title: spec || 'Ohne Spec', cards: group, collapsed: newest === undefined || now.getTime() - newest >= RECENT_MS };
  });
}

// Gliedert die erledigten Tickets einer Spalte: zuerst „Wartet auf Merge“ (Tickets mit Branch), danach Abschnitte
// nach Zeit oder Spec, jeweils neueste zuerst. Archivierte Tickets fehlen, außer bei showArchived oder aktiver Suche.
export function buildDoneLayout(cards: DbCardDto[], options: DoneLayoutOptions): DoneLayout {
  const revealAll = options.showArchived || options.searching;
  const visible = revealAll ? cards : cards.filter((card) => !isArchived(card, options));
  const merging = visible.filter(hasBranch).sort(byNewest);
  const rest = visible.filter((card) => !hasBranch(card)).sort(byNewest);
  const groups = options.grouping === 'spec' ? specSections(rest, options.now) : timeSections(rest, options.now);
  const all: DoneSection[] = [...(merging.length > 0 ? [{ key: 'merge', title: 'Wartet auf Merge', cards: merging, collapsed: false }] : []), ...groups];
  // Der erste Abschnitt bleibt offen, damit die Spalte nie leer wirkt; bei aktiver Suche ist nichts eingeklappt
  const sections = all.map((section, index) => ({ ...section, collapsed: !options.searching && index > 0 && section.collapsed }));
  return { sections, shown: visible.length, archived: cards.length - visible.length };
}

const two = (value: number) => String(value).padStart(2, '0');

// Kurzes Datum einer Zeile: heute die Uhrzeit, sonst Tag und Monat, in einem anderen Jahr mit Jahr; leer, wenn nicht lesbar
export function formatDoneDate(iso: string, now: Date): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return '';
  if (sameDay(date, now)) return `${two(date.getHours())}:${two(date.getMinutes())}`;
  const day = `${two(date.getDate())}.${two(date.getMonth() + 1)}.`;
  return date.getFullYear() === now.getFullYear() ? day : `${day}${two(date.getFullYear() % 100)}`;
}
