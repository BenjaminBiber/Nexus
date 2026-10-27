import { HarnessError } from '@harness/core';

export const EXCERPT_CHARS = 300;

/** Kürzt lange Ausgaben für Fehlermeldungen, damit der Client nicht ein ganzes Protokoll sieht */
export const excerpt = (text: string) => (text.length > EXCERPT_CHARS ? `${text.slice(0, EXCERPT_CHARS)} …` : text).trim();

export interface ClaudeResultText {
  text: string;
  costUsd: number | null;
}

// Packt das Ergebnis von `claude -p --output-format json` aus: die letzte Zeile mit type „result“ zählt.
// Gemeinsam genutzt von der Optimierungs-Analyse, dem Umschreiben eines Dokument-Bereichs und den Ticket-Entwürfen.
export class ClaudeResultEnvelope {
  // Antworttext und Kosten; fehlerhafte oder unlesbare Antworten werden zu `analysis-failed`
  parse(output: string): ClaudeResultText {
    for (const line of output.trim().split('\n').reverse()) {
      try {
        const parsed = JSON.parse(line) as { type?: string; subtype?: string; is_error?: boolean; result?: unknown; total_cost_usd?: unknown };
        if (parsed.type !== 'result') continue;
        if (parsed.is_error || parsed.subtype !== 'success' || typeof parsed.result !== 'string') {
          throw new HarnessError('analysis-failed', `Claude meldet einen Fehler (${parsed.subtype ?? 'unbekannt'}).`);
        }
        return { text: parsed.result, costUsd: typeof parsed.total_cost_usd === 'number' ? parsed.total_cost_usd : null };
      } catch (error) {
        if (error instanceof HarnessError) throw error;
      }
    }
    throw new HarnessError('analysis-failed', `Antwort nicht lesbar: ${excerpt(output)}`);
  }
}
