import { describe, expect, it } from 'vitest';
import { QuestionIds } from '../../../src/board/QuestionIds.js';

describe('QuestionIds', () => {
  const ids = new QuestionIds();

  it('bildet und vereinheitlicht Fragenummern', () => {
    expect(ids.format('ARC', 7)).toBe('ARC-F0007');
    expect(ids.format('ARC', 12345)).toBe('ARC-F12345');
    expect(ids.canonical(' arc-f7 ')).toBe('ARC-F0007');
    expect(ids.isQuestion('ARC-F0001')).toBe(true);
    expect(ids.isQuestion('ARC-0001')).toBe(false);
    expect(ids.keyOf('arc-f0003')).toBe('ARC');
    expect(() => ids.canonical('ARC-0001')).toThrow(expect.objectContaining({ code: 'question-id-invalid' }));
  });

  it('vereinheitlicht Verweise auf Fragen oder Tickets', () => {
    expect(ids.reference('arc-f2')).toBe('ARC-F0002');
    expect(ids.reference('arc-3')).toBe('ARC-0003');
    expect(() => ids.reference('quatsch')).toThrow(expect.objectContaining({ code: 'reference-invalid' }));
  });
});
