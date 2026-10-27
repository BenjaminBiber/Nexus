import { DISPATCH_MARKERS, MERGE_REASON_PREFIX, type BoardTicketDetail, type MergeEntry } from '@harness/core';

/** Ergebnis von „Erneut mergen“, „Abnehmen“ bzw. „Zurück an den Agenten“ für das angefragte Ticket (accepted: nur erledigt,
 *  kein Merge angestoßen; returned: zurück nach Geplant) */
export interface MergeRetryResult {
  outcome: 'merged' | 'blocked' | 'waiting' | 'session' | 'pending' | 'accepted' | 'returned';
  /** Eine Zeile für CLI und Oberfläche, z. B. „ARC-0012 gemergt in main (abc1234).“ */
  text: string;
}

// Liest nach „Erneut mergen“ aus Bericht, Ticket (Status, Grund, letzte Dispatcher-Kommentare) und Merge-Auftrag,
// was mit dem Merge dieses Tickets geschehen ist.
export class MergeResultReader {
  // Ergebnis für das Ticket `id`; `detail` null = Ticket nicht mehr lesbar
  read(id: string, merged: string[], errors: string[], detail: BoardTicketDetail | null, entry: MergeEntry | undefined): MergeRetryResult {
    if (merged.includes(id)) return { outcome: 'merged', text: this.mergedText(id, detail) };
    const reason = detail?.blockedReason ?? '';
    if (detail?.status === 'blocked' && reason.startsWith(MERGE_REASON_PREFIX)) {
      return { outcome: 'blocked', text: `${id}: Merge blockiert – ${reason.slice(MERGE_REASON_PREFIX.length).trim()}` };
    }
    if (entry?.phase === 'session') return { outcome: 'session', text: `${id}: Merge-Konflikt – Merge-Session gestartet.` };
    if (entry?.phase === 'waiting') {
      const wait = this.lastComment(detail, DISPATCH_MARKERS.MERGE_WAIT);
      const rest = wait ? wait.slice(DISPATCH_MARKERS.MERGE_WAIT.length).trim() : 'auf weitere Tickets des Worktrees.';
      return { outcome: 'waiting', text: `${id}: Merge wartet ${rest}` };
    }
    const error = errors.find((e) => e.split(':')[0].split(', ').includes(id));
    if (error) return { outcome: 'pending', text: `${id}: Merge nicht ausgeführt – ${error.slice(error.indexOf(':') + 1).trim()}` };
    if (entry?.conflictSeen) {
      return { outcome: 'pending', text: `${id}: Merge-Konflikt – die Merge-Session wartet (Dispatcher ausgeschaltet, Pause oder außerhalb des Zeitfensters).` };
    }
    return { outcome: 'pending', text: `${id}: Merge wartet auf den nächsten Durchlauf des Dispatchers.` };
  }

  // „<ID> gemergt in <Ziel> (<Commit>).“ aus dem Kommentar des Dispatchers, samt Hinweis auf unvollständiges Aufräumen
  private mergedText(id: string, detail: BoardTicketDetail | null): string {
    const comment = this.lastComment(detail, DISPATCH_MARKERS.MERGED);
    const head = comment?.match(/^\[dispatch\] Gemergt in (.+?) \(([^)]+)\)\./);
    if (!comment || !head) return `${id} gemergt.`;
    const lines = comment.split('\n');
    const incomplete = lines[0].includes('Aufräumen unvollständig') ? ` Aufräumen unvollständig: ${lines.slice(1).join(' ')}` : '';
    return `${id} gemergt in ${head[1]} (${head[2]}).${incomplete}`;
  }

  // Letzter Kommentar mit dem Marker oder null
  private lastComment(detail: BoardTicketDetail | null, marker: string): string | null {
    const found = (detail?.comments ?? []).filter((c) => c.body.startsWith(marker)).at(-1);
    return found?.body ?? null;
  }
}
