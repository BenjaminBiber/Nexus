import { useEffect } from 'react';
import { useGame } from '../data/GameContext.js';
import type { GameEvent } from '../data/events.js';
import { formatAmountSa } from '../shared/format.js';
import { Wordmark } from '../shared/Wordmark.js';

type Passed = Extract<GameEvent, { kind: 'passed' }>;
type Busted = Extract<GameEvent, { kind: 'busted' }>;

function PassedBanner({ event }: { event: Passed }) {
  return (
    <>
      <Wordmark name="missionPassedSa" label="Mission passed!" className="gta-sa-passed" />
      <div className="gta-sa-amount">{formatAmountSa(event.reward)}</div>
      <Wordmark name="respect" className="gta-sa-respect" />
      <p className="gta-sa-banner-note">
        {event.ticketId} · {event.title}
      </p>
    </>
  );
}

function BustedBanner({ event }: { event: Busted }) {
  const ticket = `${event.ticketId} · ${event.title}`;
  return (
    <>
      <Wordmark name="busted" label="Busted" className="gta-sa-big" />
      <p className="gta-sa-banner-text">{event.reason ? `${ticket}: ${event.reason}` : ticket}</p>
    </>
  );
}

function WastedBanner() {
  return (
    <>
      <Wordmark name="wasted" label="Wasted" className="gta-sa-big" />
      <p className="gta-sa-banner-text">5-Stunden-Kontingent aufgebraucht</p>
    </>
  );
}

// Banner wie in San Andreas, mitten über dem Bild: „mission passed!“ in Gold mit Betrag und „respect +“, oder „busted“
// bzw. „wasted“ über dem abgedunkelten Rahmen (data-dead an der Shell). Klick auf das Banner oder Esc schließt es (ein
// Esc, das schon ein Dialog des Kerns verarbeitet hat, nicht); außerhalb gehen Klicks durch (pointer-events in
// theme.css). Die Hülle mit role="status" bleibt stehen, damit Screenreader jedes Banner ansagen.
export function SaBanner() {
  const { banner, dismissBanner } = useGame();

  useEffect(() => {
    if (!banner) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape' && !event.defaultPrevented) dismissBanner();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [banner, dismissBanner]);

  return (
    <div className="gta-sa-banner-live" role="status">
      {banner ? (
        <div className="gta-sa-banner" data-gta-part="banner" data-kind={banner.kind}>
          <div className="gta-sa-banner-in" onClick={dismissBanner}>
            {banner.kind === 'passed' ? <PassedBanner event={banner} /> : banner.kind === 'busted' ? <BustedBanner event={banner} /> : <WastedBanner />}
          </div>
        </div>
      ) : null}
    </div>
  );
}
