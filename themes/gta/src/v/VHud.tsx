import { BoardSwitcher } from '@nexus/theme-sdk';
import { useGame } from '../data/GameContext.js';
import { formatClockV, formatMoneyV } from '../shared/format.js';
import { useCountUp } from '../shared/useCountUp.js';

const STARS = 5;
const STAR = 'M12 1.8l3 6.6 7.2.8-5.4 4.9 1.5 7.1L12 17.6l-6.3 3.6 1.5-7.1L1.8 9.2 9 8.4z';

// Fünf Sterne, von rechts gefüllt; ab fünf blinken sie
function Stars({ wanted }: { wanted: number }) {
  const on = Math.max(0, Math.min(wanted, STARS));
  return (
    <div
      className="gta-v-stars"
      role="img"
      aria-label={`Fahndungslevel ${on} von ${STARS}`}
      title="Fahndungslevel = blockierte Tickets"
      data-blink={on >= STARS ? '' : undefined}
    >
      {Array.from({ length: STARS }, (_, i) => (
        <svg key={i} className={i >= STARS - on ? 'gta-v-star on' : 'gta-v-star'} viewBox="0 0 24 24" aria-hidden="true">
          <path d={STAR} />
        </svg>
      ))}
    </div>
  );
}

// Kontostand in Grün; zählt bei Änderung hoch
function Cash({ money }: { money: number }) {
  const { value, counting } = useCountUp(money);
  return (
    <div className="gta-v-cash" title="Spielgeld: verdient mit erledigten Schritten und Tickets" data-up={counting ? '' : undefined}>
      {formatMoneyV(value)}
    </div>
  );
}

// HUD oben rechts: Board-Auswahl und Uhr, daneben Sterne und Geld
export function VHud() {
  const game = useGame();
  return (
    <div className="gta-v-hud" data-gta-part="hud">
      <div className="gta-v-who">
        <div className="gta-v-board">
          <BoardSwitcher />
        </div>
        <span className="gta-v-clock">{formatClockV(game.now)}</span>
      </div>
      <div className="gta-v-score">
        <Stars wanted={game.wanted} />
        <Cash money={game.moneyShown} />
      </div>
    </div>
  );
}
