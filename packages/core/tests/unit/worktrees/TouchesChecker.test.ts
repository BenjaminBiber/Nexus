import { describe, it, expect } from 'vitest';
import { TouchesChecker } from '../../../src/worktrees/TouchesChecker.js';

describe('TouchesChecker', () => {
  const checker = new TouchesChecker();

  it('erkennt gleiche Pfade als Überschneidung', () => {
    expect(checker.overlaps(['src/login'], ['src/login'])).toBe(true);
  });

  it('erkennt einen Oberordner als Überschneidung (Präfixfall)', () => {
    expect(checker.overlaps(['src'], ['src/login'])).toBe(true);
    expect(checker.overlaps(['src/a/b'], ['src/a'])).toBe(true);
  });

  it('behandelt ähnlich beginnende Namen nicht als Überschneidung', () => {
    expect(checker.overlaps(['src/log'], ['src/login'])).toBe(false);
  });

  it('ignoriert Schreibweise mit Backslash, Punkt und Schrägstrich am Ende', () => {
    expect(checker.overlaps(['./src\\login/'], ['src/login'])).toBe(true);
  });

  it('meldet keine Überschneidung bei getrennten oder leeren Bereichen', () => {
    expect(checker.overlaps(['src/a'], ['src/b'])).toBe(false);
    expect(checker.overlaps([], ['src/b'])).toBe(false);
  });
});
