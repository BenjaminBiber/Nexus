import { describe, it, expect } from 'vitest';
import { lineDiff } from '../src/markdown/lineDiff.js';

// Kurzform der Diff-Zeilen für die Erwartungen: „=“ gleich, „+“ neu, „-“ entfernt
function shape(original: string, proposal: string): string[] {
  return lineDiff(original, proposal).map((l) => `${l.kind === 'same' ? '=' : l.kind === 'add' ? '+' : '-'}${l.text}`);
}

describe('lineDiff', () => {
  it('meldet bei gleichem Text nur unveränderte Zeilen', () => {
    const lines = lineDiff('a\nb', 'a\nb');
    expect(lines.every((l) => l.kind === 'same')).toBe(true);
    expect(lines.map((l) => l.oldLine)).toEqual([1, 2]);
    expect(lines.map((l) => l.newLine)).toEqual([1, 2]);
  });

  it('erkennt eine reine Einfügung', () => {
    expect(shape('a\nb', 'a\nneu\nb')).toEqual(['=a', '+neu', '=b']);
  });

  it('erkennt eine reine Löschung', () => {
    expect(shape('a\nweg\nb', 'a\nb')).toEqual(['=a', '-weg', '=b']);
  });

  it('behält bei einer Ersetzung in der Mitte die gemeinsamen Zeilen', () => {
    expect(shape('a\nalt\nb', 'a\nneu\nb')).toEqual(['=a', '-alt', '+neu', '=b']);
  });

  it('meldet bei leerem Vorschlag nur Löschungen', () => {
    expect(shape('a\nb', '')).toEqual(['-a', '-b', '+']);
  });

  it('zählt die Zeilennummern je Seite getrennt', () => {
    const lines = lineDiff('a\nalt\nb', 'a\nneu\nb');
    expect(lines.map((l) => [l.kind, l.oldLine, l.newLine])).toEqual([
      ['same', 1, 1],
      ['remove', 2, null],
      ['add', null, 2],
      ['same', 3, 3],
    ]);
  });
});
