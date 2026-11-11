// Spielwerte aus Board-Stand, Verbrauch und Navigation – reine Funktionen, aus denen die Rahmen nur noch lesen.
// Das Geld ist Spielgeld: 40.000 je erledigtem Ticket, 2.500 je erledigtem Schritt offener Tickets.
import type { NavItem, UsageState, UsageWindow } from '@nexus/theme-sdk';
import type { BoardView, Card, ColumnId } from './types.js';

export const TICKET_REWARD = 40_000;
export const STEP_REWARD = 2_500;

/** Ticket als Punkt auf Minimap bzw. Radar */
export interface Blip {
  id: string;
  title: string;
  column: ColumnId;
  x: number;
  y: number;
  blocked: boolean;
}

export interface ColumnStat {
  id: ColumnId;
  title: string;
  count: number;
}

export interface GameState {
  loaded: boolean; // gibt es einen Board-Stand?
  total: number; // Tickets ohne not_needed
  done: number;
  open: number;
  blocked: number;
  boardProgress: number | null; // done / total, 0..1; null ohne Tickets
  stepsDone: number; // über offene Tickets
  stepsTotal: number;
  money: number; // Spielgeld: done * TICKET_REWARD + Schritte offener Tickets * STEP_REWARD
  quota5h: number | null; // Rest des 5-Stunden-Kontingents 0..1; null wenn nicht verfügbar
  quota7d: number | null;
  tasks: number; // Badge von „Meine Aufgaben“ (NavItem inbox)
  questions: number; // Badge von „Fragen“ (NavItem questions)
  columns: ColumnStat[]; // in Reihenfolge des Boards, ohne not_needed
  blips: Blip[]; // offene Tickets
}

const RADIUS = 0.42;

const counted = (card: Card) => card.column !== 'not_needed';
const isOpen = (card: Card) => counted(card) && card.column !== 'done';
const sum = (cards: Card[], pick: (card: Card) => number) => cards.reduce((total, card) => total + pick(card), 0);

// Rest eines Limit-Fensters als 0..1
function quota(usage: UsageState, window: UsageWindow | null): number | null {
  if (!usage.available || !window || !Number.isFinite(window.percent)) return null;
  return Math.min(1, Math.max(0, 1 - window.percent / 100));
}

const badge = (nav: NavItem[], id: NavItem['id']) => nav.find((item) => item.id === id)?.badge ?? 0;

export function deriveGame(view: BoardView | null, usage: UsageState, nav: NavItem[]): GameState {
  const cards = (view?.columns ?? []).flatMap((column) => column.tickets).filter(counted);
  const open = cards.filter(isOpen);
  const done = cards.length - open.length;
  const stepsDone = sum(open, (card) => card.stepsDone);
  return {
    loaded: view !== null,
    total: cards.length,
    done,
    open: open.length,
    blocked: cards.filter((card) => card.column === 'blocked').length,
    boardProgress: cards.length ? done / cards.length : null,
    stepsDone,
    stepsTotal: sum(open, (card) => card.stepsTotal),
    money: done * TICKET_REWARD + stepsDone * STEP_REWARD,
    quota5h: quota(usage, usage.fiveHour),
    quota7d: quota(usage, usage.sevenDay),
    tasks: badge(nav, 'inbox'),
    questions: badge(nav, 'questions'),
    columns: (view?.columns ?? [])
      .filter((column) => column.id !== 'not_needed')
      .map((column) => ({ id: column.id, title: column.title, count: column.tickets.length })),
    blips: open.map((card) => ({
      id: card.id,
      title: card.title,
      column: card.column,
      blocked: card.column === 'blocked',
      ...blipPosition(card.id),
    })),
  };
}

// FNV-1a über die UTF-16-Einheiten, danach durchgemischt (Murmur3-Finalizer), damit ähnliche IDs weit auseinander liegen
function fnv1a(text: string): number {
  let hash = 0x811c9dc5;
  for (let i = 0; i < text.length; i++) {
    hash ^= text.charCodeAt(i);
    hash = Math.imul(hash, 0x01000193);
  }
  return hash >>> 0;
}

function mix(hash: number): number {
  let h = hash;
  h = Math.imul(h ^ (h >>> 16), 0x85ebca6b);
  h = Math.imul(h ^ (h >>> 13), 0xc2b2ae35);
  return (h ^ (h >>> 16)) >>> 0;
}

/** Feste Position aus der Ticket-ID: x, y in 0..1, immer innerhalb eines Kreises mit Radius 0,42 um die Mitte (0,5/0,5) */
export function blipPosition(id: string): { x: number; y: number } {
  const a = mix(fnv1a(id));
  const b = mix(a ^ 0x9e3779b9);
  const angle = (a / 2 ** 32) * 2 * Math.PI;
  // Wurzel: gleichmäßig über die Fläche statt gehäuft in der Mitte
  const r = RADIUS * Math.sqrt(b / 2 ** 32);
  return { x: 0.5 + r * Math.cos(angle), y: 0.5 + r * Math.sin(angle) };
}
