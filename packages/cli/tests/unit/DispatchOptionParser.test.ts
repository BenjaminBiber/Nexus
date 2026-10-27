import { describe, expect, it } from 'vitest';
import { DispatchOptionParser } from '../../src/board/DispatchOptionParser.js';

describe('DispatchOptionParser', () => {
  const parser = new DispatchOptionParser();

  it('akzeptiert gültige Minuten', () => {
    expect(parser.minutes('15')).toBe(15);
    expect(parser.minutes('1440')).toBe(1440);
  });

  it.each(['abc', '0', '1441', '1.5', '-3', ''])('weist --minutes %j ab', (v) => {
    expect(() => parser.minutes(v)).toThrowError(expect.objectContaining({ code: 'input-invalid', message: 'minutes muss eine ganze Zahl von 1 bis 1440 sein.' }));
  });

  it('weist --board x ab', () => {
    expect(() => parser.boardId('x')).toThrowError(expect.objectContaining({ code: 'id-invalid' }));
    expect(parser.boardId('3')).toBe(3);
    expect(parser.boardId()).toBeUndefined();
  });

  it('bevorzugt Option vor Umgebung vor Standard', () => {
    expect(parser.port('4800', '4900')).toBe(4800);
    expect(parser.port(undefined, '4900')).toBe(4900);
    expect(() => parser.port('70000', undefined)).toThrowError(expect.objectContaining({ code: 'port-invalid' }));
  });
});
