import { HarnessError } from '../errors/HarnessError.js';
import type { TicketDraft, TicketDraftCheck, TicketDraftSource } from './TicketDraft.js';

/** Mehr Entwürfe auf einmal sind nicht mehr überschaubar */
export const MAX_DRAFTS = 15;
/** Ein ganzes Dokument (etwa ein Plan mit vielen Tasks) zerfällt in mehr, dafür kleinere Tickets */
export const MAX_DOC_DRAFTS = 40;

const text = (value: unknown): string => (typeof value === 'string' ? value.trim() : '');
const list = (value: unknown): string[] => (Array.isArray(value) ? value.map(text).filter((s) => s !== '') : []);

// Prüft die Antwort von Claude auf Ticket-Entwürfe: Pflichtfelder, Zeilenbereiche, Indizes und Kreise.
export class TicketDraftValidator {
  // Liefert die brauchbaren Entwürfe (höchstens `max`) und Hinweise auf Verworfenes; ohne brauchbaren Entwurf ein Fehler
  check(items: unknown, doc: { lines: number; max?: number }): TicketDraftCheck {
    if (!Array.isArray(items)) throw new HarnessError('analysis-failed', 'Antwort ist keine Liste von Entwürfen.');
    const dropped: string[] = [];
    const max = doc.max ?? MAX_DRAFTS;
    const kept = items.slice(0, max);
    if (items.length > max) dropped.push(`Nur die ersten ${max} von ${items.length} Entwürfen übernommen.`);

    // Erst die Pflichtfelder prüfen und dabei die alte Position merken
    const positions = new Map<number, number>();
    const drafts: TicketDraft[] = [];
    const raw: unknown[] = [];
    kept.forEach((item, index) => {
      const i = (item ?? {}) as Record<string, unknown>;
      const title = text(i.title);
      const description = text(i.description);
      const acceptance = text(i.acceptance);
      if (!title || !description || !acceptance) {
        dropped.push(`Entwurf ${index + 1} verworfen: Titel, Beschreibung oder Abnahme fehlt.`);
        return;
      }
      positions.set(index, drafts.length);
      raw.push(i.dependsOn);
      drafts.push({
        title, description, acceptance,
        steps: list(i.steps), dependsOn: [], labels: list(i.labels),
        source: this.source(i.source, doc.lines),
      });
    });
    if (drafts.length === 0) throw new HarnessError('analysis-failed', 'Die Antwort enthält keinen brauchbaren Entwurf.');

    this.link(drafts, raw, positions, dropped);
    this.breakCycles(drafts, dropped);
    return { drafts, dropped };
  }

  // Zeilenbereich auf das Dokument begrenzen; unbrauchbare Angaben werden zur ersten Zeile
  private source(value: unknown, lines: number): TicketDraftSource {
    const s = (value ?? {}) as Record<string, unknown>;
    const heading = text(s.heading);
    const start = Number(s.startLine);
    const end = Number(s.endLine);
    const ok = Number.isInteger(start) && Number.isInteger(end) && start >= 1 && end >= start && start <= lines;
    if (!ok) return { heading, startLine: 1, endLine: 1 };
    return { heading, startLine: start, endLine: Math.min(end, lines) };
  }

  // dependsOn über die alten auf die neuen Positionen umrechnen; Unbrauchbares fällt weg
  private link(drafts: TicketDraft[], raw: unknown[], positions: Map<number, number>, dropped: string[]): void {
    drafts.forEach((item, index) => {
      const values = Array.isArray(raw[index]) ? (raw[index] as unknown[]) : [];
      const seen = new Set<number>();
      for (const value of values) {
        const old = Number(value);
        const next = Number.isInteger(old) ? positions.get(old) : undefined;
        if (next === undefined || next === index || seen.has(next)) {
          dropped.push(`Abhängigkeit „${String(value)}“ bei „${item.title}“ entfernt.`);
          continue;
        }
        seen.add(next);
      }
      item.dependsOn = [...seen];
    });
  }

  // Kanten der Reihe nach aufnehmen; jede Kante, die einen Kreis schließt, wird entfernt
  private breakCycles(drafts: TicketDraft[], dropped: string[]): void {
    const edges = new Map<number, Set<number>>(drafts.map((_, i) => [i, new Set<number>()]));
    drafts.forEach((item, to) => {
      const keep: number[] = [];
      for (const from of item.dependsOn) {
        if (this.reaches(edges, to, from)) {
          dropped.push(`Abhängigkeit von „${drafts[from].title}“ zu „${item.title}“ entfernt: schließt einen Kreis.`);
          continue;
        }
        edges.get(from)!.add(to);
        keep.push(from);
      }
      item.dependsOn = keep;
    });
  }

  // Erreicht `from` den Knoten `to` über die schon aufgenommenen Kanten?
  private reaches(edges: Map<number, Set<number>>, from: number, to: number): boolean {
    const stack = [from];
    const seen = new Set<number>();
    while (stack.length > 0) {
      const node = stack.pop()!;
      if (node === to) return true;
      if (seen.has(node)) continue;
      seen.add(node);
      stack.push(...(edges.get(node) ?? []));
    }
    return false;
  }
}
