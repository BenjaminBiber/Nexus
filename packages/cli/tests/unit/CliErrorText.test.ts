import { describe, expect, it } from 'vitest';
import { HarnessError } from '@harness/core';
import { CliErrorText } from '../../src/output/CliErrorText.js';

describe('CliErrorText', () => {
  const text = new CliErrorText();

  it('ergänzt bei offener Frage oder Voraussetzung den Hinweis auf --force', () => {
    expect(text.of(new HarnessError('question-open', 'ARC-0001 wartet auf Frage ARC-F0001. Erst die Frage entscheiden lassen.')))
      .toBe('ARC-0001 wartet auf Frage ARC-F0001. Erst die Frage entscheiden lassen. Mit --force übergehen.');
    expect(text.of(new HarnessError('dependency-open', 'ARC-0002 wartet auf ARC-0001. Erst die Voraussetzung erledigen.')))
      .toBe('ARC-0002 wartet auf ARC-0001. Erst die Voraussetzung erledigen. Mit --force übergehen.');
  });

  it('ergänzt bei einer Dublette den Hinweis, dass --force trotzdem anlegt', () => {
    expect(text.line(new HarnessError('ticket-duplicate', 'Ein offenes Ticket mit diesem Titel gibt es schon: ARC-0012.')))
      .toBe('Ein offenes Ticket mit diesem Titel gibt es schon: ARC-0012. Mit --force trotzdem anlegen. [ticket-duplicate]');
  });

  it('lässt andere Meldungen unverändert', () => {
    expect(text.of(new HarnessError('conflict', 'Bitte neu lesen.'))).toBe('Bitte neu lesen.');
  });

  it('hängt für den Klartext den Code in eckigen Klammern an', () => {
    expect(text.line(new HarnessError('board-repo-mismatch', 'Falsches Board.'))).toBe('Falsches Board. [board-repo-mismatch]');
    expect(text.line(new HarnessError('dependency-open', 'ARC-0002 wartet auf ARC-0001.'))).toBe('ARC-0002 wartet auf ARC-0001. Mit --force übergehen. [dependency-open]');
  });
});
