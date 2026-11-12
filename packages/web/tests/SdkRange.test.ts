import { describe, expect, it } from 'vitest';
import { satisfiesSdkRange } from '../src/theme/sdkRange.js';

describe('semver-Bereich der SDK-Version', () => {
  it.each([
    ['0.2.0', '^0.2', true],
    ['0.2.0', '^0.2.1', false],
    ['0.2.0', '^0.3', false],
    ['0.2.0', '^1.0', false],
    ['0.2.0', '~0.2.0', true],
    ['0.2.0', '0.2.0', true],
    ['0.2.0', '>=0.1.0', true],
    ['0.2.0', '*', true],
    ['0.2.0', 'Unsinn', false],
    ['0.2.0', '', false],
    ['1.5.0', '^1.2', true],
    ['1.5.0', '1.5.0', true],
    ['1.5.0', '^2', false],
    ['1.5.0', '~1.4', false],
    ['1.5.0', '>=1.0.0 <2.0.0', true],
    ['1.5.0', '>1.5.0', false],
    ['1.5.0', '<=1.5', true],
    ['0.0.3', '^0.0.3', true],
    ['0.0.4', '^0.0.3', false],
  ])('%s erfüllt „%s“: %s', (version, range, expected) => {
    expect(satisfiesSdkRange(version, range)).toBe(expected);
  });
});
