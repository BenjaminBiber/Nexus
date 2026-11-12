import type { RefObject } from 'react';
import { BoardSwitcher, NavLink, useNav, useOpenSearch } from '@nexus/theme-sdk';
import { useGame } from '../data/GameContext.js';
import { Wordmark } from '../shared/Wordmark.js';
import { SaNavIcon } from './SaNavIcon.js';

// Menüspalte im Stil San Andreas: oben die Wortmarke mit dem Board in der Schmuckschrift darunter (Board-Auswahl des
// Kerns), dann die Menüliste aus useNav() – Symbol, Beschriftung in Versalien, Zähler rechts, aktiver Eintrag in der
// Akzentfarbe mit Marke davor –, am Ende Stats und Suche.
export function SaMenu({ statsButton }: { statsButton?: RefObject<HTMLButtonElement | null> }) {
  const items = useNav();
  const { setStatsOpen } = useGame();
  const openSearch = useOpenSearch();
  return (
    <div className="gta-sa-menu">
      <div className="gta-sa-logo">
        <Wordmark name="nexus" label="Nexus" />
        <div className="gta-sa-board">
          <BoardSwitcher />
        </div>
      </div>
      <nav className="gta-sa-nav" aria-label="Hauptnavigation">
        {items.map((entry) => (
          <NavLink key={entry.id} id={entry.id} className="gta-sa-menu-item">
            {(item) => (
              <>
                <SaNavIcon name={item.id} />
                <span className="gta-sa-menu-label">{item.label}</span>
                {item.badge ? <b className="gta-sa-menu-count">{item.badge}</b> : null}
              </>
            )}
          </NavLink>
        ))}
      </nav>
      <div className="gta-sa-menu-extra">
        <button
          ref={statsButton}
          type="button"
          className="gta-sa-menu-item"
          data-gta-part="stats-button"
          aria-haspopup="dialog"
          onClick={() => setStatsOpen(true)}
        >
          <SaNavIcon name="dashboard" />
          <span className="gta-sa-menu-label">Stats</span>
        </button>
        <button type="button" className="gta-sa-menu-item" onClick={openSearch}>
          <SaNavIcon name="search" />
          <span className="gta-sa-menu-label">Suchen</span>
          <span className="gta-sa-keys">
            <kbd>Strg</kbd>
            <kbd>K</kbd>
          </span>
        </button>
      </div>
    </div>
  );
}
