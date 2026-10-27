import type { Comment } from '../board/BoardTypes.js';
import { DISPATCH_MARKERS, type DispatchConfig } from './DispatchConfig.js';

/** Anfang des Kommentars bei einer Pause wegen Nutzungslimit */
export const LIMIT_PAUSE = `${DISPATCH_MARKERS.PAUSE}: Nutzungslimit`;

// Zählt gescheiterte automatische Versuche an den Abbruch-Kommentaren des Dispatchers.
export class AttemptCounter {
  constructor(private readonly cfg: DispatchConfig) {}

  // Anzahl der Kommentare des Dispatchers, die mit „[dispatch] Abbruch“ beginnen
  failed(comments: Pick<Comment, 'authorName' | 'body'>[]): number {
    return comments.filter((c) => c.authorName === this.cfg.dispatcherName && c.body.startsWith(DISPATCH_MARKERS.ABORT)).length;
  }

  // Wie viele Limit-Pausen („[dispatch] Pause: Nutzungslimit …“) zuletzt in Folge kamen; Start-Kommentare dazwischen
  // zählen nicht, jeder andere Kommentar des Dispatchers (Abbruch, Frage, …) beendet die Folge
  limitPausesInRow(comments: Pick<Comment, 'authorName' | 'body'>[]): number {
    let count = 0;
    for (const c of [...comments].reverse()) {
      if (c.authorName !== this.cfg.dispatcherName || c.body.startsWith(DISPATCH_MARKERS.START)) continue;
      if (!c.body.startsWith(LIMIT_PAUSE)) break;
      count += 1;
    }
    return count;
  }
}
