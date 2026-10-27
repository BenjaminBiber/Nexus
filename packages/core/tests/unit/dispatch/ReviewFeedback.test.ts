import { describe, expect, it } from 'vitest';
import { ReviewFeedback } from '../../../src/dispatch/ReviewFeedback.js';

const feedback = new ReviewFeedback();

// Kommentar mit Autor und Text
const c = (authorName: string, body: string) => ({ authorName, body });

describe('ReviewFeedback', () => {
  it('liefert nur Rückmeldungen des Dispatchers nach dem letzten Start', () => {
    const comments = [
      c('dispatcher', '[dispatch] Start: an junior-ARC-0001 übergeben'),
      c('dispatcher', '[dispatch] Zurück an den Agenten (von Florian): alt, schon erledigt'),
      c('dispatcher', '[dispatch] Start: an junior-ARC-0001 übergeben'),
      c('dispatcher', '[dispatch] Review: wartet auf Freigabe'),
      c('dispatcher', '[dispatch] Zurück an den Agenten (von Florian): Bitte auch den Fehlerfall testen'),
      c('Florian', '[dispatch] Zurück an den Agenten (von Florian): nachgemacht'),
      c('dispatcher', '[dispatch] Zurück an den Agenten (von Anna): Und\nzweite Zeile'),
    ];
    expect(feedback.pending(comments, 'dispatcher')).toEqual(['Bitte auch den Fehlerfall testen', 'Und\nzweite Zeile']);
  });

  it('ohne Start zählen alle Rückmeldungen; ohne Rückmeldung ist die Liste leer', () => {
    expect(feedback.pending([c('dispatcher', '[dispatch] Zurück an den Agenten (von X): eins')], 'dispatcher')).toEqual(['eins']);
    expect(feedback.pending([c('dispatcher', '[dispatch] Start: x')], 'dispatcher')).toEqual([]);
  });
});
