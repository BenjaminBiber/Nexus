import { useEffect, useRef, useState, type CSSProperties, type KeyboardEvent, type ReactNode, type RefObject } from 'react';
import { useGame, type Game } from '../data/GameContext.js';
import type { Blip } from '../data/derive.js';
import { formatMoneyV, formatPercent } from '../shared/format.js';

type Category = 'general' | 'columns' | 'usage';

const CATEGORIES: [Category, string][] = [
  ['general', 'Allgemein'],
  ['columns', 'Spalten'],
  ['usage', 'Claude-Verbrauch'],
];

const NEXT_ORDER: Blip['column'][] = ['blocked', 'in_progress', 'planned'];
const MAX_SEGMENTS = 12;

/** Segmentbalken wie in der Preview; `segments` Teile, gefüllt bis `value` (0..1) */
function StatBar({ value, segments = 5, color }: { value: number; segments?: number; color?: string }) {
  const style = { '--v-p': `${Math.round(Math.min(1, Math.max(0, value)) * 1000) / 10}%`, '--v-n': segments, '--v-c': color } as CSSProperties;
  return <span className="gta-v-statbar" style={style} aria-hidden="true" />;
}

function Row({ label, value, bar }: { label: string; value: ReactNode; bar?: ReactNode }) {
  return (
    <div className="gta-v-row">
      <span className="gta-v-row-label">{label}</span>
      {bar}
      <span className="gta-v-row-value">{value}</span>
    </div>
  );
}

function General({ game }: { game: Game }) {
  const stars = '★'.repeat(Math.min(game.wanted, 5)) || '–';
  return (
    <>
      <Row label="Tickets gesamt" value={game.total} />
      <Row label="Abgeschlossen" value={game.done} />
      <Row label="Offen" value={game.open} />
      <Row label="Blockiert" value={game.blocked} />
      <Row label="Fortschritt des Boards" value={formatPercent(game.boardProgress ?? 0)} bar={<StatBar value={game.boardProgress ?? 0} />} />
      <Row label="Schritte erledigt" value={`${game.stepsDone} / ${game.stepsTotal}`} />
      <Row label="Offene Fragen" value={game.questions} />
      <Row label="Fahndungslevel" value={`${stars} (${game.blocked} blockiert)`} />
      <Row label="Verdient insgesamt" value={formatMoneyV(game.moneyShown)} />
    </>
  );
}

function Columns({ game }: { game: Game }) {
  return (
    <>
      {game.columns.map((column) => (
        <Row
          key={column.id}
          label={column.title}
          value={column.count}
          bar={<StatBar value={game.total ? column.count / game.total : 0} color={`var(--gta-c-${column.id})`} />}
        />
      ))}
    </>
  );
}

function Usage({ game }: { game: Game }) {
  if (game.quota5h === null && game.quota7d === null) {
    return <p className="gta-v-stats-hint">Claude-Verbrauch ist gerade nicht verfügbar.</p>;
  }
  const rest = (label: string, value: number | null) => (
    <Row label={label} value={value === null ? '–' : formatPercent(value)} bar={<StatBar value={value ?? 0} />} />
  );
  return (
    <>
      {rest('Rest 5 Stunden', game.quota5h)}
      {rest('Rest 7 Tage', game.quota7d)}
    </>
  );
}

// Bis zu drei offene Tickets: blockierte zuerst, dann in Arbeit, dann geplant
function nextTickets(blips: Blip[]): Blip[] {
  return NEXT_ORDER.flatMap((column) => blips.filter((blip) => blip.column === column)).slice(0, 3);
}

function Checklist({ game }: { game: Game }) {
  const next = nextTickets(game.blips);
  return (
    <div className="gta-v-pane gta-v-checklist">
      <div className="gta-v-pct">
        {formatPercent(game.boardProgress ?? 0)}
        <small>
          {game.done} von {game.total} Tickets
        </small>
      </div>
      <StatBar value={game.boardProgress ?? 0} segments={Math.max(1, Math.min(game.total, MAX_SEGMENTS))} />
      <h3>Als Nächstes</h3>
      {next.length ? (
        <ul>
          {next.map((blip) => (
            <li key={blip.id}>
              <b>{blip.id}</b> {blip.title}
            </li>
          ))}
        </ul>
      ) : (
        <p>Nichts dringend – alles im Plan.</p>
      )}
    </div>
  );
}

const FOCUSABLE = 'button:not([disabled]), [href], [tabindex]:not([tabindex="-1"])';

// Tab bleibt im Dialog: am Ende wieder vorne, am Anfang wieder hinten
function trapTab(event: KeyboardEvent<HTMLElement>): void {
  const items = [...event.currentTarget.querySelectorAll<HTMLElement>(FOCUSABLE)];
  const first = items[0];
  const last = items.at(-1);
  if (!first || !last) return;
  if (event.shiftKey && document.activeElement === first) {
    event.preventDefault();
    last.focus();
  } else if (!event.shiftKey && document.activeElement === last) {
    event.preventDefault();
    first.focus();
  }
}

// Stats-Menü im Aufbau der Statistik-Seite des Pausenmenüs: links Kategorien, Mitte Zeilen, rechts die
// 100-%-Checkliste. Fokus beim Öffnen hinein, Esc oder Schließen schließen, danach Fokus zurück auf `returnFocus`.
export function VStats({ returnFocus }: { returnFocus?: RefObject<HTMLElement | null> }) {
  const game = useGame();
  const [category, setCategory] = useState<Category>('general');
  const dialog = useRef<HTMLDivElement>(null);

  useEffect(() => {
    dialog.current?.querySelector<HTMLElement>('[aria-pressed="true"]')?.focus();
    const back = returnFocus?.current;
    return () => back?.focus();
  }, [returnFocus]);

  const close = () => game.setStatsOpen(false);
  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key === 'Escape') {
      event.stopPropagation();
      close();
    }
    if (event.key === 'Tab') trapTab(event);
  };
  const title = CATEGORIES.find(([id]) => id === category)![1];

  return (
    <div className="gta-v-stats-layer">
      <div ref={dialog} className="gta-v-stats" role="dialog" aria-modal="true" aria-label="Stats" data-gta-part="stats" onKeyDown={onKeyDown}>
        <div className="gta-v-stats-col">
          <h2 className="gta-v-h">Stats</h2>
          {CATEGORIES.map(([id, label]) => (
            <button key={id} type="button" className="gta-v-row gta-v-cat" aria-pressed={id === category} onClick={() => setCategory(id)}>
              {label}
            </button>
          ))}
          <button type="button" className="gta-v-stats-close" aria-label="Stats schließen" onClick={close}>
            Schließen <kbd>Esc</kbd>
          </button>
        </div>
        <div className="gta-v-stats-col">
          <div className="gta-v-h">
            {title}
            <span className="gta-v-h-note">{game.boardName ?? ''}</span>
          </div>
          <div className="gta-v-stats-rows">
            {category === 'general' ? <General game={game} /> : category === 'columns' ? <Columns game={game} /> : <Usage game={game} />}
          </div>
        </div>
        <div className="gta-v-stats-col">
          <div className="gta-v-h">100-%-Checkliste</div>
          <Checklist game={game} />
        </div>
      </div>
    </div>
  );
}
