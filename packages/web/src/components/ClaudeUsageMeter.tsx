import { useEffect, useState } from 'react';
import { useApi } from '../api/ApiContext.js';
import type { ClaudeUsageDto, UsageWindowDto } from '../api/usageTypes.js';

const REFRESH_MS = 120_000;
/** Nach einem vorübergehenden Fehlschlag (Netz, abgelaufenes Token) wird schnell wiederholt statt erst nach REFRESH_MS */
export const RETRY_MS = 8_000;

// Gründe, die von allein wieder vergehen – nur bei ihnen wird schnell wiederholt und der gemerkte Platz behalten
function transient(value: ClaudeUsageDto | null): boolean {
  return value === null || value.reason === 'request-failed' || value.reason === 'token-expired';
}

/** Meldung der Einstellungen an den Block: der Schalter hat sich geändert, sofort neu laden */
export const USAGE_SETTING_EVENT = 'harness:usage-setting';

// Merkt sich, ob der Block beim letzten Mal sichtbar war, damit sein Platz schon beim Laden frei bleibt (sonst springt der
// Rest der Seitenleiste). Ohne Speicher (privates Fenster) bleibt es beim Nachrücken.
const REMEMBER_KEY = 'harness:usage-ring';
function remembered(): boolean {
  try {
    return localStorage.getItem(REMEMBER_KEY) === '1';
  } catch {
    return false;
  }
}
function remember(visible: boolean) {
  try {
    localStorage.setItem(REMEMBER_KEY, visible ? '1' : '0');
  } catch {
    // ohne Speicher kein Merken
  }
}

// Stufe für die Farbe des Balkens
function level(percent: number): 'ok' | 'warn' | 'crit' {
  return percent >= 90 ? 'crit' : percent >= 75 ? 'warn' : 'ok';
}

// Zeitpunkt des Zurücksetzens: innerhalb der nächsten 24 Stunden als Restzeit („in 3 Std. 12 Min.“), später mit Wochentag und
// Uhrzeit in Ortszeit („am Do., 16:00“); leer ohne gültigen Zeitpunkt
export function resetLabel(resetsAt: string | null, now: Date): string {
  const at = resetsAt ? new Date(resetsAt) : null;
  if (!at || Number.isNaN(at.getTime())) return '';
  const minutes = Math.max(0, Math.ceil((at.getTime() - now.getTime()) / 60_000));
  if (minutes < 24 * 60) {
    const hours = Math.floor(minutes / 60);
    const rest = minutes % 60;
    return `in ${[hours > 0 && `${hours} Std.`, (rest > 0 || hours === 0) && `${rest} Min.`].filter(Boolean).join(' ')}`;
  }
  const day = at.toLocaleDateString('de-DE', { weekday: 'short' }).replace(/\.?$/, '.');
  return `am ${day}, ${at.toLocaleTimeString('de-DE', { hour: '2-digit', minute: '2-digit' })}`;
}

function Limit({ name, window, now }: { name: string; window: UsageWindowDto; now: Date }) {
  const reset = resetLabel(window.resetsAt, now);
  const shown = Math.min(100, Math.max(0, window.percent));
  return (
    <div className="usage-limit">
      <div className="usage-row">
        <span className="usage-name">{name}</span>
        <strong className={`usage-value ${level(window.percent)}`}>{window.percent} %</strong>
      </div>
      <div className="usage-bar" role="progressbar" aria-label={name} aria-valuemin={0} aria-valuemax={100} aria-valuenow={shown}>
        <span className={`usage-fill ${level(window.percent)}`} style={{ width: `${shown}%` }} />
      </div>
      {reset && <span className="usage-reset">Zurücksetzung {reset}</span>}
    </div>
  );
}

// Claude-Nutzungslimits ganz unten in der Seitenleiste: je ein Balken für die Sitzung (5 Stunden) und die Woche mit
// Prozentwert und Zeitpunkt des Zurücksetzens. Fehlt die Anmeldung oder antwortet der Endpunkt nicht, bleibt der Block
// unsichtbar; vorübergehende Fehlschläge werden kurz darauf wiederholt und beim Zurückkehren in den Tab, sodass der Block
// ohne Neuladen der Seite zurückkommt.
export function ClaudeUsageMeter() {
  const api = useApi();
  const [usage, setUsage] = useState<ClaudeUsageDto | null>(null);
  // Erst nach der ersten Antwort entscheidet sich, ob der Block erscheint
  const [failed, setFailed] = useState(false);
  // Vorübergehend fehlgeschlagen: der Platz bleibt reserviert, bis die Wiederholung Bescheid gibt
  const [retrying, setRetrying] = useState(false);
  useEffect(() => {
    let alive = true;
    let timer: ReturnType<typeof setTimeout> | undefined;
    // Statt setInterval eine selbst geplante Schleife: Hintergrund-Tabs drosseln Intervalle, und nach einem Fehlschlag
    // soll der nächste Versuch früher kommen als nach zwei Minuten.
    const load = () => {
      clearTimeout(timer);
      Promise.resolve(api.getClaudeUsage())
        .then((value) => {
          if (!alive) return;
          setUsage(value);
          setFailed(false);
          const soft = !value.available && transient(value);
          setRetrying(soft);
          if (!soft) remember(Boolean(value.available && (value.fiveHour || value.sevenDay)));
          timer = setTimeout(load, soft ? RETRY_MS : REFRESH_MS);
        })
        .catch(() => {
          if (!alive) return;
          setUsage(null);
          setFailed(true);
          setRetrying(true);
          timer = setTimeout(load, RETRY_MS);
        });
    };
    const onVisible = () => {
      if (document.visibilityState === 'visible') load();
    };
    load();
    window.addEventListener(USAGE_SETTING_EVENT, load);
    document.addEventListener('visibilitychange', onVisible);
    return () => {
      alive = false;
      clearTimeout(timer);
      window.removeEventListener(USAGE_SETTING_EVENT, load);
      document.removeEventListener('visibilitychange', onVisible);
    };
  }, [api]);
  if ((!usage && !failed) || retrying) return remembered() ? <div className="usage-meter usage-skel" aria-hidden="true" /> : null;
  if (!usage?.available || (!usage.fiveHour && !usage.sevenDay)) return null;
  const now = new Date();
  const stand = usage.fetchedAt ? `Stand ${new Date(usage.fetchedAt).toLocaleTimeString('de-DE', { hour: '2-digit', minute: '2-digit' })}` : undefined;
  return (
    <section className="usage-meter" aria-label="Claude-Nutzungslimits">
      <h2 className="usage-head" title={stand}>
        Deine Nutzungslimits{usage.plan && <span className="usage-plan"> · {usage.plan}</span>}
      </h2>
      {usage.fiveHour && <Limit name="Sitzungslimit" window={usage.fiveHour} now={now} />}
      {usage.sevenDay && <Limit name="Wöchentlich · alle Modelle" window={usage.sevenDay} now={now} />}
    </section>
  );
}
