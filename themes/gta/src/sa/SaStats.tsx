import type { CSSProperties, ReactNode, RefObject } from 'react';
import { useGame, type Game } from '../data/GameContext.js';
import { formatMoneySa, formatPercent, percentOf } from '../shared/format.js';
import { useStatsDialog } from '../shared/useStatsDialog.js';

// Lage der Reviere auf der Karte der Preview (x, y, Breite, Höhe), eins je Spalte des Boards; das Board hat höchstens
// acht Spalten (ohne „Nicht nötig“ sieben), jede bekommt also ein eigenes Gebiet
const TURFS: [number, number, number, number][] = [
  [22, 34, 44, 32],
  [76, 24, 50, 42],
  [256, 76, 44, 40],
  [136, 76, 50, 40],
  [196, 26, 50, 40],
  [20, 126, 46, 36],
  [76, 76, 50, 40],
  [196, 126, 50, 36],
];

const share = (value: number) => `${percentOf(value)}%`;

function Panel({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="gta-sa-panel">
      <h2 className="gta-sa-panel-title">{title}</h2>
      {children}
    </section>
  );
}

// Balken mit Kopfzeile: Name, kleine Erklärung, Zahl rechts
function Stat({ name, hint, value, fill, color }: { name: string; hint: string; value: string; fill: number; color?: string }) {
  return (
    <div className="gta-sa-stat">
      <div className="gta-sa-stat-head">
        <span>
          <span className="gta-sa-stat-name">{name}</span> <small>{hint}</small>
        </span>
        <span className="gta-sa-stat-value">{value}</span>
      </div>
      <i className="gta-sa-sbar" style={{ '--sa-p': share(fill), '--sa-c': color } as CSSProperties} aria-hidden="true" />
    </div>
  );
}

function Statistics({ game }: { game: Game }) {
  const free = (value: number | null) => (value === null ? '–' : `${formatPercent(value)} frei`);
  return (
    <Panel title="Statistik">
      <Stat name="Respekt" hint="Fortschritt des Boards" value={game.boardProgress === null ? '–' : formatPercent(game.boardProgress)} fill={game.boardProgress ?? 0} />
      <Stat name="Ausdauer" hint="5-Stunden-Kontingent" value={free(game.quota5h)} fill={game.quota5h ?? 0} color="var(--gta-danger)" />
      <Stat name="Panzerung" hint="7-Tage-Kontingent" value={free(game.quota7d)} fill={game.quota7d ?? 0} />
      <Stat
        name="Schritte"
        hint="offener Tickets"
        value={`${game.stepsDone}/${game.stepsTotal}`}
        fill={game.stepsTotal ? game.stepsDone / game.stepsTotal : 0}
        color="var(--gta-c-in_progress)"
      />
    </Panel>
  );
}

// Revierkarte: Wasser, Land, Straßen und je Spalte ein Gebiet in der Spaltenfarbe; leere Gebiete gedimmt
function Turfs({ game }: { game: Game }) {
  return (
    <Panel title="Reviere">
      <svg className="gta-sa-turf" viewBox="0 0 320 226" role="img" aria-label="Karte: je Spalte des Boards ein Revier in ihrer Farbe">
        <rect width="320" height="226" fill="#44719a" />
        <path d="M12 30Q64 6 152 16T304 32V192Q250 222 160 212T14 198Z" fill="#7b7e67" />
        <path d="M12 70H306M12 120H306M14 166H304M70 14V206M130 14V212M190 16V212M250 22V204" stroke="#292c2f" strokeWidth="5" />
        <path d="M0 214L320 84" stroke="#292c2f" strokeWidth="7" />
        {game.columns.map((column, i) => {
          const [x, y, w, h] = TURFS[i % TURFS.length]!;
          return (
            <rect
              key={column.id}
              className="gta-sa-turf-area"
              data-turf={column.id}
              data-empty={column.count === 0 ? '' : undefined}
              x={x}
              y={y}
              width={w}
              height={h}
              fill={`var(--gta-c-${column.id})`}
            >
              <title>
                {column.title}: {column.count}
              </title>
            </rect>
          );
        })}
      </svg>
      <ul className="gta-sa-legend">
        {game.columns.map((column) => (
          <li key={column.id} data-empty={column.count === 0 ? '' : undefined}>
            <i className="gta-sa-swatch" style={{ '--sa-c': `var(--gta-c-${column.id})` } as CSSProperties} aria-hidden="true" />
            {column.title} {column.count}
          </li>
        ))}
      </ul>
    </Panel>
  );
}

function Numbers({ game }: { game: Game }) {
  const rows: [string, ReactNode][] = [
    ['Tickets gesamt', game.total],
    ['Abgeschlossen', game.done],
    ['Offen', game.open],
    ['Blockiert', game.blocked],
    ['Offene Fragen', game.questions],
    ['Fahndungslevel', game.wanted],
    ['Verdient insgesamt', formatMoneySa(game.moneyShown)],
  ];
  return (
    <Panel title="Zahlen">
      {rows.map(([label, value]) => (
        <div key={label} className="gta-sa-st">
          <span>{label}</span>
          <b>{value}</b>
        </div>
      ))}
    </Panel>
  );
}

// Stats-Menü im Aufbau der Dashboard-Seite der Preview: Statistik, Reviere und Zahlen nebeneinander. Fokus beim Öffnen
// auf „Schließen“, Esc, Schließen oder ein Klick daneben schließen, danach Fokus zurück auf `returnFocus`. Der Rahmen
// setzt den Dialog an die Stelle des Inhalts.
export function SaStats({ returnFocus }: { returnFocus?: RefObject<HTMLElement | null> }) {
  const game = useGame();
  const { ref, onKeyDown, close } = useStatsDialog('.gta-sa-stats-close', returnFocus);
  return (
    <>
      <div className="gta-sa-stats-backdrop" aria-hidden="true" onClick={close} />
      <div ref={ref} className="gta-sa-stats" role="dialog" aria-modal="true" aria-label="Stats" data-gta-part="stats" onKeyDown={onKeyDown}>
        <div className="gta-sa-stats-head">
          <span className="gta-sa-stats-board">{game.boardName ?? ''}</span>
          <button type="button" className="gta-sa-stats-close" aria-label="Stats schließen" onClick={close}>
            Schließen <kbd>Esc</kbd>
          </button>
        </div>
        <div className="gta-sa-stats-grid">
          <Statistics game={game} />
          <Turfs game={game} />
          <Numbers game={game} />
        </div>
      </div>
    </>
  );
}
