import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { SDK_VERSION } from '../src/index.js';
import type { ProgressProps, ThemeDefinition } from '../src/index.js';

describe('SDK_VERSION', () => {
  it('ist eine semver-Version der Reihe 0.x', () => {
    expect(SDK_VERSION).toMatch(/^0\.\d+\.\d+$/);
  });

  it('stimmt mit der Version in package.json überein', () => {
    const pkg = JSON.parse(readFileSync(join(import.meta.dirname, '..', 'package.json'), 'utf8')) as { name: string; version: string };
    expect(pkg.name).toBe('@nexus/theme-sdk');
    expect(SDK_VERSION).toBe(pkg.version);
  });
});

describe('Slot-Typen', () => {
  it('erlaubt ein Theme mit Progress-Slot', () => {
    const Progress = (props: ProgressProps) => `${props.done}/${props.total}`;
    const theme = { id: 'beispiel', slots: { Progress: Progress as never } } satisfies ThemeDefinition;
    expect(theme.id).toBe('beispiel');
  });
});
