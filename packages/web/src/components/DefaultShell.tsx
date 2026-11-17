import { useRef } from 'react';
import type { ShellProps } from '@nexus/theme-sdk';
import { RailPill } from './goo/RailPill.js';
import { SidebarToggleIcon } from './NavIcons.js';
import { useRailHover } from './railHover.js';
import { useSidebarRail } from './sidebarRail.js';

// Standard-Layout: Seitenleiste mit Suche ganz oben, darunter Board-Auswahl, dann die Navigation und ganz unten der
// Umschalter der Leiste und die Claude-Nutzung; rechts daneben der Inhalt. Eingeklappt (auf Laptop-Breite von sich aus)
// zeigt die Leiste nur Symbole; sie klappt nicht auf, der überfahrene Eintrag fließt als Pille heraus (RailPill).
// Regeln in base.css
export function DefaultShell({ nav, content, parts }: ShellProps) {
  const { collapsed, toggle } = useSidebarRail();
  const aside = useRef<HTMLElement>(null);
  // In der Einstellung „Nur Symbole“ fließt keine Pille heraus, das überfahrene Symbol wird nur kräftig (base.css)
  const liquid = useRailHover() === 'liquid';
  return (
    <>
      <aside ref={aside} className={`shell-sidebar${collapsed ? ' is-rail' : ''}${collapsed && !liquid ? ' rail-icons' : ''}`}>
        <div className="shell-dock">
          {parts.search}
          <div className="dock-row">{parts.boardSwitcher}</div>
        </div>
        <nav className="nav" aria-label="Hauptnavigation">
          {nav}
        </nav>
        <div className="usage-dock">
          <button
            type="button"
            className="rail-toggle"
            aria-expanded={!collapsed}
            title={collapsed ? 'Seitenleiste dauerhaft ausklappen' : 'Seitenleiste auf Symbole einklappen'}
            onClick={toggle}
          >
            <SidebarToggleIcon collapsed={collapsed} />
            <span className="nav-label">{collapsed ? 'Ausklappen' : 'Einklappen'}</span>
          </button>
          {parts.usage}
        </div>
        {collapsed && liquid && <RailPill rail={aside} />}
      </aside>
      {content}
    </>
  );
}
