import { describe, expect, it } from 'vitest';
import { bump, compare, format, highest, parse, tryParse } from '../lib/SemVer.mjs';

describe('SemVer', () => {
  it('zerlegt eine Version', () => {
    expect(parse('1.4.2')).toEqual({ major: 1, minor: 4, patch: 2, prerelease: null, build: null });
  });

  it('zerlegt Vorabversion und Build-Anhang', () => {
    expect(parse('1.4.2-rc.1+abc')).toEqual({ major: 1, minor: 4, patch: 2, prerelease: 'rc.1', build: 'abc' });
  });

  it('wirft bei ungültigen Eingaben', () => {
    for (const text of ['1.4', '1.4.2.3', 'v1.4.2', '01.4.2', '1.4.x', '', ' ', null, 7]) {
      expect(() => parse(text), String(text)).toThrow(/keine Version/);
    }
  });

  it('liefert bei tryParse null statt eines Fehlers', () => {
    expect(tryParse('keine')).toBeNull();
    expect(tryParse('1.4.2')).not.toBeNull();
  });

  it('schreibt eine zerlegte Version wieder als Text', () => {
    expect(format(parse('1.4.2-rc.1+abc'))).toBe('1.4.2-rc.1+abc');
    expect(format({ ...parse('1.4.2-rc.1+abc'), build: null })).toBe('1.4.2-rc.1');
  });

  it('erhöht patch, minor und major', () => {
    expect(bump('1.4.2', 'patch')).toBe('1.4.3');
    expect(bump('1.4.2', 'minor')).toBe('1.5.0');
    expect(bump('1.4.2', 'major')).toBe('2.0.0');
  });

  it('nimmt eine Vorabversion als Weg zu ihrer eigenen Version', () => {
    expect(bump('1.4.2-rc.1', 'patch')).toBe('1.4.2');
    expect(bump('1.5.0-rc.1', 'minor')).toBe('1.5.0');
    expect(bump('2.0.0-rc.1', 'major')).toBe('2.0.0');
    // Nur wenn die Vorabversion nicht selbst schon die gesuchte Stelle ist
    expect(bump('1.4.2-rc.1', 'minor')).toBe('1.5.0');
    expect(bump('1.4.2-rc.1', 'major')).toBe('2.0.0');
  });

  it('wirft bei einer unbekannten Stelle', () => {
    expect(() => bump('1.4.2', 'build')).toThrow(/keine Stelle/);
  });

  it('vergleicht nach Zahlen, nicht nach Text', () => {
    expect(compare('0.10.0', '0.9.0')).toBe(1);
    expect(compare('0.9.0', '0.10.0')).toBe(-1);
    expect(compare('1.2.3', '1.2.3')).toBe(0);
  });

  it('stellt eine Vorabversion unter ihre Version', () => {
    expect(compare('0.10.0-rc.1', '0.10.0')).toBe(-1);
    expect(compare('0.10.0-rc.1', '0.10.0-rc.2')).toBe(-1);
    expect(compare('0.10.0-rc.2', '0.10.0-rc.10')).toBe(-1);
    expect(compare('0.10.0-alpha', '0.10.0-beta')).toBe(-1);
    expect(compare('0.10.0-rc.1', '0.10.0-rc.1.1')).toBe(-1);
  });

  it('übergeht den Build-Anhang beim Vergleich', () => {
    expect(compare('1.0.0+a', '1.0.0+b')).toBe(0);
  });

  it('findet die höchste Version', () => {
    expect(highest(['0.9.0', '0.10.0', '0.2.0'])).toBe('0.10.0');
    expect(highest([])).toBeNull();
  });
});
