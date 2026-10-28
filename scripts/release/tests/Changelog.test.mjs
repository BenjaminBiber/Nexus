import { describe, expect, it } from 'vitest';
import { heading, release, section } from '../lib/Changelog.mjs';

const TEXT = [
  '# Changelog', '',
  '## Unveröffentlicht', '',
  '### Vertrag', '',
  '- Neue Route.', '',
  '### Sonstiges', '',
  '- Eine Kleinigkeit.', '',
  '## 0.0.9 – 2026-10-01', '',
  '- Der erste Stand.', '',
].join('\n');

describe('Changelog', () => {
  it('benennt „Unveröffentlicht“ in den Versionsabschnitt um', () => {
    const next = release(TEXT, '0.2.0', '2026-11-01');
    expect(next).toContain('## 0.2.0 – 2026-11-01');
    // Der Inhalt bleibt dort stehen, wo er war: unter der neuen Überschrift
    expect(next.indexOf('- Neue Route.')).toBeGreaterThan(next.indexOf('## 0.2.0 – 2026-11-01'));
  });

  it('setzt einen leeren Abschnitt „Unveröffentlicht“ darüber', () => {
    const next = release(TEXT, '0.2.0', '2026-11-01');
    expect(next.indexOf('## Unveröffentlicht')).toBeLessThan(next.indexOf('## 0.2.0'));
    expect(section(next, '0.2.0')).toContain('- Neue Route.');
    expect(section(next, 'Unveröffentlicht')).toBe([
      '### Vertrag', '',
      '- Keine Änderungen.', '',
      '### Angekündigt', '',
      '- Keine.', '',
      '### Sonstiges',
    ].join('\n'));
  });

  it('schreibt den Gedankenstrich U+2013 in die Überschrift', () => {
    expect(heading('0.2.0', '2026-11-01')).toBe('## 0.2.0 – 2026-11-01');
  });

  it('wirft, wenn „Unveröffentlicht“ fehlt', () => {
    expect(() => release('# Changelog\n\n## 0.0.9 – 2026-10-01\n', '0.2.0', '2026-11-01'))
      .toThrow(/fehlt die Überschrift/);
  });

  it('wirft, wenn es den Abschnitt der Version schon gibt', () => {
    expect(() => release(TEXT, '0.0.9', '2026-11-01')).toThrow(/schon ein Abschnitt/);
  });

  it('wirft bei einem Datum, das keines ist', () => {
    expect(() => release(TEXT, '0.2.0', '01.11.2026')).toThrow(/kein Datum/);
  });

  it('liefert den Abschnitt einer Version ohne Überschrift', () => {
    expect(section(TEXT, '0.0.9')).toBe('- Der erste Stand.');
  });

  it('liefert null, wenn es den Abschnitt nicht gibt', () => {
    expect(section(TEXT, '5.0.0')).toBeNull();
  });

  it('hört beim nächsten Abschnitt auf', () => {
    expect(section(TEXT, 'Unveröffentlicht')).not.toContain('Der erste Stand');
    expect(section(TEXT, 'Unveröffentlicht')).toContain('### Sonstiges');
  });

  it('arbeitet auch mit Windows-Zeilenenden', () => {
    const next = release(TEXT.replaceAll('\n', '\r\n'), '0.2.0', '2026-11-01');
    expect(next).toContain('## 0.2.0 – 2026-11-01\r\n');
    expect(next).not.toMatch(/[^\r]\n/);
  });
});
