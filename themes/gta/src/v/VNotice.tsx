import type { CheatId } from '../shared/keys.js';
import { CHEAT_MONEY, useGame } from '../data/GameContext.js';
import { formatMoneyV } from '../shared/format.js';

const CHEAT_TEXT: Record<CheatId, string> = {
  money: `+${formatMoneyV(CHEAT_MONEY)}`,
  clear: 'Fahndungslevel aufgehoben',
  wanted: 'Fünf Sterne',
};

// Hinweisfeld neben der Wortmarke (schwarzer Kasten wie die Hilfe der Preview): Cheat-Meldung oder neues Board. Die
// Hülle bleibt stehen, damit Screenreader die Meldung als Statusänderung ansagen.
export function VNotice() {
  const { cheat, station } = useGame();
  return (
    <div className="gta-v-notice" role="status">
      {cheat ? (
        <p className="gta-v-help" data-gta-part="cheat">
          <b>Cheat aktiviert</b> – {CHEAT_TEXT[cheat]}
        </p>
      ) : station ? (
        <p className="gta-v-help" data-gta-part="station">
          Board: <b>{station}</b>
        </p>
      ) : null}
    </div>
  );
}
