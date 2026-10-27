import { useLiveConnection } from '../live/useLive.js';

// Hinweis bei Verbindungsverlust: Zeitpunkt des letzten Stands und Möglichkeit, neu zu verbinden.
export function ConnectionBanner() {
  const { state, lastSync, reconnect } = useLiveConnection();
  if (state !== 'disconnected') return null;
  const time = lastSync?.toLocaleTimeString('de-DE', { hour: 'numeric', minute: '2-digit' });
  return (
    <div className="banner connection" role="alert">
      <strong>Server nicht erreichbar</strong>
      <span>
        {time ? `Die Seite zeigt den Stand von ${time} Uhr. ` : ''}Änderungen werden erst gespeichert, wenn die Verbindung wieder steht.
      </span>
      <button type="button" className="btn" onClick={reconnect}>
        Erneut verbinden
      </button>
    </div>
  );
}
