import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { SDK_VERSION } from '../src/index.js';
import type { ButtonProps, PageName, PageProps, PageSlots, CardProps, DialogProps, InputProps, PanelProps, ProgressProps, StartAnimationProps, ThemeDefinition, ThemeSlots } from '../src/index.js';

describe('SDK_VERSION', () => {
  it('ist eine semver-Version der Reihe 0.x', () => {
    expect(SDK_VERSION).toMatch(/^0\.\d+\.\d+$/);
  });

  it('ist 0.7.0 (Seiten-Slots)', () => {
    expect(SDK_VERSION).toBe('0.7.0');
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

  it('kennt den Slot StartAnimation mit onDone', () => {
    const StartAnimation = (props: StartAnimationProps) => (props.onDone(), null);
    const theme = { id: 'start', slots: { StartAnimation } } satisfies ThemeDefinition;
    const key: keyof ThemeSlots = 'StartAnimation';
    expect(Object.keys(theme.slots)).toEqual([key]);
  });
});

describe('Seiten-Slots', () => {
  it('kennt page:dashboard mit pid, unbekannte Seiten sind ein Typfehler', () => {
    const Dashboard = (props: PageProps) => props.pid;
    const name: PageName = 'dashboard';
    const pages: PageSlots = { [`page:${name}`]: Dashboard as never };
    const theme = { id: 'seiten', slots: { 'page:dashboard': Dashboard as never } } satisfies ThemeDefinition;
    // @ts-expect-error nur page:dashboard ist ein Seiten-Slot
    const unbekannt: keyof ThemeSlots = 'page:inbox';
    expect(Object.keys(theme.slots)).toEqual(Object.keys(pages));
    expect(unbekannt).toBe('page:inbox');
  });
});
