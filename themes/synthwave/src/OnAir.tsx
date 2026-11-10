import { ConnectionStatus } from '@nexus/theme-sdk';

// Leuchtschild „ON AIR“ um den Verbindungspunkt des Hosts. Das SDK liefert den Zustand nur als fertigen Punkt; theme.css
// liest ihn darüber ab (.live.off = getrennt) und schaltet das Schild aus. Der Punkt bleibt für Screenreader erhalten.
export function OnAir() {
  return (
    <span className="synth-onair">
      <ConnectionStatus />
      <span className="synth-onair-text" aria-hidden="true">
        ON AIR
      </span>
    </span>
  );
}
