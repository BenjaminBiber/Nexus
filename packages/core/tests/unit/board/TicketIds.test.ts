import { describe, expect, it } from 'vitest';
import { TicketIds } from '../../../src/board/TicketIds.js';

describe('TicketIds', () => {
  const ids = new TicketIds();

  it('normalisiert den Projektkey', () => {
    expect(ids.normalizeKey(' arc ')).toBe('ARC');
    expect(ids.normalizeKey('Inc-1')).toBe('INC1');
  });

  it.each(['1AB', 'A', 'ABCDEFG', ''])('lehnt den Key „%s“ ab', (key) => {
    expect(() => ids.normalizeKey(key)).toThrow(expect.objectContaining({ code: 'project-key-invalid' }));
  });

  it('formatiert vierstellig und wächst darüber hinaus', () => {
    expect(ids.format('ARC', 42)).toBe('ARC-0042');
    expect(ids.format('ARC', 12345)).toBe('ARC-12345');
  });

  it('zerlegt eine ID und akzeptiert Kleinschreibung', () => {
    expect(ids.parse('arc-0042')).toEqual({ key: 'ARC', number: 42 });
  });

  it('lehnt eine ungültige ID ab', () => {
    expect(() => ids.parse('42')).toThrow(expect.objectContaining({ code: 'ticket-id-invalid' }));
  });
});
