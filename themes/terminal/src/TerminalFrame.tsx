import { useEffect, useRef, useState } from 'react';
import { BoardSwitcher, UsageMeter, useNav, type ShellProps } from '@nexus/theme-sdk';
import { rememberBoot } from './BootScreen.js';
import { TermGlyph } from './glyphs.js';
import { useRootAttributes, useTerminalSettings } from './settings.js';
import { TerminalPrompt, type TerminalPromptHandle } from './TerminalPrompt.js';
import { useTerminalActions, useTerminalPath } from './useTerminalActions.js';

// Tippt jemand gerade in ein Feld oder ist ein Dialog offen? Dann gehört „/“ dorthin.
function slashBelongsElsewhere(target: EventTarget | null): boolean {
  const el = target instanceof HTMLElement ? target : null;
  if (el && (el.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(el.tagName))) return true;
  return document.querySelector('[role="dialog"], [role="alertdialog"]') !== null;
}

// Uhrzeit der Statuszeile (hh:mm), springt spätestens 15 s nach der vollen Minute weiter
function useClock(): Date {
  const [now, setNow] = useState(() => new Date());
  useEffect(() => {
    const id = window.setInterval(() => setNow(new Date()), 15_000);
    return () => window.clearInterval(id);
  }, []);
  return now;
}

// Rahmen im Schema „Terminal“: oben die Prompt-Zeile in Powerline-Segmenten mit Befehlseingabe und der Board-Auswahl,
// links die Navigation als Ausgabe von „ls --icons ~/“, rechts die Seite und unten eine Statuszeile wie in vim/tmux mit
// Bereich, Claude-Nutzung, Tasten und Uhr. Die Einstellungen des Themes landen als data-term-* auf <html>.
export function TerminalFrame({ pid, nav, content }: ShellProps) {
  const path = useTerminalPath(pid);
  const active = useNav().find((item) => item.active)?.id;
  const actions = useTerminalActions(pid);
  const prompt = useRef<TerminalPromptHandle>(null);
  const now = useClock();
  const settings = useTerminalSettings();
  useRootAttributes(settings);

  // Die Startanimation läuft vor dem Rahmen und kennt die Einstellungen nicht: Schalter für den nächsten Start merken
  useEffect(() => {
    if (!settings.loading) rememberBoot(settings.boot);
  }, [settings.loading, settings.boot]);

  // „/“ springt von überall in die Prompt-Zeile
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== '/' || event.ctrlKey || event.metaKey || event.altKey || slashBelongsElsewhere(event.target)) return;
      event.preventDefault();
      prompt.current?.focus();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, []);

  const section = active ?? (path.split('/')[0] || 'board');
  const time = now.toLocaleTimeString('de-DE', { hour: '2-digit', minute: '2-digit' });
  const date = now.toLocaleDateString('de-DE', { weekday: 'long', day: 'numeric', month: 'long', year: 'numeric' });

  return (
    <div className="term-screen">
      <header className="term-topbar">
        <TerminalPrompt ref={prompt} actions={actions} path={path} typeOutput={settings.typeOutput} />
        <div className="term-board">
          <BoardSwitcher />
        </div>
      </header>
      <div className="term-body">
        <nav className="nav term-nav" aria-label="Hauptnavigation">
          <div className="term-nav-hint" aria-hidden="true">
            {settings.icons === 'off' ? '$ ls ~/' : '$ ls --icons ~/'}
          </div>
          {nav}
        </nav>
        {content}
      </div>
      <footer className="term-statusline" data-section={section}>
        <span className="term-mode">{section.toUpperCase()}</span>
        <span className="term-status-usage">
          <UsageMeter />
        </span>
        <span className="term-spacer" />
        <span className="term-keys" aria-hidden="true">
          <kbd>/</kbd> Befehl · <kbd>Tab</kbd> ergänzen · <kbd>↑↓</kbd> Verlauf · <kbd>Strg K</kbd> Suchen
        </span>
        <time className="term-clock" dateTime={now.toISOString()} title={date}>
          <TermGlyph name="clock" />
          {time}
        </time>
      </footer>
    </div>
  );
}
