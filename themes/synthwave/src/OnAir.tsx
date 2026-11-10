import { ConnectionStatus } from '@nexus/theme-sdk';

// Leuchtschild „ON AIR“ um den Verbindungspunkt des Hosts. Das SDK liefert den Zustand nur als fertigen Punkt; theme.css
// liest ihn darüber ab (.live.off = getrennt), schaltet das Schild aus und zeigt dann „OFF AIR“ wie im Studio, wenn die
// Sendung vorbei ist. Der Punkt bleibt für Screenreader erhalten.
export function OnAir() {
  return (
    <span className="synth-onair">
      <ConnectionStatus />
      <span className="synth-onair-text" aria-hidden="true">
        ON AIR
      </span>
      <span className="synth-offair-text" aria-hidden="true">
        OFF AIR
      </span>
    </span>
  );
}
