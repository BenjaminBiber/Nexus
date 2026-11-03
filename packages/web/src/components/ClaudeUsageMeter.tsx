import { useEffect, useRef, useState } from 'react';
import { Link, useParams } from 'react-router-dom';
import { useApi } from '../api/ApiContext.js';
import type { AccountUsageEntryDto, ClaudeUsageDto, UsageOverviewDto, UsageWindowDto } from '../api/usageTypes.js';

const REFRESH_MS = 120_000;
/** Nach einem vorübergehenden Fehlschlag (Netz, abgelaufenes Token) wird schnell wiederholt statt erst nach REFRESH_MS */
export const RETRY_MS = 8_000;

/** Warum ein Account gerade keine Werte hat */
export const USAGE_REASONS: Record<NonNullable<ClaudeUsageDto['reason']>, string> = {
  disabled: 'Limits nicht abgefragt',
  'no-login': 'Keine Anmeldung gefunden',
  'token-expired': 'Anmeldung abgelaufen – beim nächsten Lauf erneuert Claude Code sie',
  'rate-limited': 'Anthropic drosselt die Abfrage – neuer Versuch in einigen Minuten',
  'request-failed': 'Limits gerade nicht abrufbar',
};

// Gründe, die von allein wieder vergehen – nur bei ihnen wird schnell wiederholt, der letzte gute Stand behalten und ein Account
// ohne Stand als grauer Ring gezeigt
function transient(value: ClaudeUsageDto): boolean {
  return value.reason === 'request-failed' || value.reason === 'token-expired' || value.reason === 'rate-limited';
}

// Hat die Antwort Werte, die sich zeigen lassen?
function shows(value: ClaudeUsageDto): boolean {
  return value.available && Boolean(value.fiveHour || value.sevenDay);
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

// Ein alter Stand zählt nach dem Zurücksetzen seines Fensters als 0 %
function settled(window: UsageWindowDto | null, now: Date): UsageWindowDto | null {
  if (!window?.resetsAt) return window;
  const at = new Date(window.resetsAt);
  return Number.isNaN(at.getTime()) || at > now ? window : { percent: 0, resetsAt: null };
}

const SIZE = 52;
const C = SIZE / 2;
const OUTER = { r: 22, width: 5 };
const INNER = { r: 14, width: 4 };

/** Ab dieser Auslastung wird ein Ring rot, unabhängig von der Grenze */
export const CRITICAL_PERCENT = 85;

type Tone = 'ok' | 'over' | 'crit';

// Farbe eines Fensters: ab 85 % rot, sonst ab der wirksamen Grenze orange
function tone(window: UsageWindowDto | null, limit: number): Tone {
  if (!window) return 'ok';
  return window.percent >= CRITICAL_PERCENT ? 'crit' : window.percent >= limit ? 'over' : 'ok';
}

// Ring eines Fensters: Bogen von oben im Uhrzeigersinn, Strich an der eigenen Grenze
function Arc({ r, width, percent, limit, tone: t, kind }: { r: number; width: number; percent: number | null; limit: number | null; tone: Tone; kind: 'outer' | 'inner' }) {
  const shown = Math.min(100, Math.max(0, percent ?? 0));
  const angle = ((limit ?? 0) / 100) * 2 * Math.PI - Math.PI / 2;
  const from = r - width / 2 - 2;
  const to = r + width / 2 + 2;
  return (
    <>
      <circle className="usage-ring-track" cx={C} cy={C} r={r} strokeWidth={width} />
      {shown > 0 && (
        <circle
          className={`usage-ring-arc ${kind}${t === 'ok' ? '' : ` ${t}`}`}
          cx={C} cy={C} r={r} strokeWidth={width} pathLength={100} strokeDasharray={`${shown} 100`}
          transform={`rotate(-90 ${C} ${C})`}
        />
      )}
      {limit !== null && (
        <line className="usage-ring-tick" x1={C + from * Math.cos(angle)} y1={C + from * Math.sin(angle)} x2={C + to * Math.cos(angle)} y2={C + to * Math.sin(angle)} />
      )}
    </>
  );
}

// Wert mit eigener Grenze („92/90 %“) oder ohne („8 %“); über der Grenze orange, ab 85 % rot
function Value({ percent, limit, tone: t }: { percent: number | null; limit: number | null; tone: Tone }) {
  if (percent === null) return <>–</>;
  return (
    <>
      <span className={`usage-num${t === 'ok' ? '' : ` ${t}`}`}>{percent}</span>
      <span className={limit === null ? 'usage-num-unit' : 'usage-num-limit'}>{limit === null ? ' %' : `/${limit} %`}</span>
    </>
  );
}

interface Row {
  entry: AccountUsageEntryDto;
  /** Werte zum Zeigen; null = noch nie welche (grauer Ring, Grund in `entry.usage.reason`) */
  usage: ClaudeUsageDto | null;
  /** Letzter guter Stand, weil die aktuelle Abfrage gerade scheitert */
  stale: boolean;
}

// Grund, warum ein Account keine (frischen) Werte hat; leer, wenn die Abfrage gerade klappt
function why(row: Row): string {
  const reason = row.entry.usage.reason;
  return row.usage === row.entry.usage || !reason ? '' : USAGE_REASONS[reason];
}

// Werte eines Accounts, wie Ring, Text und Hover-Karte sie brauchen
function measure({ entry, usage }: Row, defaultLimit: number, now: Date) {
  const five = settled(usage?.fiveHour ?? null, now);
  const week = settled(usage?.sevenDay ?? null, now);
  return {
    five, week,
    fiveTone: tone(five, entry.limitFiveHour ?? defaultLimit),
    weekTone: tone(week, entry.limitWeek ?? defaultLimit),
  };
}

// Kürzel eines Account-Namens für die Mitte des Rings: Anfangsbuchstaben der Teile („Privat-Max“ → „PM“), bei einem Wort
// die ersten beiden Buchstaben („Support“ → „SU“); bei einer E-Mail zählt nur der Teil vor dem @
export function initials(name: string): string {
  const parts = name.split('@')[0]!.split(/[^\p{L}\p{N}]+/u).filter(Boolean);
  const letters = parts.length > 1 ? parts.slice(0, 2).map((p) => [...p][0]).join('') : [...(parts[0] ?? '')].slice(0, 2).join('');
  return letters.toLocaleUpperCase('de-DE');
}

// Doppelring eines Accounts (außen 5 Stunden, innen Woche), in der Mitte das Kürzel des Namens
function Ring({ row, defaultLimit, now }: { row: Row; defaultLimit: number; now: Date }) {
  const { five, week, fiveTone, weekTone } = measure(row, defaultLimit, now);
  return (
    <svg className={`usage-ring${row.usage ? '' : ' empty'}`} viewBox={`0 0 ${SIZE} ${SIZE}`} width={SIZE} height={SIZE} role="img" aria-label={`Sitzung ${five ? `${five.percent} %` : 'unbekannt'}, Woche ${week ? `${week.percent} %` : 'unbekannt'}`}>
      <Arc {...OUTER} kind="outer" percent={five?.percent ?? null} limit={row.entry.limitFiveHour} tone={fiveTone} />
      <Arc {...INNER} kind="inner" percent={week?.percent ?? null} limit={row.entry.limitWeek} tone={weekTone} />
      <text className="usage-ring-initials" x={C} y={C} textAnchor="middle" dominantBaseline="central" aria-hidden="true">{initials(row.entry.name)}</text>
    </svg>
  );
}

// Zurücksetzen für die Hover-Karte: innerhalb von 24 Stunden mit Uhrzeit dahinter („Reset in 25 Min. (12:50)“), später
// „Reset am Do., 16:00“; leer ohne gültigen Zeitpunkt
export function resetDetail(resetsAt: string | null, now: Date): string {
  const label = resetLabel(resetsAt, now);
  if (!label) return '';
  const clock = label.startsWith('in ') ? ` (${new Date(resetsAt!).toLocaleTimeString('de-DE', { hour: '2-digit', minute: '2-digit' })})` : '';
  return `Reset ${label}${clock}`;
}

// Ein Fenster als Zeile: Auslastung / Grenze (ohne eigene die Vorgabe), darunter, wann es zurückgesetzt wird
function UsageLine({ label, window: w, own, defaultLimit, tone: t, now }: { label: string; window: UsageWindowDto | null; own: number | null; defaultLimit: number; tone: Tone; now: Date }) {
  const reset = w ? resetDetail(w.resetsAt, now) : '';
  return (
    <div className="usage-pop-row">
      <span className="usage-pop-label">{label}</span>
      <span className="usage-pop-value">
        {w ? <span className={`usage-num${t === 'ok' ? '' : ` ${t}`}`}>{w.percent} %</span> : '–'}
        <span className="usage-pop-limit" title={own === null ? 'Vorgabe' : undefined}> / {own ?? defaultLimit} %</span>
      </span>
      {reset && <span className="usage-pop-reset">{reset}</span>}
    </div>
  );
}

// Genaue Werte eines Accounts beim Darüberfahren über seinen Ring in der Seitenleiste: je Fenster Auslastung / Grenze und
// darunter, wann es zurückgesetzt wird
function Details({ row, defaultLimit, now }: { row: Row; defaultLimit: number; now: Date }) {
  const { entry, usage, stale } = row;
  const { five, week, fiveTone, weekTone } = measure(row, defaultLimit, now);
  const line = (label: string, w: UsageWindowDto | null, own: number | null, t: Tone) => (
    <UsageLine label={label} window={w} own={own} defaultLimit={defaultLimit} tone={t} now={now} />
  );
  const at = usage?.fetchedAt ? new Date(usage.fetchedAt).toLocaleTimeString('de-DE', { hour: '2-digit', minute: '2-digit' }) : null;
  const plan = usage?.plan ?? entry.usage.plan;
  const reason = why(row);
  return (
    <div className="usage-pop" role="tooltip">
      <strong className="usage-pop-name">{entry.name}{plan && <span className="usage-pop-plan"> · {plan}</span>}</strong>
      {line('Sitzung', five, entry.limitFiveHour, fiveTone)}
      {line('Woche', week, entry.limitWeek, weekTone)}
      {at && <span className="usage-pop-stand">Stand {at}{stale ? ' · gerade nicht erreichbar' : ''}</span>}
      {reason && <span className="usage-pop-why">{reason}</span>}
    </div>
  );
}

// Ring mit Name und Werten daneben (Einstellungen)
function AccountCard({ row, defaultLimit, now }: { row: Row; defaultLimit: number; now: Date }) {
  const { five, week, fiveTone, weekTone } = measure(row, defaultLimit, now);
  return (
    <div className={`usage-acc${row.stale ? ' stale' : ''}`} role="group" aria-label={row.entry.name}>
      <Ring row={row} defaultLimit={defaultLimit} now={now} />
      <div className="usage-acc-text">
        <strong className="usage-acc-name">{row.entry.name}</strong>
        {row.usage ? (
          <>
            <span className="usage-acc-five"><Value percent={five?.percent ?? null} limit={row.entry.limitFiveHour} tone={fiveTone} /></span>
            <span className="usage-acc-week">W <Value percent={week?.percent ?? null} limit={row.entry.limitWeek} tone={weekTone} /></span>
          </>
        ) : <span className="usage-acc-why">{why(row)}</span>}
      </div>
    </div>
  );
}

// Doppelring eines Accounts (Kacheln der Accounts); ohne Werte der leere Ring als Platzhalter
export function AccountRing({ entry, defaultLimit }: { entry: AccountUsageEntryDto; defaultLimit: number }) {
  const row: Row = { entry, usage: shows(entry.usage) ? entry.usage : null, stale: false };
  return <Ring row={row} defaultLimit={defaultLimit} now={new Date()} />;
}

// Werte je Fenster zum Ring eines Accounts (Kacheln der Accounts), je Fenster eine Zeile; ohne Werte dieselben Zeilen
// mit „–“, damit alle Kacheln gleich hoch bleiben
export function AccountUsageLines({ entry, defaultLimit }: { entry: AccountUsageEntryDto; defaultLimit: number }) {
  const now = new Date();
  const known = shows(entry.usage);
  const { five, week, fiveTone, weekTone } = known
    ? measure({ entry, usage: entry.usage, stale: false }, defaultLimit, now)
    : { five: null, week: null, fiveTone: 'ok' as Tone, weekTone: 'ok' as Tone };
  return (
    <div className={`usage-tile-rows${known ? '' : ' empty'}`}>
      <UsageLine label="Sitzung (5 Std.)" window={five} own={entry.limitFiveHour} defaultLimit={defaultLimit} tone={fiveTone} now={now} />
      <UsageLine label="Woche" window={week} own={entry.limitWeek} defaultLimit={defaultLimit} tone={weekTone} now={now} />
    </div>
  );
}

// Nur der Ring; beim Darüberfahren oder mit Fokus die genauen Werte, Klick führt zu den Accounts (Seitenleiste)
function AccountDot({ row, defaultLimit, now, to }: { row: Row; defaultLimit: number; now: Date; to: string | null }) {
  const ring = <Ring row={row} defaultLimit={defaultLimit} now={now} />;
  return (
    <div className={`usage-dot${row.stale || !row.usage ? ' stale' : ''}`} role="group" aria-label={row.entry.name}>
      {to ? <Link className="usage-dot-link" to={to} aria-label={`${row.entry.name}: alle Accounts`}>{ring}</Link> : ring}
      <Details row={row} defaultLimit={defaultLimit} now={now} />
    </div>
  );
}

// Lädt die Limits aller aktiven Accounts und hält je Account den letzten guten Stand. Vorübergehende Fehlschläge (Netz,
// abgelaufenes Token, Server kurz weg) lassen die Anzeige nicht verschwinden: es bleibt beim letzten guten Stand, bis die
// schnelle Wiederholung neue Werte bringt.
function useUsageOverview(): { overview: UsageOverviewDto | null; rows: Row[]; waiting: boolean } {
  const api = useApi();
  const [overview, setOverview] = useState<UsageOverviewDto | null>(null);
  // Letzter Stand mit Werten je Account
  const lastGood = useRef(new Map<string, ClaudeUsageDto>());
  useEffect(() => {
    let alive = true;
    let timer: ReturnType<typeof setTimeout> | undefined;
    // Statt setInterval eine selbst geplante Schleife: Hintergrund-Tabs drosseln Intervalle, und nach einem Fehlschlag
    // soll der nächste Versuch früher kommen als nach zwei Minuten.
    const load = () => {
      clearTimeout(timer);
      Promise.resolve(api.getUsageOverview())
        .then((value) => {
          if (!alive) return;
          const good = lastGood.current;
          const ids = new Set(value.accounts.map((a) => a.id));
          for (const id of [...good.keys()]) if (!ids.has(id) || !value.enabled) good.delete(id);
          for (const a of value.accounts) {
            if (shows(a.usage)) good.set(a.id, a.usage);
            else if (!transient(a.usage)) good.delete(a.id);
          }
          setOverview(value);
          const soft = value.enabled && value.accounts.some((a) => !shows(a.usage) && transient(a.usage));
          // Sichtbar ist der Block mit Werten oder grauen Ringen (vorübergehender Grund)
          remember(value.enabled && (good.size > 0 || soft));
          timer = setTimeout(load, soft ? RETRY_MS : REFRESH_MS);
        })
        .catch(() => {
          // Server kurz nicht erreichbar: der letzte Stand bleibt stehen
          if (!alive) return;
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
  if (!overview?.enabled) return { overview, rows: [], waiting: !overview };
  // Ohne Werte, aber mit vorübergehendem Grund (Token abgelaufen, gedrosselt) bleibt der Account als grauer Ring stehen
  const rows = overview.accounts.flatMap((entry): Row[] => {
    const usage = shows(entry.usage) ? entry.usage : lastGood.current.get(entry.id) ?? null;
    if (!usage && !transient(entry.usage)) return [];
    return [{ entry, usage, stale: usage !== null && usage !== entry.usage }];
  });
  return { overview, rows, waiting: false };
}

// Claude-Nutzungslimits je aktivem Account als Doppelring (außen Sitzung über 5 Stunden, innen Woche, Strich = eigene Grenze);
// über der Grenze orange, ab 85 % rot. „compact“ (Seitenleiste): nur die Ringe, genaue Werte beim Darüberfahren, Klick führt
// zu den Accounts. „full“ (Einstellungen): Ringe mit Name und Werten daneben. Ohne eingeschaltete Einstellung oder ohne Werte
// bleibt der Block unsichtbar.
export function ClaudeUsageMeter({ variant = 'compact' }: { variant?: 'compact' | 'full' }) {
  const { pid = '' } = useParams();
  const { overview, rows, waiting } = useUsageOverview();
  const skeleton = variant === 'compact' && remembered() ? <div className="usage-meter usage-skel" aria-hidden="true" /> : null;
  if (!overview || !rows.length) return waiting ? skeleton : null;
  const now = new Date();
  const to = pid ? `/p/${pid}/settings#accounts` : null;
  return (
    <section className={`usage-meter ${variant}`} aria-label="Claude-Nutzungslimits">
      <div className="usage-accounts">
        {rows.map((row) => variant === 'full'
          ? <AccountCard key={row.entry.id} row={row} defaultLimit={overview.defaultLimit} now={now} />
          : <AccountDot key={row.entry.id} row={row} defaultLimit={overview.defaultLimit} now={now} to={to} />)}
      </div>
    </section>
  );
}
