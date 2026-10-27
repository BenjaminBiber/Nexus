import { describe, expect, it } from 'vitest';
import { initials } from '../src/components/db/DbCommentComposer.js';
import { isLong } from '../src/components/Collapsible.js';

describe('initials', () => {
  it('nimmt zwei Wörter oder die ersten zwei Buchstaben', () => {
    expect(initials('Florian Pallmann')).toBe('FP');
    expect(initials('florian.pallmann')).toBe('FP');
    expect(initials('admin')).toBe('AD');
    expect(initials(undefined)).toBe('?');
  });
});

describe('isLong', () => {
  it('klappt ab 280 Zeichen oder mehr als 5 Zeilen ein', () => {
    expect(isLong('x'.repeat(280))).toBe(false);
    expect(isLong('x'.repeat(281))).toBe(true);
    expect(isLong('a\nb\nc\nd\ne')).toBe(false);
    expect(isLong('a\nb\nc\nd\ne\nf')).toBe(true);
  });
});
