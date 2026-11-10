// @vitest-environment jsdom
// Slots Button und Input: Props des Hosts werden durchgereicht, der Knopf spielt den Klick-Ton, mehrzeilig wird ein textarea
import { cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ThemeHostProvider, type ThemeHost } from '@nexus/theme-sdk';
import type { ReactNode } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ResourcesProvider } from '../src/context.js';
import { McButtonSlot } from '../src/slots/McButtonSlot.js';
import { McInput } from '../src/slots/McInput.js';

const soundUrls = { click: '/snd/click', menuOpen: '/snd/menu', hotbar: '/snd/pop' };

// Host-Attrappe: nur die Felder, die die Slots über die SDK-Hooks lesen (Theme-Einstellungen)
function fakeHost(): ThemeHost {
  return {
    nav: [],
    boards: { current: null, projects: [], boards: [], loading: false, choose: vi.fn() },
    usage: { enabled: false, available: false, reason: null, fiveHour: null, sevenDay: null },
    settings: { values: null, loading: true, update: vi.fn(async () => {}) },
    themeSettings: { values: {}, loading: false, set: vi.fn(async () => {}) },
    colorScheme: { scheme: 'dark', themeId: 'minecraft', mode: 'minecraft' },
    navigate: vi.fn(),
    openSearch: vi.fn(),
    parts: {
      search: null,
      boardSwitcher: null,
      usage: null,
      navItems: {},
      outlet: null,
      connection: null,
      status: null,
    },
  };
}

function wrapper({ children }: { children: ReactNode }) {
  return (
    <ThemeHostProvider value={fakeHost()}>
      <ResourcesProvider urls={{}} soundUrls={soundUrls}>
        {children}
      </ResourcesProvider>
    </ThemeHostProvider>
  );
}

// Abgespielte Töne als Liste der URLs; Audio gemockt, nie echter Ton
let played: string[];

beforeEach(() => {
  played = [];
  vi.stubGlobal(
    'Audio',
    vi.fn(function (this: { src: string; play: () => Promise<void> }, src: string) {
      this.src = src;
      this.play = () => {
        played.push(src);
        return Promise.resolve();
      };
    }),
  );
});

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

describe('Slot Button', () => {
  it('reicht Props durch und spielt den Klick-Ton', async () => {
    const onClick = vi.fn();
    render(
      <McButtonSlot type="submit" variant="danger" title="Löschen" aria-label="Ticket löschen" aria-pressed onClick={onClick}>
        Löschen
      </McButtonSlot>,
      { wrapper },
    );
    const button = screen.getByRole('button', { name: 'Ticket löschen', pressed: true });
    expect(button.getAttribute('type')).toBe('submit');
    expect(button.getAttribute('title')).toBe('Löschen');
    expect(button.classList.contains('mc-button')).toBe(true);
    expect(button.classList.contains('mc-button--danger')).toBe(true);
    await userEvent.click(button);
    expect(onClick).toHaveBeenCalledOnce();
    expect(played).toEqual(['/snd/click']);
  });

  it('ist deaktivierbar und kennt ghost', () => {
    render(
      <McButtonSlot disabled variant="ghost">
        Mehr
      </McButtonSlot>,
      { wrapper },
    );
    const button = screen.getByRole('button', { name: 'Mehr' });
    expect((button as HTMLButtonElement).disabled).toBe(true);
    expect(button.classList.contains('mc-button--ghost')).toBe(true);
  });

  it('kennt aria-expanded, eigene Klassen und ist standardmäßig type=button', () => {
    render(
      <McButtonSlot aria-expanded className="small">
        Mehr
      </McButtonSlot>,
      { wrapper },
    );
    const button = screen.getByRole('button', { name: 'Mehr', expanded: true });
    expect(button.getAttribute('type')).toBe('button');
    expect(button.classList.contains('small')).toBe(true);
    expect(button.classList.contains('mc-button--danger')).toBe(false);
  });
});

describe('Slot Input', () => {
  it('rendert input mit Typ und meldet Änderungen als String', async () => {
    const onChange = vi.fn();
    render(<McInput value="" onChange={onChange} type="search" placeholder="Suchen" id="q" name="q" required aria-describedby="hint" />, {
      wrapper,
    });
    const input = screen.getByPlaceholderText('Suchen');
    expect(input.getAttribute('type')).toBe('search');
    expect(input.getAttribute('id')).toBe('q');
    expect(input.getAttribute('name')).toBe('q');
    expect((input as HTMLInputElement).required).toBe(true);
    expect(input.getAttribute('aria-describedby')).toBe('hint');
    expect(input.classList.contains('mc-input')).toBe(true);
    await userEvent.type(input, 'a');
    expect(onChange).toHaveBeenCalledWith('a');
  });

  it('rendert textarea bei multiline', () => {
    render(<McInput value="x" onChange={() => {}} multiline rows={4} aria-label="Notiz" />, { wrapper });
    const area = screen.getByRole('textbox', { name: 'Notiz' });
    expect(area.tagName).toBe('TEXTAREA');
    expect(area.getAttribute('rows')).toBe('4');
    expect((area as HTMLTextAreaElement).value).toBe('x');
    expect(area.classList.contains('mc-input')).toBe(true);
    expect(area.classList.contains('mc-input--multiline')).toBe(true);
  });

  it('ist deaktivierbar und nimmt eigene Klassen an', () => {
    render(<McInput value="" onChange={() => {}} disabled className="wide" aria-label="Suche" />, { wrapper });
    const input = screen.getByRole('textbox', { name: 'Suche' });
    expect(input.getAttribute('type')).toBe('text');
    expect((input as HTMLInputElement).disabled).toBe(true);
    expect(input.classList.contains('wide')).toBe(true);
  });
});
