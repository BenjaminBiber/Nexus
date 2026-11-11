// @vitest-environment jsdom
// Slots des Themes: gleich in Manifest und main.tsx; NavIcon, Icon und Progress wählen nach Stil, StartAnimation nach gemerktem Stil
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import type { ReactElement } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render } from '@testing-library/react';
import { ThemeHostProvider } from '@nexus/theme-sdk';
import theme from '../src/main.js';
import { GtaShell } from '../src/GtaShell.js';
import { STYLE_KEY } from '../src/settings.js';
import { GtaIcon, GtaNavIcon, GtaProgress, GtaStartAnimation, STYLE_SLOTS } from '../src/slots.js';
import { SaBoot } from '../src/sa/SaBoot.js';
import { SaIcon } from '../src/sa/SaIcon.js';
import { SaNavIcon } from '../src/sa/SaNavIcon.js';
import { SaProgress } from '../src/sa/SaProgress.js';
import { VBoot } from '../src/v/VBoot.js';
import { VIcon } from '../src/v/VIcon.js';
import { VNavIcon } from '../src/v/VNavIcon.js';
import { VProgress } from '../src/v/VProgress.js';
import { fakeHost } from './helpers/fakes.js';

afterEach(() => {
  cleanup();
  window.localStorage.clear();
});

const manifest = JSON.parse(readFileSync(join(import.meta.dirname, '..', 'manifest.json'), 'utf8'));
const SLOTS = ['Shell', 'NavIcon', 'Icon', 'Progress', 'StartAnimation'];

const inStyle = (style: 'v' | 'sa', ui: ReactElement) =>
  render(<ThemeHostProvider value={fakeHost({ themeSettings: { values: { style }, loading: false, set: vi.fn(async () => {}) } })}>{ui}</ThemeHostProvider>);

describe('Slots', () => {
  it('Manifest und main.tsx nennen dieselben Slots', () => {
    expect(manifest.slots).toEqual(SLOTS);
    expect(Object.keys(theme.slots)).toEqual(SLOTS);
    expect(theme.slots).toEqual({
      Shell: GtaShell,
      NavIcon: GtaNavIcon,
      Icon: GtaIcon,
      Progress: GtaProgress,
      StartAnimation: GtaStartAnimation,
    });
  });

  it('jeder Stil hat eigene Bausteine', () => {
    expect(STYLE_SLOTS.v).toEqual({ NavIcon: VNavIcon, Icon: VIcon, Progress: VProgress, StartAnimation: VBoot });
    expect(STYLE_SLOTS.sa).toEqual({ NavIcon: SaNavIcon, Icon: SaIcon, Progress: SaProgress, StartAnimation: SaBoot });
  });

  it.each(['v', 'sa'] as const)('NavIcon, Icon und Progress rendern im Stil %s dessen Variante', (style) => {
    const { container } = inStyle(
      style,
      <>
        <GtaNavIcon name="board" />
        <GtaIcon name="doc" />
        <GtaProgress done={1} total={2} label="Schritte" caption="Schritte" />
      </>,
    );
    expect([...container.querySelectorAll(`svg.gta-${style}-icon`)].map((svg) => svg.getAttribute('data-icon'))).toEqual(['board', 'doc']);
    expect(container.querySelector(`.gta-${style}-progress`)).not.toBeNull();
    const other = style === 'v' ? 'sa' : 'v';
    expect(container.querySelector(`.gta-${other}-icon, .gta-${other}-progress`)).toBeNull();
  });

  it.each(['v', 'sa'])('StartAnimation läuft ohne Host im gemerkten Stil (%s)', (style) => {
    window.localStorage.setItem(STYLE_KEY, style);
    render(<GtaStartAnimation onDone={vi.fn()} />);
    expect(document.querySelector(`[data-gta-part="boot"].gta-${style}-boot`)).not.toBeNull();
  });
});

describe('Wahl nach Stil', () => {
  // San Andreas bekommt für die Dauer eines Tests erkennbare Attrappen; so zeigt sich, welcher Stil gewählt wurde
  const real = STYLE_SLOTS.sa;
  beforeEach(() => {
    STYLE_SLOTS.sa = {
      NavIcon: () => <i>sa-navicon</i>,
      Icon: () => <i>sa-icon</i>,
      Progress: () => <i>sa-progress</i>,
      StartAnimation: () => <i>sa-boot</i>,
    };
  });
  afterEach(() => {
    STYLE_SLOTS.sa = real;
  });

  const slots = (
    <>
      <GtaNavIcon name="board" />
      <GtaIcon name="doc" />
      <GtaProgress done={1} total={2} label="Schritte" caption="Schritte" />
    </>
  );

  it('NavIcon, Icon und Progress folgen der Einstellung Stil', () => {
    expect(inStyle('sa', slots).container.textContent).toBe('sa-naviconsa-iconsa-progress');
    cleanup();
    expect(inStyle('v', slots).container.textContent).not.toContain('sa-');
  });

  it('während die Einstellungen laden, gilt der gemerkte Stil', () => {
    window.localStorage.setItem(STYLE_KEY, 'sa');
    const host = fakeHost({ themeSettings: { values: {}, loading: true, set: vi.fn(async () => {}) } });
    const { container } = render(<ThemeHostProvider value={host}>{slots}</ThemeHostProvider>);
    expect(container.textContent).toBe('sa-naviconsa-iconsa-progress');
  });

  it('StartAnimation nimmt den gemerkten Stil', () => {
    window.localStorage.setItem(STYLE_KEY, 'sa');
    expect(render(<GtaStartAnimation onDone={vi.fn()} />).container.textContent).toBe('sa-boot');
    cleanup();
    window.localStorage.setItem(STYLE_KEY, 'v');
    render(<GtaStartAnimation onDone={vi.fn()} />);
    expect(document.querySelector('[data-gta-part="boot"]')).not.toBeNull();
  });
});
