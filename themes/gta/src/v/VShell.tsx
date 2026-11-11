import { useRef, type ReactNode } from 'react';
import { useOpenSearch } from '@nexus/theme-sdk';
import { useGame } from '../data/GameContext.js';
import { Wordmark } from '../shared/Wordmark.js';
import { VBanner } from './VBanner.js';
import { VHud } from './VHud.js';
import { VMinimap } from './VMinimap.js';
import { VNotice } from './VNotice.js';
import { VPhone } from './VPhone.js';
import { VScene } from './VScene.js';
import { VStats } from './VStats.js';
import { VTabs } from './VTabs.js';

// Board und Seite unten rechts; nach einem Seitenwechsel kurz hervorgehoben
function Zone() {
  const { boardName, active, zone } = useGame();
  return (
    <div className="gta-v-zone" data-gta-part="zone" data-flash={zone ? '' : undefined}>
      <small className="gta-v-zone-board">{boardName ?? ''}</small>
      <span key={zone ?? ''} className="gta-v-zone-page">
        {active?.label ?? ''}
      </span>
    </div>
  );
}

// Tastenhinweise; „Suchen“ öffnet die Suche auch per Klick
function Keys() {
  const openSearch = useOpenSearch();
  return (
    <div className="gta-v-keys">
      <span>
        <kbd>Q</kbd>
        <kbd>E</kbd> Reiter
      </span>
      <button type="button" onClick={openSearch}>
        <kbd>Strg</kbd>
        <kbd>K</kbd> Suchen
      </button>
    </div>
  );
}

// Rahmen im Stil GTA V: das Pausenmenü über der Abendszene. Kopf mit Wortmarke, Hinweisfeld und HUD, darunter die
// Reiterzeile, der Inhalt des Kerns in einem eigenen Scroll-Bereich und die Fußleiste mit Minimap, Handy, Ort, Tasten und
// Stats. Banner und Stats-Menü liegen über allem; bei Busted und Wasted wird der Rahmen grau (data-dead).
export function VShell({ content }: { content: ReactNode }) {
  const game = useGame();
  const statsButton = useRef<HTMLButtonElement>(null);
  const dead = game.banner?.kind === 'busted' || game.banner?.kind === 'wasted';
  return (
    <div className="gta-frame gta-v" data-gta-part="shell" data-dead={dead ? '' : undefined}>
      <VScene />
      <div className="gta-v-frame">
        <header className="gta-v-top">
          <div className="gta-v-logo">
            <Wordmark name="nexus" label="Nexus" />
          </div>
          <VNotice />
          <VHud />
        </header>
        <VTabs />
        <div className="gta-v-main">{content}</div>
        <footer className="gta-v-bottom">
          {game.settings.radar && <VMinimap />}
          <VPhone />
          <span className="gta-v-spacer" />
          <div className="gta-v-corner">
            <Zone />
            <div className="gta-v-actions">
              <Keys />
              <button
                ref={statsButton}
                type="button"
                className="gta-v-stats-button"
                data-gta-part="stats-button"
                aria-haspopup="dialog"
                onClick={() => game.setStatsOpen(true)}
              >
                Stats
              </button>
            </div>
          </div>
        </footer>
      </div>
      <VBanner />
      {game.statsOpen && <VStats returnFocus={statsButton} />}
    </div>
  );
}
