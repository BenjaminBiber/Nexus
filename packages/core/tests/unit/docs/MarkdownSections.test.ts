import { describe, it, expect } from 'vitest';
import { MarkdownSections } from '../../../src/docs/MarkdownSections.js';
import { HarnessError } from '../../../src/errors/HarnessError.js';

const DOC = [
  'Vorspann ohne Überschrift', // 1
  '# Titel',                   // 2
  'eins',                      // 3
  '## Ziel',                   // 4
  'zwei',                      // 5
  '### Teilziel',              // 6
  'drei',                      // 7
  '## Schritte',               // 8
  'vier',                      // 9
].join('\n');

describe('MarkdownSections', () => {
  const sections = new MarkdownSections();

  it('erkennt Ebenen und Grenzen', () => {
    expect(sections.parse(DOC)).toEqual([
      { heading: 'Titel', level: 1, startLine: 2, endLine: 9 },
      { heading: 'Ziel', level: 2, startLine: 4, endLine: 7 },
      { heading: 'Teilziel', level: 3, startLine: 6, endLine: 7 },
      { heading: 'Schritte', level: 2, startLine: 8, endLine: 9 },
    ]);
  });

  it('lässt die letzte Section bis zum Dateiende laufen', () => {
    const parsed = sections.parse('# A\neins\nzwei\n');
    expect(parsed).toEqual([{ heading: 'A', level: 1, startLine: 1, endLine: 4 }]);
  });

  it('übergeht Überschriften in Code-Zäunen', () => {
    const text = ['# Echt', '```', '# Unecht', '```', '~~~', '```', '## Auch unecht', '~~~', '## Wieder echt'].join('\n');
    expect(sections.parse(text).map((s) => s.heading)).toEqual(['Echt', 'Wieder echt']);
  });

  it('findet eine Section über ihre Überschrift', () => {
    expect(sections.find(DOC, 'Ziel')?.startLine).toBe(4);
    expect(sections.find(DOC, 'Fehlt')).toBeNull();
  });

  it('ersetzt genau den Zeilenbereich', () => {
    const out = sections.replaceLines(DOC, 4, 7, '## Ziel\nneu\nnoch neu');
    expect(out.split('\n')).toEqual([
      'Vorspann ohne Überschrift', '# Titel', 'eins', '## Ziel', 'neu', 'noch neu', '## Schritte', 'vier',
    ]);
  });

  it('behält CRLF und den abschließenden Umbruch', () => {
    const crlf = '# A\r\neins\r\n';
    expect(sections.replaceLines(crlf, 2, 2, 'zwei')).toBe('# A\r\nzwei\r\n');
  });

  it('wirft bei ungültigem Bereich', () => {
    expect(() => sections.replaceLines(DOC, 0, 2, 'x')).toThrow(HarnessError);
    expect(() => sections.replaceLines(DOC, 3, 2, 'x')).toThrow(/doc-range-invalid|Zeilenbereich/);
    try {
      sections.replaceLines(DOC, 1, 99, 'x');
      expect.unreachable();
    } catch (error) {
      expect((error as HarnessError).code).toBe('doc-range-invalid');
    }
  });
});
