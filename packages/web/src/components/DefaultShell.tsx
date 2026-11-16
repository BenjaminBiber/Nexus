import type { ShellProps } from '@nexus/theme-sdk';
import { GooNav } from './goo/GooNav.js';
import { SidebarToggleIcon } from './NavIcons.js';
import { useSidebarRail } from './sidebarRail.js';

// Standard-Layout: Seitenleiste mit Suche ganz oben, darunter Board-Auswahl, dann die Navigation und ganz unten der
// Umschalter der Leiste und die Claude-Nutzung; rechts daneben der Inhalt. Eingeklappt (auf Laptop-Breite von sich aus)
// zeigt die Leiste nur Symbole und klappt beim Überfahren über den Inhalt auf, Regeln in base.css
export function DefaultShell({ nav, content, parts }: ShellProps) {
  const { collapsed, toggle } = useSidebarRail();
  return (
    <>
      <aside className={`shell-sidebar${collapsed ? ' is-rail' : ''}`}>
        <div className="shell-dock">
          {parts.search}
          <div className="dock-row">{parts.boardSwitcher}</div>
        </div>
        <GooNav className="nav" aria-label="Hauptnavigation">
          {nav}
        </GooNav>
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
      </aside>
      {content}
    </>
  );
}
