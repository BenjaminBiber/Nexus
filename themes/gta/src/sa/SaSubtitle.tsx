import { CHEAT_MONEY, useGame } from '../data/GameContext.js';
import { formatAmountSa } from '../shared/format.js';
import type { CheatId } from '../shared/keys.js';

const CHEAT_TEXT: Record<CheatId, string> = {
  money: `+${formatAmountSa(CHEAT_MONEY)}`,
  clear: 'Fahndungslevel aufgehoben',
  wanted: 'Sechs Sterne',
};

// Untertitel-Zeile unten mittig wie die Dialogzeilen der Preview: Cheat-Meldung oder der Spruch nach einem abgelehnten
// Anruf. Beides verschwindet von selbst (die Zeiten hält der GameProvider); die Hülle bleibt stehen und sagt jede neue
// Zeile höflich an.
export function SaSubtitle() {
  const { cheat, declined } = useGame();
  return (
    <div className="gta-sa-sub" aria-live="polite">
      {cheat ? (
        <p className="gta-sa-sub-line gta-sa-sub-cheat" data-gta-part="cheat">
          <b className="gta-sa-sub-who">Cheat aktiviert</b> – {CHEAT_TEXT[cheat]}
        </p>
      ) : declined ? (
        <p className="gta-sa-sub-line">
          <b className="gta-sa-sub-who">Du</b>: Später. Die Frage wartet unter „Fragen“.
        </p>
      ) : null}
    </div>
  );
}
