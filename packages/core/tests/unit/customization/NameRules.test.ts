import { describe, expect, it } from 'vitest';
import { NameRules } from '../../../src/customization/NameRules.js';
describe('NameRules', () => {
  const rules = new NameRules();
  it('erlaubt Kleinbuchstaben, Ziffern und einzelne Bindestriche', () => {
    expect(rules.isValid('mein-review')).toBe(true);
    expect(rules.isValid('harness-implement2')).toBe(true);
  });
  it('lehnt Großbuchstaben, doppelte und randständige Bindestriche und Überlänge ab', () => {
    for (const name of ['Mein', 'a--b', '-a', 'a-', 'mit leer', 'a'.repeat(65), '']) expect(rules.isValid(name), name).toBe(false);
    expect(rules.isValid('a'.repeat(64))).toBe(true);
  });
});
