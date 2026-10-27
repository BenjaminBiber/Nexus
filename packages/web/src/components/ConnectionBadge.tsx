import { useLiveConnection } from '../live/useLive.js';

// Verbindungsanzeige als Punkt neben der Board-Auswahl; der Text „verbunden“ oder „getrennt“ erscheint beim Überfahren.
export function ConnectionBadge() {
  const { state } = useLiveConnection();
  if (state !== 'connected' && state !== 'disconnected') return null;
  return (
    <span className={`live${state === 'disconnected' ? ' off' : ''}`} role="status">
      <span className="dot" />
      <span className="live-text">{state === 'connected' ? 'verbunden' : 'getrennt'}</span>
    </span>
  );
}
