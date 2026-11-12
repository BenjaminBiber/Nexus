import { useGame, type Game } from '../data/GameContext.js';

const count = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;

// Unterzeile aus echten Daten: auf dem Board offene Aufträge und blockierte, sonst offene Tickets und Fragen; ohne
// Board-Stand nur die Fragen
function info(game: Game): string {
  if (!game.loaded) return count(game.questions, 'Frage', 'Fragen');
  if (game.active?.id === 'board') return `${count(game.open, 'Auftrag', 'Aufträge')}, ${game.blocked} blockiert`;
  return `${game.open} offen · ${count(game.questions, 'Frage', 'Fragen')}`;
}

// Kopf des Inhalts: Name der Seite groß in der gotischen Schrift, darunter die Unterzeile, rechts daneben der schwarze
// Hilfekasten der Preview. Er erscheint nur nach einem Board-Wechsel (Name des Boards wie ein Radiosender); die Hülle
// mit role="status" bleibt stehen, damit Screenreader die Meldung ansagen. Der Seitenname ist keine Überschrift: die
// Seite des Kerns bringt ihr eigenes h1 mit.
export function SaTitle() {
  const game = useGame();
  return (
    <div className="gta-sa-title">
      <div className="gta-sa-title-text">
        <p className="gta-sa-title-page">{game.active?.label ?? ''}</p>
        <p className="gta-sa-title-info">{info(game)}</p>
      </div>
      <div className="gta-sa-help-live" role="status">
        {game.station ? (
          <p className="gta-sa-help" data-gta-part="station">
            Board: <b>{game.station}</b>
          </p>
        ) : null}
      </div>
    </div>
  );
}
