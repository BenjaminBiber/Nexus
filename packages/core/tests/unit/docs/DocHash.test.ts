import { describe, it, expect } from 'vitest';
import { DocHash } from '../../../src/docs/DocHash.js';

describe('DocHash', () => {
  it('behandelt CRLF wie LF', () => {
    expect(DocHash.of('# A\r\neins\r\n')).toBe(DocHash.of('# A\neins\n'));
  });

  it('unterscheidet verschiedene Inhalte', () => {
    expect(DocHash.of('eins')).not.toBe(DocHash.of('zwei'));
  });

  it('übergeht ein führendes BOM', () => {
    expect(DocHash.of('﻿# A')).toBe(DocHash.of('# A'));
  });

  it('liefert 64 Hex-Zeichen', () => {
    expect(DocHash.of('eins')).toMatch(/^[0-9a-f]{64}$/);
  });
});
