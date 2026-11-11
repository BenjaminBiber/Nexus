// @vitest-environment jsdom
// Slots des Themes: gleich in Manifest und main.tsx; NavIcon, Icon und Progress wählen nach Stil, StartAnimation nach gemerktem Stil
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import type { ReactElement } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render } from '@testing-library/react';
import { ThemeHostProvider } from '@nexus/theme-sdk';
import theme from '../src/main.js';
import { GtaShell } from '../src/GtaShell.js';
import { STYLE_KEY } from '../src/settings.js';
import { GtaIcon, GtaNavIcon, GtaProgress, GtaStartAnimation, STYLE_SLOTS } from '../src/slots.js';
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

  it('bis San Andreas eigene Bausteine hat, liefern beide Stile die GTA-V-Variante', () => {
    for (const style of ['v', 'sa'] as const) {
      expect(STYLE_SLOTS[style], style).toEqual({ NavIcon: VNavIcon, Icon: VIcon, Progress: VProgress, StartAnimation: VBoot });
    }
  });

  it.each(['v', 'sa'] as const)('NavIcon, Icon und Progress rendern im Stil %s', (style) => {
    const { container } = inStyle(
      style,
      <>
        <GtaNavIcon name="board" />
        <GtaIcon name="doc" />
        <GtaProgress done={1} total={2} label="Schritte" caption="Schritte" />
      </>,
    );
    expect([...container.querySelectorAll('svg.gta-v-icon')].map((svg) => svg.getAttribute('data-icon'))).toEqual(['board', 'doc']);
    expect(container.querySelector('.gta-v-progress')).not.toBeNull();
  });

  it.each(['v', 'sa'])('StartAnimation läuft ohne Host und nimmt den gemerkten Stil (%s)', (style) => {
    window.localStorage.setItem(STYLE_KEY, style);
    render(<GtaStartAnimation onDone={vi.fn()} />);
    expect(document.querySelector('[data-gta-part="boot"]')).not.toBeNull();
  });
});
