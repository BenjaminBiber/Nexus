import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { SDK_VERSION } from '../src/index.js';
import type { ButtonProps, CardProps, DialogProps, InputProps, PanelProps, ProgressProps, ThemeDefinition, ThemeSlots } from '../src/index.js';

describe('SDK_VERSION', () => {
  it('ist eine semver-Version der Reihe 0.x', () => {
    expect(SDK_VERSION).toMatch(/^0\.\d+\.\d+$/);
  });

  it('ist 0.5.0 (Baustein-Slots)', () => {
    expect(SDK_VERSION).toBe('0.5.0');
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

  it('kennt die Baustein-Slots Button, Card, Panel, Dialog und Input', () => {
    const Button = (props: ButtonProps) => props.children;
    const Card = (props: CardProps) => props.children;
    const Panel = (props: PanelProps) => props.children;
    const Dialog = (props: DialogProps) => props.children;
    const Input = (props: InputProps) => props.value;
    const theme = {
      id: 'bausteine',
      slots: { Button: Button as never, Card: Card as never, Panel: Panel as never, Dialog: Dialog as never, Input: Input as never },
    } satisfies ThemeDefinition;
    const keys: Array<keyof ThemeSlots> = ['Button', 'Card', 'Panel', 'Dialog', 'Input'];
    expect(Object.keys(theme.slots)).toEqual(keys);
  });
});
