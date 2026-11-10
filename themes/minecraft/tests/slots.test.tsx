// @vitest-environment jsdom
// Slots des Themes: Props des Hosts werden durchgereicht, der Knopf spielt den Klick-Ton, mehrzeilig wird ein textarea,
// Fortschritt ist eine XP-Leiste, Karten tragen den Listenrahmen, Bereich und Dialog den Popup-Hintergrund
import { cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ThemeHostProvider, type ThemeHost } from '@nexus/theme-sdk';
import type { ReactNode } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ResourcesProvider } from '../src/context.js';
import { McButtonSlot } from '../src/slots/McButtonSlot.js';
import { McCard } from '../src/slots/McCard.js';
import { McDialog } from '../src/slots/McDialog.js';
import { McInput } from '../src/slots/McInput.js';
import { McPanel } from '../src/slots/McPanel.js';
import { McProgress } from '../src/slots/McProgress.js';
import { McStart } from '../src/slots/McStart.js';

const soundUrls = { click: '/snd/click', menuOpen: '/snd/menu', hotbar: '/snd/pop' };

// Host-Attrappe: nur die Felder, die die Slots über die SDK-Hooks lesen (Theme-Einstellungen)
function fakeHost(): ThemeHost {
  return {
    nav: [],
    boards: { current: null, projects: [], boards: [], loading: false, choose: vi.fn() },
    usage: { enabled: false, available: false, reason: null, fiveHour: null, sevenDay: null, accounts: [], defaultLimit: 90 },
    settings: { values: null, loading: true, update: vi.fn(async () => {}) },
    themeSettings: { values: {}, loading: false, set: vi.fn(async () => {}) },
    colorScheme: { scheme: 'dark', themeId: 'minecraft', mode: 'minecraft' },
    connection: { connected: true },
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

describe('Slot Progress', () => {
  it('Progress ist eine XP-Leiste mit Zahl und Beschriftung', () => {
    const { container } = render(<McProgress done={3} total={4} label="Fortschritt der Schritte" caption="Schritte erledigt" />, {
      wrapper,
    });
    const bar = screen.getByRole('progressbar', { name: 'Fortschritt der Schritte' });
    expect(bar.getAttribute('aria-valuenow')).toBe('3');
    expect(bar.getAttribute('aria-valuemax')).toBe('4');
    expect(bar.getAttribute('aria-valuemin')).toBe('0');
    expect((container.querySelector('.mc-xp-fill') as HTMLElement).style.clipPath).toBe('inset(0 25% 0 0)');
    expect(container.querySelector('.mc-xp-level')?.textContent).toBe('3');
    expect(screen.getByText('Schritte erledigt').classList.contains('mc-shadow--gray')).toBe(true);
  });

  it('bleibt ohne Schritte leer statt zu rechnen', () => {
    const { container } = render(<McProgress done={0} total={0} label="Fortschritt" caption="keine Schritte" />, { wrapper });
    expect((container.querySelector('.mc-xp-fill') as HTMLElement).style.clipPath).toBe('inset(0 100% 0 0)');
    expect(container.querySelector('.mc-xp-level')?.textContent).toBe('0');
  });
});

describe('Slots Card, Panel und Dialog', () => {
  it('Card zeigt Titel, Aktionen und Inhalt', () => {
    render(
      <McCard title="ARC-1" actions={<button>x</button>}>
        Inhalt
      </McCard>,
      { wrapper },
    );
    expect(screen.getByText('ARC-1').classList.contains('mc-card-title')).toBe(true);
    expect(screen.getByRole('button', { name: 'x' })).toBeTruthy();
    expect(screen.getByText('Inhalt').closest('.mc-card')).not.toBeNull();
  });

  it('Card ohne Titel und Aktionen hat keinen Kopf und nimmt eigene Klassen an', () => {
    const { container } = render(<McCard className="option-card">nur Inhalt</McCard>, { wrapper });
    expect(container.querySelector('.mc-card-head')).toBeNull();
    expect(container.querySelector('.mc-card')?.classList.contains('option-card')).toBe(true);
  });

  it('Panel und Dialog liegen auf dem Popup-Hintergrund', () => {
    const { container } = render(
      <>
        <McPanel title="Darstellung">p</McPanel>
        <McDialog labelledBy="t" role="alertdialog">
          <h2 id="t">Frage</h2>
        </McDialog>
      </>,
      { wrapper },
    );
    expect(screen.getByText('Darstellung').classList.contains('mc-popup-title')).toBe(true);
    const dialog = screen.getByRole('alertdialog', { name: 'Frage' });
    expect(dialog.classList.contains('mc-popup')).toBe(true);
    expect(dialog.classList.contains('mc-dialog')).toBe(true);
    expect(dialog.style.borderImageSource).toBe('var(--mc-popup, none)');
    expect(container.querySelectorAll('.mc-popup')).toHaveLength(2);
  });
});

describe('Slot StartAnimation', () => {
  it('StartAnimation ruft onDone sofort (Platzhalter bis Phase 6)', () => {
    const onDone = vi.fn();
    const { container } = render(<McStart onDone={onDone} />, { wrapper });
    expect(onDone).toHaveBeenCalledOnce();
    expect(container.innerHTML).toBe('');
  });
});
