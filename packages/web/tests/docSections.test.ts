import { describe, it, expect } from 'vitest';
import { sliceLines } from '../src/markdown/docSections.js';

const text = 'eins\nzwei\ndrei\nvier';

describe('sliceLines', () => {
  it('schneidet einen Bereich 1-basiert und einschließlich aus', () => {
    expect(sliceLines(text, 2, 3)).toBe('zwei\ndrei');
  });

  it('nimmt die erste und die letzte Zeile mit', () => {
    expect(sliceLines(text, 1, 1)).toBe('eins');
    expect(sliceLines(text, 4, 4)).toBe('vier');
  });

  it('klemmt einen Bereich über das Dateiende auf die vorhandenen Zeilen', () => {
    expect(sliceLines(text, 3, 99)).toBe('drei\nvier');
    expect(sliceLines(text, 0, 2)).toBe('eins\nzwei');
  });

  it('liefert leeren Text, wenn der Bereich hinter dem Ende liegt oder verdreht ist', () => {
    expect(sliceLines(text, 10, 12)).toBe('');
    expect(sliceLines(text, 3, 2)).toBe('');
  });
});
