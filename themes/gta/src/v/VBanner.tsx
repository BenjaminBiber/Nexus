import { useGame } from '../data/GameContext.js';
import type { GameEvent } from '../data/events.js';
import { useBannerEscape } from '../shared/banners.js';
import { formatMoneyV } from '../shared/format.js';
import { Wordmark } from '../shared/Wordmark.js';

type Passed = Extract<GameEvent, { kind: 'passed' }>;
type Busted = Extract<GameEvent, { kind: 'busted' }>;

function Result({ label, value }: { label: string; value: string }) {
  return (
    <div className="gta-v-result">
      <span>{label}</span>
      <b>{value}</b>
      <i className="gta-v-check" aria-hidden="true" />
    </div>
  );
}

function PassedBand({ event }: { event: Passed }) {
  return (
    <>
      <Wordmark name="missionPassed" label="Mission Passed" className="gta-v-big" />
      <p className="gta-v-banner-title">{event.title}</p>
      <div className="gta-v-results">
        <Result label="Auftrag" value={event.ticketId} />
        <Result label="Belohnung" value={formatMoneyV(event.reward)} />
      </div>
      <div className="gta-v-medal" aria-hidden="true">
        GOLD
      </div>
      <Wordmark name="respect" className="gta-v-respect" />
    </>
  );
}

function BustedBand({ event }: { event: Busted }) {
  return (
    <>
      <Wordmark name="busted" label="Busted" className="gta-v-big" />
      <p className="gta-v-banner-title">{event.title}</p>
      {event.reason ? <p className="gta-v-banner-text">{event.reason}</p> : null}
    </>
  );
}

function WastedBand() {
  return (
    <>
      <Wordmark name="wasted" label="Wasted" className="gta-v-big" />
      <p className="gta-v-banner-text">5-Stunden-Kontingent aufgebraucht</p>
    </>
  );
}

// Banner über dem Inhalt wie in GTA V: ein dunkles Band quer über die Mitte. Klick auf das Band oder Esc schließt es
// (ein Esc, das schon ein Dialog des Kerns verarbeitet hat, nicht); außerhalb des Bands gehen Klicks durch
// (pointer-events in theme.css). Die Hülle mit role="status" bleibt stehen, damit Screenreader jedes Banner ansagen.
export function VBanner() {
  const { banner, dismissBanner } = useGame();
  useBannerEscape(banner, dismissBanner);

  return (
    <div className="gta-v-banner-live" role="status">
      {banner ? (
        <div className="gta-v-banner" data-gta-part="banner" data-kind={banner.kind}>
          <div className="gta-v-band" onClick={dismissBanner}>
            {banner.kind === 'passed' ? <PassedBand event={banner} /> : banner.kind === 'busted' ? <BustedBand event={banner} /> : <WastedBand />}
          </div>
        </div>
      ) : null}
    </div>
  );
}
