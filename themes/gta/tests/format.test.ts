// Anzeigeformate für Geld, Uhr und Prozent
import { describe, expect, it } from 'vitest';
import { formatAmountSa, formatClockSa, formatClockV, formatMoneySa, formatMoneyV, formatPercent, openTickets, percentOf } from '../src/shared/format.js';

describe('Geld', () => {
  it('GTA V: Dollar mit deutschen Tausenderpunkten', () => {
    expect(formatMoneyV(184_000)).toBe('$184.000');
    expect(formatMoneyV(0)).toBe('$0');
    expect(formatMoneyV(999)).toBe('$999');
    expect(formatMoneyV(1_234_567)).toBe('$1.234.567');
    expect(formatMoneyV(1234.6)).toBe('$1.235');
  });

  it('San Andreas: acht Stellen mit Nullen, größere Zahlen ungekürzt', () => {
    expect(formatMoneySa(184_000)).toBe('$00184000');
    expect(formatMoneySa(0)).toBe('$00000000');
    expect(formatMoneySa(123_456_789)).toBe('$123456789');
  });

  it('San Andreas als Betrag (Banner, Cheat): ohne führende Nullen', () => {
    expect(formatAmountSa(40_000)).toBe('$40000');
    expect(formatAmountSa(250_000)).toBe('$250000');
    expect(formatAmountSa(-3)).toBe('$0');
  });

  it('negative Werte und NaN wie 0', () => {
    for (const n of [-5, Number.NaN, Number.NEGATIVE_INFINITY]) {
      expect(formatMoneyV(n)).toBe('$0');
      expect(formatMoneySa(n)).toBe('$00000000');
      expect(formatPercent(n)).toBe('0 %');
    }
  });
});

describe('Uhr', () => {
  it('GTA V: deutscher Wochentag und Uhrzeit', () => {
    expect(formatClockV(new Date(2026, 10, 11, 9, 41))).toBe('Mittwoch 09:41');
    expect(formatClockV(new Date(2026, 10, 15, 23, 5))).toBe('Sonntag 23:05');
    expect(formatClockV(new Date(2026, 10, 16, 0, 0))).toBe('Montag 00:00');
  });

  it('San Andreas: nur Uhrzeit', () => {
    expect(formatClockSa(new Date(2026, 10, 11, 9, 41))).toBe('09:41');
  });
});

describe('Prozent', () => {
  it('gerundet mit Leerzeichen', () => {
    expect(formatPercent(0.42)).toBe('42 %');
    expect(formatPercent(1)).toBe('100 %');
    expect(formatPercent(0.005)).toBe('1 %');
  });

  it('als Zahl für Balken: ganz und auf 0..100 begrenzt', () => {
    expect(percentOf(0.424)).toBe(42);
    expect(percentOf(1.7)).toBe(100);
    expect(percentOf(-0.2)).toBe(0);
  });
});

describe('Tickets', () => {
  it('offene Tickets in Einzahl und Mehrzahl', () => {
    expect(openTickets(1)).toBe('1 offenes Ticket');
    expect(openTickets(0)).toBe('0 offene Tickets');
    expect(openTickets(5)).toBe('5 offene Tickets');
  });
});
