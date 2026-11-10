// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { useRef, useState } from 'react';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import type { ButtonProps, CardProps, DialogProps, InputProps, PanelProps, ThemeSlots } from '@nexus/theme-sdk';
import { registerTheme, unregisterTheme } from '../src/theme/registry.js';
import { Button } from '../src/components/ui/Button.js';
import { Card } from '../src/components/ui/Card.js';
import { Panel } from '../src/components/ui/Panel.js';
import { Input } from '../src/components/ui/Input.js';
import { Dialog } from '../src/components/ui/Dialog.js';
import { DbModal } from '../src/components/db/DbModal.js';

const ID = 'test-blocks';

afterEach(() => {
  cleanup();
  unregisterTheme(ID);
  delete document.documentElement.dataset.theme;
});

// Ersatz-Bausteine des Test-Themes; jeder zeigt sich über eine data-testid
const SLOTS: ThemeSlots = {
  Button: ({ children, onClick, variant, disabled }: ButtonProps) => (
    <button type="button" data-testid="theme-button" data-variant={variant} disabled={disabled} onClick={onClick}>
      {children}
    </button>
  ),
  Card: ({ title, children }: CardProps) => (
    <div data-testid="theme-card">
      {title}
      {children}
    </div>
  ),
  Panel: ({ children }: PanelProps) => <div data-testid="theme-panel">{children}</div>,
  Dialog: ({ children, role }: DialogProps) => (
    <div data-testid="theme-dialog" data-role={role}>
      {children}
    </div>
  ),
  Input: ({ value, onChange, multiline, 'aria-label': label }: InputProps) =>
    multiline ? (
      <textarea data-testid="theme-input" aria-label={label} value={value} onChange={(e) => onChange(e.target.value)} />
    ) : (
      <input data-testid="theme-input" aria-label={label} value={value} onChange={(e) => onChange(e.target.value)} />
    ),
};

// Meldet das Test-Theme mit den gegebenen Slots an und zeigt es an
function useTheme(slots: ThemeSlots = SLOTS) {
  registerTheme({ id: ID, slots });
  document.documentElement.dataset.theme = ID;
}

// Wirft beim Rendern (für die Error Boundary)
function Broken(): never {
  throw new Error('kaputter Baustein');
}

// Eingabefeld mit eigenem Zustand
function ControlledInput({ multiline }: { multiline?: boolean }) {
  const [value, setValue] = useState('a');
  return <Input aria-label="Feld" value={value} onChange={setValue} multiline={multiline} />;
}

describe('Baustein „Button“', () => {
  it('ohne Theme: Knopf mit den bisherigen Klassen', () => {
    const onClick = vi.fn();
    render(
      <>
        <Button onClick={onClick}>Normal</Button>
        <Button variant="primary" className="small">Haupt</Button>
        <Button variant="danger">Weg</Button>
      </>,
    );
    const normal = screen.getByRole('button', { name: 'Normal' });
    expect(normal.className).toBe('btn');
    expect(normal.getAttribute('type')).toBe('button');
    expect(normal.hasAttribute('data-slot')).toBe(false);
    fireEvent.click(normal);
    expect(onClick).toHaveBeenCalled();
    expect(screen.getByRole('button', { name: 'Haupt' }).className).toBe('btn primary small');
    expect(screen.getByRole('button', { name: 'Weg' }).className).toBe('btn danger');
  });

  it('mit Theme: der Ersatz bekommt Variante und Klick', () => {
    useTheme();
    const onClick = vi.fn();
    render(<Button variant="primary" onClick={onClick}>Los</Button>);
    const button = screen.getByTestId('theme-button');
    expect(button.dataset.variant).toBe('primary');
    fireEvent.click(button);
    expect(onClick).toHaveBeenCalled();
  });

  it('wirft der Ersatz, erscheint der Standard und ein Fehler in der Konsole', () => {
    const spy = vi.spyOn(console, 'error').mockImplementation(() => {});
    useTheme({ Button: Broken });
    render(<Button>Sicher</Button>);
    expect(screen.getByRole('button', { name: 'Sicher' }).className).toBe('btn');
    expect(spy).toHaveBeenCalled();
    spy.mockRestore();
  });
});

describe('Baustein „Input“', () => {
  it('ohne Theme: input bzw. textarea, Eingabe kommt als Text an', () => {
    render(
      <>
        <ControlledInput />
        <Input aria-label="Mehrzeilig" value="" onChange={() => {}} multiline className="wide" />
      </>,
    );
    const field = screen.getByRole('textbox', { name: 'Feld' }) as HTMLInputElement;
    expect(field.tagName).toBe('INPUT');
    fireEvent.change(field, { target: { value: 'neu' } });
    expect(field.value).toBe('neu');
    const area = screen.getByRole('textbox', { name: 'Mehrzeilig' });
    expect(area.tagName).toBe('TEXTAREA');
    expect(area.className).toBe('wide');
  });

  it('mit Theme: der Ersatz bekommt Wert und Änderung', () => {
    useTheme();
    render(<ControlledInput multiline />);
    const field = screen.getByTestId('theme-input') as HTMLTextAreaElement;
    expect(field.tagName).toBe('TEXTAREA');
    fireEvent.change(field, { target: { value: 'b' } });
    expect(field.value).toBe('b');
  });

  it('wirft der Ersatz, erscheint der Standard', () => {
    const spy = vi.spyOn(console, 'error').mockImplementation(() => {});
    useTheme({ Input: Broken });
    render(<ControlledInput />);
    expect(screen.getByRole('textbox', { name: 'Feld' }).tagName).toBe('INPUT');
    expect(spy).toHaveBeenCalled();
    spy.mockRestore();
  });
});

describe('Baustein „Card“', () => {
  it('ohne Theme: unverändertes Element mit Klasse, Inhalt direkt darin', () => {
    const { container } = render(
      <Card as="article" className="card db-card" data-id="T-1">
        <span>Inhalt</span>
      </Card>,
    );
    expect(container.innerHTML).toBe('<article class="card db-card" data-id="T-1"><span>Inhalt</span></article>');
  });

  it('mit Theme: das Element bleibt, das Innere kommt vom Theme', () => {
    useTheme();
    const { container } = render(
      <Card as="article" className="option-card" title={<b>Titel</b>}>
        <span>Inhalt</span>
      </Card>,
    );
    const host = container.querySelector('article.option-card') as HTMLElement;
    expect(host.dataset.slot).toBe('Card');
    expect(screen.getByTestId('theme-card').textContent).toBe('TitelInhalt');
  });

  it('wirft der Ersatz, erscheint der Inhalt wie ohne Theme', () => {
    const spy = vi.spyOn(console, 'error').mockImplementation(() => {});
    useTheme({ Card: Broken });
    render(<Card className="card"><span>Rückfall</span></Card>);
    expect(screen.getByText('Rückfall')).toBeTruthy();
    expect(spy).toHaveBeenCalled();
    spy.mockRestore();
  });
});

describe('Baustein „Panel“', () => {
  it('ohne Theme: unverändertes Element', () => {
    const { container } = render(
      <Panel as="section" className="settings-panel" aria-label="Token">
        <p>x</p>
      </Panel>,
    );
    expect(container.innerHTML).toBe('<section class="settings-panel" aria-label="Token"><p>x</p></section>');
  });

  it('mit Theme: Ersatz im Element', () => {
    useTheme();
    render(<Panel as="section" className="settings-panel" aria-label="Token"><p>x</p></Panel>);
    const host = screen.getByRole('region', { name: 'Token' });
    expect(host.dataset.slot).toBe('Panel');
    expect(host.querySelector('[data-testid="theme-panel"] p')).not.toBeNull();
  });

  it('wirft der Ersatz, erscheint der Standard', () => {
    const spy = vi.spyOn(console, 'error').mockImplementation(() => {});
    useTheme({ Panel: Broken });
    render(<Panel className="panel"><p>Rückfall</p></Panel>);
    expect(screen.getByText('Rückfall')).toBeTruthy();
    expect(spy).toHaveBeenCalled();
    spy.mockRestore();
  });
});

describe('Baustein „Dialog“', () => {
  it('ohne Theme: eigenständiger Dialog mit Rolle und Namen, Markup unverändert', () => {
    const { container } = render(
      <Dialog role="alertdialog" labelledBy="t" className="conflict">
        <strong id="t">Konflikt</strong>
      </Dialog>,
    );
    expect(screen.getByRole('alertdialog', { name: 'Konflikt' })).toBeTruthy();
    expect(container.innerHTML).toBe('<div class="conflict" role="alertdialog" aria-labelledby="t"><strong id="t">Konflikt</strong></div>');
  });

  it('mit Theme: Rolle und Name bleiben am Element, die Hülle kommt vom Theme', () => {
    useTheme();
    render(
      <Dialog role="alertdialog" labelledBy="t" className="conflict">
        <strong id="t">Konflikt</strong>
      </Dialog>,
    );
    const dialog = screen.getByRole('alertdialog', { name: 'Konflikt' });
    expect(dialog.dataset.slot).toBe('Dialog');
    expect(screen.getByTestId('theme-dialog').dataset.role).toBe('alertdialog');
  });

  it('wirft der Ersatz in DbModal, erscheint der Inhalt wie ohne Theme', () => {
    const spy = vi.spyOn(console, 'error').mockImplementation(() => {});
    useTheme({ Dialog: Broken });
    render(
      <DbModal labelledBy="m" onEscape={() => {}}>
        <h2 id="m">Modal</h2>
      </DbModal>,
    );
    expect(screen.getByRole('dialog', { name: 'Modal' })).toBeTruthy();
    expect(spy).toHaveBeenCalled();
    spy.mockRestore();
  });
});

// Öffnet über einen Knopf ein DbModal mit zwei Knöpfen; Esc schließt
function ModalHarness({ onEscape }: { onEscape: () => void }) {
  const [open, setOpen] = useState(false);
  const first = useRef<HTMLButtonElement>(null);
  return (
    <>
      <button type="button" onClick={() => setOpen(true)}>Öffnen</button>
      {open && (
        <DbModal
          labelledBy="dlg"
          onEscape={() => {
            onEscape();
            setOpen(false);
          }}
        >
          <h2 id="dlg">Dialog</h2>
          <button type="button" ref={first}>Erster</button>
          <button type="button">Letzter</button>
        </DbModal>
      )}
    </>
  );
}

describe('DbModal mit ersetztem Dialog', () => {
  it('rendert die Hülle des Themes; Fokusfalle, Esc und Fokusrückgabe bleiben', () => {
    useTheme();
    const onEscape = vi.fn();
    render(<ModalHarness onEscape={onEscape} />);
    const opener = screen.getByRole('button', { name: 'Öffnen' });
    opener.focus();
    fireEvent.click(opener);

    const dialog = screen.getByRole('dialog', { name: 'Dialog' });
    expect(dialog.dataset.slot).toBe('Dialog');
    expect(dialog.querySelector('[data-testid="theme-dialog"]')).not.toBeNull();
    const first = screen.getByRole('button', { name: 'Erster' });
    const last = screen.getByRole('button', { name: 'Letzter' });
    expect(document.activeElement).toBe(first);

    // Tab am letzten Element springt zum ersten, Umschalt+Tab am ersten zum letzten
    last.focus();
    fireEvent.keyDown(last, { key: 'Tab' });
    expect(document.activeElement).toBe(first);
    fireEvent.keyDown(first, { key: 'Tab', shiftKey: true });
    expect(document.activeElement).toBe(last);

    fireEvent.keyDown(last, { key: 'Escape' });
    expect(onEscape).toHaveBeenCalled();
    expect(screen.queryByRole('dialog')).toBeNull();
    expect(document.activeElement).toBe(opener);
  });

  it('ohne Theme: Markup des Dialogs unverändert', () => {
    render(
      <DbModal labelledBy="m" onEscape={() => {}}>
        <h2 id="m">Modal</h2>
      </DbModal>,
    );
    const dialog = screen.getByRole('dialog', { name: 'Modal' });
    expect(dialog.className).toBe('panel db-modal');
    expect(dialog.hasAttribute('data-slot')).toBe(false);
    expect(dialog.innerHTML).toBe('<h2 id="m">Modal</h2>');
  });
});
