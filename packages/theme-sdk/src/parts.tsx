import type { MouseEvent, ReactNode } from 'react';
import { useHost } from './hooks.js';
import type { NavItem } from './host.js';
import type { NavIconName } from './slots.js';

/** Fertige Bausteine des Rahmens */
export const useShellParts = () => useHost('useShellParts').parts;

/** Link eines Eintrags der Hauptnavigation; `children` als Funktion ersetzt das Aussehen, unbekannte ID rendert nichts */
export function NavLink({
  id,
  className,
  role,
  onClick: onClickProp,
  children,
}: {
  id: NavIconName;
  className?: string;
  /** Rolle des Ankers, z. B. `menuitem` in einem Menü */
  role?: string;
  /** Läuft bei jedem Klick vor dem Routenwechsel */
  onClick?: (event: MouseEvent<HTMLAnchorElement>) => void;
  children?: (item: NavItem) => ReactNode;
}) {
  const host = useHost('NavLink');
  const item = host.nav.find((n) => n.id === id);
  if (!item) return null;
  // Nur ein einfacher Linksklick wechselt im Host die Route; Strg-, Umschalt- oder Mittelklick bleiben beim Browser
  const onClick = (event: MouseEvent<HTMLAnchorElement>) => {
    onClickProp?.(event);
    if (event.button !== 0 || event.ctrlKey || event.metaKey || event.shiftKey || event.altKey) return;
    event.preventDefault();
    host.navigate(item.href);
  };
  return (
    <a href={item.href} className={className} role={role} aria-current={item.active ? 'page' : undefined} data-active={item.active} onClick={onClick}>
      {children ? children(item) : (
        <>
          <span className="nav-label">{item.label}</span>
          {item.badge ? <span className="nav-badge">{item.badge}</span> : null}
        </>
      )}
    </a>
  );
}

/** Board-Auswahl des Hosts */
export const BoardSwitcher = () => <>{useHost('BoardSwitcher').parts.boardSwitcher}</>;
/** Knopf der globalen Suche des Hosts */
export const SearchButton = () => <>{useHost('SearchButton').parts.search}</>;
/** Claude-Verbrauch des Hosts */
export const UsageMeter = () => <>{useHost('UsageMeter').parts.usage}</>;
/** Aktuelle Seite ohne Standard-Wrapper */
export const Outlet = () => <>{useHost('Outlet').parts.outlet}</>;
/** Verbindungspunkt der Live-Verbindung */
export const ConnectionStatus = () => <>{useHost('ConnectionStatus').parts.status}</>;
