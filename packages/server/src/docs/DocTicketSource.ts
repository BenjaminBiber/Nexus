/** Stelle eines Dokuments, aus der ein Ticket entstanden ist */
export interface DocTicketSourceRef {
  path: string;
  heading: string;
  startLine: number;
  endLine: number;
}

// Trenner zwischen Pfad und Überschrift sowie Gedankenstrich im Zeilenbereich (U+2013) – beide Zeichen stehen so in den Beschreibungen
const LINE = /^Quelle: (.+?) › (.+) \(Z\. (\d+)–(\d+)\)$/;

// Quellenzeile für die Beschreibung eines Tickets; Schreiben und Lesen hängen an diesem einen Format
export function formatSource(docPath: string, heading: string, startLine: number, endLine: number): string {
  return `Quelle: ${docPath} › ${heading} (Z. ${startLine}–${endLine})`;
}

// Liest die Quellenzeile aus einer Beschreibung; ohne passende Zeile null.
// Gelesen wird die letzte Zeile, damit eine nachträglich angehängte Quelle gilt.
export function parseSource(description: string): DocTicketSourceRef | null {
  const lines = description.split(/\r?\n/);
  for (let i = lines.length - 1; i >= 0; i -= 1) {
    const match = LINE.exec(lines[i].trim());
    if (match) return { path: match[1], heading: match[2], startLine: Number(match[3]), endLine: Number(match[4]) };
  }
  return null;
}
