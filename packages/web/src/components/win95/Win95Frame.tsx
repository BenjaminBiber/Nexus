import { useEffect, useRef, useState, type KeyboardEvent } from 'react';
import { BoardSwitcher, NavLink, UsageMeter, useNav, useOpenSearch, type NavIconName, type ShellProps } from '@nexus/theme-sdk';
import { WIN95_NAV_ICONS, Win95Icon } from './Win95Icons.js';
import { windowTitle } from './sectionTitles.js';

/** Schlüssel im localStorage: Fenster maximiert (Standard) oder als Fenster auf dem Desktop */
export const WIN95_MAXIMIZED_KEY = 'harness-w95-maximized';

const START_LINKS: Array<[NavIconName, string]> = [
  ['board', 'Board'],
  ['dashboard', 'Dashboard'],
  ['sessions', 'Sessions'],
  ['guide', 'Anleitung'],
  ['settings', 'Einstellungen'],
];

// Maximiert-Merker lesen (ohne localStorage: maximiert)
function readMaximized(): boolean {
  try {
    return window.localStorage.getItem(WIN95_MAXIMIZED_KEY) !== 'false';
  } catch {
    return true;
  }
}

// Uhrzeit der Taskleiste (hh:mm), springt zur vollen Minute weiter
function useClock(): Date {
  const [now, setNow] = useState(() => new Date());
  useEffect(() => {
    const id = window.setInterval(() => setNow(new Date()), 15_000);
    return () => window.clearInterval(id);
  }, []);
  return now;
}

// Windows-Flagge aus vier gewellten Feldern (Farben aus win95.css)
function WindowsFlag() {
  return (
    <svg className="w95-flag" viewBox="0 0 16 16" aria-hidden="true">
      <path className="w95-flag-red" d="M1 3l2-1 3 1v5L3 7 1 8z" />
      <path className="w95-flag-green" d="M7 3l3-1 3 1-1 5-3-1-2 1z" />
      <path className="w95-flag-blue" d="M1 9l2-1 3 1v5l-3-1-2 1z" />
      <path className="w95-flag-yellow" d="M7 9l2-1 3 1-1 5-2-1-2 1z" />
    </svg>
  );
}

// Programmsymbol in der Titelleiste und auf dem Task-Knopf
function AppIcon() {
  return (
    <svg className="w95-app-icon" viewBox="0 0 16 16" aria-hidden="true">
      <path className="w95-app-frame" d="M1 2h14v12H1z" />
      <path className="w95-app-bar" d="M1 2h14v3H1z" />
      <path className="w95-app-col" d="M3 7h4v5H3zM9 7h4v3H9z" />
    </svg>
  );
}

// Startmenü über dem Start-Knopf: blaue Seitenleiste, Bereiche der Harness und „Suchen“. Pfeiltasten wandern, Esc schließt.
function StartMenu({ active, onClose }: { active?: NavIconName; onClose: (focusStart: boolean) => void }) {
  const menu = useRef<HTMLDivElement>(null);
  const openSearch = useOpenSearch();

  useEffect(() => {
    menu.current?.querySelector<HTMLElement>('[role="menuitem"]')?.focus();
  }, []);

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const items = Array.from(menu.current?.querySelectorAll<HTMLElement>('[role="menuitem"]') ?? []);
    const index = items.indexOf(document.activeElement as HTMLElement);
    if (event.key === 'Escape') {
      event.preventDefault();
      onClose(true);
    } else if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault();
      const step = event.key === 'ArrowDown' ? 1 : -1;
      items[(index + step + items.length) % items.length]?.focus();
    }
  };

  return (
    <div ref={menu} className="w95-start-menu" role="menu" aria-label="Start" onKeyDown={onKeyDown}>
      <div className="w95-start-banner" aria-hidden="true">
        <b>AI Harness</b>95
      </div>
      <div className="w95-start-items">
        {START_LINKS.map(([id, label]) => (
          <NavLink key={id} id={id} role="menuitem" className={`w95-start-item${active === id ? ' active' : ''}`} onClick={() => onClose(false)}>
            {() => (
              <>
                <Win95Icon name={WIN95_NAV_ICONS[id]} />
                <span>{label}</span>
              </>
            )}
          </NavLink>
        ))}
        <hr className="w95-start-sep" />
        <button
          type="button"
          role="menuitem"
          className="w95-start-item"
          onClick={() => {
            onClose(false);
            openSearch();
          }}
        >
          <Win95Icon name={WIN95_NAV_ICONS.search} />
          <span>Suchen …</span>
        </button>
      </div>
    </div>
  );
}

// Rahmen im Schema „Windows 95“: Desktop mit Symbolen (Navigation), die Seite als Fenster mit Titelleiste (Minimieren,
// Maximieren/Wiederherstellen, Schließen = minimieren), Adressleiste mit Board-Auswahl und Statusleiste mit der Claude-Nutzung;
// unten die Taskleiste mit Start-Knopf, Task-Knopf des Fensters und Uhr.
export function Win95Frame({ nav, content }: ShellProps) {
  const active = useNav().find((item) => item.active)?.id;
  const title = windowTitle(active);
  const [maximized, setMaximized] = useState(readMaximized);
  const [minimized, setMinimized] = useState(false);
  const [startOpen, setStartOpen] = useState(false);
  const start = useRef<HTMLButtonElement>(null);
  const startArea = useRef<HTMLDivElement>(null);
  const now = useClock();

  // Wechsel des Bereichs holt das Fenster zurück
  useEffect(() => setMinimized(false), [active]);

  // Klick außerhalb schließt das Startmenü
  useEffect(() => {
    if (!startOpen) return;
    const onDown = (event: PointerEvent) => {
      if (!startArea.current?.contains(event.target as Node)) setStartOpen(false);
    };
    document.addEventListener('pointerdown', onDown);
    return () => document.removeEventListener('pointerdown', onDown);
  }, [startOpen]);

  const toggleMaximized = () => {
    setMaximized((value) => {
      try {
        window.localStorage.setItem(WIN95_MAXIMIZED_KEY, String(!value));
      } catch {
        // ohne localStorage gilt die Wahl nur bis zum Neuladen
      }
      return !value;
    });
  };

  const closeStart = (focusStart: boolean) => {
    setStartOpen(false);
    if (focusStart) start.current?.focus();
  };

  const time = now.toLocaleTimeString('de-DE', { hour: '2-digit', minute: '2-digit' });
  const date = now.toLocaleDateString('de-DE', { weekday: 'long', day: 'numeric', month: 'long', year: 'numeric' });

  return (
    <div className="w95-screen">
      <div className="w95-desktop">
        <nav className="nav w95-icons" aria-label="Hauptnavigation">
          {nav}
        </nav>
        <section
          className={`w95-window${maximized ? ' maximized' : ''}`}
          aria-label={`AI Harness – ${title}`}
          hidden={minimized}
        >
          <div className="w95-titlebar" onDoubleClick={toggleMaximized}>
            <AppIcon />
            <span className="w95-title">AI Harness - {title}</span>
            <div className="w95-title-buttons">
              <button type="button" className="w95-title-btn w95-min" aria-label="Minimieren" title="Minimieren" onClick={() => setMinimized(true)} />
              <button
                type="button"
                className={`w95-title-btn ${maximized ? 'w95-restore' : 'w95-max'}`}
                aria-label={maximized ? 'Wiederherstellen' : 'Maximieren'}
                title={maximized ? 'Wiederherstellen' : 'Maximieren'}
                onClick={toggleMaximized}
              />
              <button type="button" className="w95-title-btn w95-close" aria-label="Schließen" title="Schließen" onClick={() => setMinimized(true)} />
            </div>
          </div>
          <div className="w95-addressbar">
            <span className="w95-address-label">Adresse</span>
            <BoardSwitcher />
          </div>
          {content}
          <footer className="w95-statusbar">
            <span className="w95-status-field">{title}</span>
            <span className="w95-status-field w95-status-usage">
              <UsageMeter />
            </span>
          </footer>
        </section>
      </div>
      <footer className="w95-taskbar">
        <div ref={startArea} className="w95-start-area">
          <button
            ref={start}
            type="button"
            className="w95-start"
            aria-haspopup="menu"
            aria-expanded={startOpen}
            onClick={() => setStartOpen((value) => !value)}
          >
            <WindowsFlag />
            Start
          </button>
          {startOpen && <StartMenu active={active} onClose={closeStart} />}
        </div>
        <button
          type="button"
          className={`w95-task${minimized ? '' : ' active'}`}
          aria-pressed={!minimized}
          onClick={() => setMinimized((value) => !value)}
        >
          <AppIcon />
          <span>AI Harness - {title}</span>
        </button>
        <div className="w95-tray">
          <time className="w95-clock" dateTime={now.toISOString()} title={date}>
            {time}
          </time>
        </div>
      </footer>
    </div>
  );
}
