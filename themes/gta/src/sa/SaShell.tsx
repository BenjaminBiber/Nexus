import { useRef, type ReactNode } from 'react';
import { useGame } from '../data/GameContext.js';
import { SaBanner } from './SaBanner.js';
import { SaHud } from './SaHud.js';
import { SaMenu } from './SaMenu.js';
import { SaPhone } from './SaPhone.js';
import { SaRadar } from './SaRadar.js';
import { SaScene } from './SaScene.js';
import { SaStats } from './SaStats.js';
import { SaSubtitle } from './SaSubtitle.js';
import { SaTitle } from './SaTitle.js';

// Name der Seite unten rechts in der gotischen Schrift; nach einem Seitenwechsel kurz eingeblendet wie ein Ortsname.
// Für Screenreader ausgeblendet: den Seitennamen nennt schon die Seite selbst.
function Zone() {
  const { active, zone } = useGame();
  return (
    <div className="gta-sa-zone" data-gta-part="zone" data-flash={zone ? '' : undefined} aria-hidden="true">
      <span key={zone ?? ''}>{active?.label ?? ''}</span>
    </div>
  );
}

// Rahmen im Stil San Andreas: der Vorort bei Sonnenuntergang hinten, links die Menüspalte mit Wortmarke, Menüliste und
// Radar, rechts oben gotischer Seitentitel und HUD-Block, darunter der Inhalt des Kerns in einem eigenen Scroll-Bereich
// und unten Untertitel und Zone. Handy, Banner und Stats-Menü liegen darüber; bei Busted und Wasted wird der Rahmen
// dunkel (data-dead).
export function SaShell({ content }: { content: ReactNode }) {
  const game = useGame();
  const statsButton = useRef<HTMLButtonElement>(null);
  const dead = game.banner?.kind === 'busted' || game.banner?.kind === 'wasted';
  return (
    <div className="gta-frame gta-sa" data-gta-part="shell" data-dead={dead ? '' : undefined} data-stats={game.statsOpen ? '' : undefined}>
      <SaScene />
      <div className="gta-sa-frame">
        <aside className="gta-sa-side">
          <SaMenu statsButton={statsButton} />
          {game.settings.radar && <SaRadar />}
        </aside>
        <header className="gta-sa-top">
          <SaTitle />
          <SaHud />
        </header>
        <div className="gta-sa-main">{content}</div>
        <footer className="gta-sa-bottom">
          <SaSubtitle />
          <Zone />
        </footer>
        {game.statsOpen && <SaStats returnFocus={statsButton} />}
      </div>
      <SaPhone />
      <SaBanner />
    </div>
  );
}
