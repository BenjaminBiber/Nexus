import type { ShellProps } from '../theme/slots.js';
import { GooNav } from './goo/GooNav.js';

// Standard-Layout: Seitenleiste mit Suche ganz oben, darunter Board-Auswahl, dann die Navigation und ganz unten die
// Claude-Nutzung; rechts daneben der Inhalt
export function DefaultShell({ nav, content, parts }: ShellProps) {
  return (
    <>
      <aside className="shell-sidebar">
        <div className="shell-dock">
          {parts.search}
          <div className="dock-row">{parts.boardSwitcher}</div>
        </div>
        <GooNav className="nav" aria-label="Hauptnavigation">
          {nav}
        </GooNav>
        <div className="usage-dock">{parts.usage}</div>
      </aside>
      {content}
    </>
  );
}
