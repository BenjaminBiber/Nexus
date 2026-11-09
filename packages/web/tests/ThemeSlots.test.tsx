// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, render, screen } from '@testing-library/react';
import { registerTheme, unregisterTheme } from '../src/theme/registry.js';
import { SlotBoundary } from '../src/theme/SlotBoundary.js';
import { useSlot } from '../src/theme/useSlot.js';
import { THEME_EVENT } from '../src/theme/theme.js';
import { MemoryRouter } from 'react-router-dom';
import { App } from '../src/App.js';
import { ApiProvider } from '../src/api/ApiContext.js';
import { BoardIcon, CogIcon, SettingsIcon } from '../src/components/NavIcons.js';
import { ProgressRing } from '../src/components/ProgressRing.js';
import type { ShellProps } from '../src/theme/slots.js';
import { fakeApi } from './helpers/fakeApi.js';

afterEach(() => {
  cleanup();
  unregisterTheme('test-slots');
  delete document.documentElement.dataset.theme;
});

// Zeigt den Namen der Progress-Komponente des angezeigten Themes oder „standard“
function ProgressProbe() {
  const Progress = useSlot('Progress');
  return <span>{Progress ? 'eigen' : 'standard'}</span>;
}

// Wirft beim Rendern (für die Error Boundary)
function Broken(): never {
  throw new Error('kaputtes Theme');
}

describe('useSlot', () => {
  it('liefert ohne registriertes Theme nichts', () => {
    render(<ProgressProbe />);
    expect(screen.getByText('standard')).toBeTruthy();
  });

  it('liefert den Slot des angezeigten Themes und folgt einem Wechsel', () => {
    registerTheme({ id: 'test-slots', slots: { Progress: () => <span>x</span> } });
    render(<ProgressProbe />);
    expect(screen.getByText('standard')).toBeTruthy();
    act(() => {
      document.documentElement.dataset.theme = 'test-slots';
      window.dispatchEvent(new CustomEvent(THEME_EVENT));
    });
    expect(screen.getByText('eigen')).toBeTruthy();
  });
});

describe('SlotBoundary', () => {
  it('zeigt bei einem Fehler der Theme-Komponente den Rückfall', () => {
    const spy = vi.spyOn(console, 'error').mockImplementation(() => {});
    render(
      <SlotBoundary fallback={<span>rueckfall</span>} resetKey="test-slots">
        <Broken />
      </SlotBoundary>,
    );
    expect(screen.getByText('rueckfall')).toBeTruthy();
    spy.mockRestore();
  });

  it('versucht es nach einem Theme-Wechsel erneut', () => {
    const spy = vi.spyOn(console, 'error').mockImplementation(() => {});
    const { rerender } = render(
      <SlotBoundary fallback={<span>rueckfall</span>} resetKey="a">
        <Broken />
      </SlotBoundary>,
    );
    rerender(
      <SlotBoundary fallback={<span>rueckfall</span>} resetKey="b">
        <span>wieder da</span>
      </SlotBoundary>,
    );
    expect(screen.getByText('wieder da')).toBeTruthy();
    spy.mockRestore();
  });
});

const SETTINGS = { notifications: 'off', heroImages: false, claudeUsage: false, compactCards: false, hideNotNeeded: false, hideInPlanning: false };

// Rendert die ganze App mit dem Test-Theme als gespeichertem Schema
function openApp(path: string) {
  document.documentElement.dataset.theme = 'test-slots';
  return render(
    <ApiProvider api={fakeApi({ getUserSettings: async () => ({ ...SETTINGS, theme: 'test-slots' }) })}>
      <MemoryRouter initialEntries={[path]}>
        <App />
      </MemoryRouter>
    </ApiProvider>,
  );
}

// Eigene Shell: Navigation oben als Leiste, Teile in fester Reihenfolge
function TopBarShell({ nav, content, parts }: ShellProps) {
  return (
    <div data-testid="top-bar-shell">
      <header>{parts.search}{parts.boardSwitcher}</header>
      <nav aria-label="Hauptnavigation">{nav}</nav>
      {content}
      <footer>{parts.usage}</footer>
    </div>
  );
}

describe('Slot „Shell“', () => {
  it('ersetzt das Layout durch die Shell des Themes', async () => {
    registerTheme({ id: 'test-slots', slots: { Shell: TopBarShell } });
    openApp('/p/p1/sessions');
    const shell = await screen.findByTestId('top-bar-shell');
    expect(shell.querySelector('.shell-sidebar')).toBeNull();
    expect(screen.getByRole('link', { name: 'Board' }).getAttribute('href')).toBe('/p/p1/board');
  });

  it('zeigt die Standard-Seitenleiste, wenn die Shell des Themes scheitert', async () => {
    const spy = vi.spyOn(console, 'error').mockImplementation(() => {});
    registerTheme({ id: 'test-slots', slots: { Shell: () => { throw new Error('kaputt'); } } });
    const { container } = openApp('/p/p1/sessions');
    await screen.findByRole('link', { name: 'Board' });
    expect(container.querySelector('.shell-sidebar')).not.toBeNull();
    spy.mockRestore();
  });
});

describe('Slot „NavIcon“', () => {
  it('ersetzt die Navigationssymbole nach Namen', () => {
    registerTheme({ id: 'test-slots', slots: { NavIcon: ({ name }) => <i data-testid={`icon-${name}`} /> } });
    document.documentElement.dataset.theme = 'test-slots';
    render(<><BoardIcon /><CogIcon /></>);
    expect(screen.getByTestId('icon-board')).toBeTruthy();
    expect(screen.getByTestId('icon-settings')).toBeTruthy();
  });

  it('lässt die Symbole der Einstellungs-Navigation unverändert', () => {
    registerTheme({ id: 'test-slots', slots: { NavIcon: () => <i data-testid="ersetzt" /> } });
    document.documentElement.dataset.theme = 'test-slots';
    const { container } = render(<SettingsIcon name="folder" />);
    expect(screen.queryByTestId('ersetzt')).toBeNull();
    expect(container.querySelector('svg.nav-icon')).not.toBeNull();
  });
});

describe('Slot „Progress“', () => {
  it('reicht Fortschritt, Bezeichnung und Beschriftung an das Theme weiter', () => {
    registerTheme({
      id: 'test-slots',
      slots: { Progress: ({ done, total, label, caption }) => <output>{`${done}/${total} ${label} ${caption}`}</output> },
    });
    document.documentElement.dataset.theme = 'test-slots';
    render(<ProgressRing done={2} total={5} />);
    expect(screen.getByText('2/5 Fortschritt der Schritte Schritte erledigt')).toBeTruthy();
  });

  it('zeigt ohne Theme-Slot den Ring', () => {
    const { container } = render(<ProgressRing done={1} total={4} />);
    expect(container.querySelector('.progress-ring')).not.toBeNull();
  });
});
