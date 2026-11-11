import { useEffect } from 'react';
import { BoardSwitcher, SearchButton, type ShellProps } from '@nexus/theme-sdk';
import { GameProvider } from './data/GameContext.js';
import { rememberBoot, rememberStyle, useGtaSettings, useRootAttributes } from './settings.js';
import { Wordmark } from './shared/Wordmark.js';

// Platzhalter-Rahmen im Schema „GTA“: oben Wortmarke, Board-Auswahl und Suche, darunter Navigation und Seite. Die Rahmen
// für GTA V und San Andreas ersetzen ihn später; Stil, Akzent und Radar landen schon jetzt als data-gta-* auf <html>.
export function GtaShell({ pid, nav, content }: ShellProps) {
  const settings = useGtaSettings();
  useRootAttributes(settings);

  // Erst nach dem Laden merken, sonst überschrieben die Defaults den gemerkten Stil
  useEffect(() => {
    if (settings.loading) return;
    rememberStyle(settings.style);
    rememberBoot(settings.boot);
  }, [settings.loading, settings.style, settings.boot]);

  return (
    <GameProvider pid={pid}>
      <div className="gta-screen" data-gta-part="shell">
        <header className="gta-head">
          <Wordmark name="nexus" label="Nexus" />
          <BoardSwitcher />
          <SearchButton />
        </header>
        <nav className="gta-nav" aria-label="Hauptnavigation">
          {nav}
        </nav>
        {content}
      </div>
    </GameProvider>
  );
}
