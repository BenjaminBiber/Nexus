import { useUsage, type UsageWindow } from '@nexus/theme-sdk';

const SEGMENTS = 12;

// Zahl der leuchtenden Segmente für eine Auslastung in Prozent; jede Nutzung über 0 zeigt mindestens ein Segment
export function litSegments(percent: number, total = SEGMENTS): number {
  const clamped = Math.min(100, Math.max(0, percent));
  const lit = Math.round((clamped / 100) * total);
  return clamped > 0 ? Math.max(1, lit) : lit;
}

// Segmente 9–10 sind die Warnzone, 11–12 die Spitze
function zone(index: number): 'warn' | 'peak' | undefined {
  if (index >= 10) return 'peak';
  if (index >= 8) return 'warn';
  return undefined;
}

function resetTitle(resetsAt: string | null): string {
  if (!resetsAt) return 'Rücksetzzeitpunkt unbekannt';
  const date = new Date(resetsAt);
  if (Number.isNaN(date.getTime())) return 'Rücksetzzeitpunkt unbekannt';
  const time = date.toLocaleTimeString('de-DE', { hour: '2-digit', minute: '2-digit' });
  return `setzt sich um ${time} zurück`;
}

function Meter({ window, tag, name, slim }: { window: UsageWindow; tag: string; name: string; slim?: boolean }) {
  const percent = Math.round(window.percent);
  const lit = litSegments(window.percent);
  return (
    <div
      className="synth-vu"
      data-slim={slim ? '' : undefined}
      role="meter"
      aria-label={`Claude-Nutzung ${name}`}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={percent}
      title={`Claude-Nutzung ${name}: ${percent} % – ${resetTitle(window.resetsAt)}`}
    >
      <span className="synth-vu-tag">{tag}</span>
      <span className="synth-vu-bar">
        {Array.from({ length: SEGMENTS }, (_, i) => (
          <i key={i} data-on={i < lit ? '' : undefined} data-zone={zone(i)} data-top={i === lit - 1 ? '' : undefined} />
        ))}
      </span>
      <span className="synth-vu-value">{percent}%</span>
    </div>
  );
}

// Claude-Nutzung als VU-Meter mit Segmenten: oben das 5-Stunden-Fenster, darunter schmaler das 7-Tage-Fenster
export function VuMeter() {
  const usage = useUsage();
  if (!usage.enabled) return null;
  if (!usage.available) {
    return usage.reason ? (
      <span className="synth-vu-off" title={usage.reason}>
        VU –
      </span>
    ) : null;
  }
  if (!usage.fiveHour) return null;
  return (
    <div className="synth-vu-stack">
      <Meter window={usage.fiveHour} tag="5H" name="5 Stunden" />
      {usage.sevenDay && <Meter window={usage.sevenDay} tag="7D" name="7 Tage" slim />}
    </div>
  );
}
