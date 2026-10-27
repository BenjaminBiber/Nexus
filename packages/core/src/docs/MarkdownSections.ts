import { HarnessError } from '../errors/HarnessError.js';

// Ein Abschnitt eines Markdown-Dokuments. Zeilennummern sind 1-basiert und einschließlich,
// passend zu `lines` in DocComment – so lassen sich Kommentare und Abschnitte vergleichen.
export interface MarkdownSection {
  heading: string;
  level: number;
  /** Zeile der Überschrift */
  startLine: number;
  /** Letzte Zeile vor der nächsten Überschrift gleicher oder höherer Ebene, sonst das Dateiende */
  endLine: number;
}

const HEADING = /^(#{1,6})\s+(.*)$/;
const FENCE = /^(`{3,}|~{3,})/;

// Zerlegt ein Markdown-Dokument in Abschnitte und ersetzt Zeilenbereiche, ohne den Rest anzurühren.
export class MarkdownSections {
  // Alle Überschriften mit ihrem Zeilenbereich; Text vor der ersten Überschrift wird kein Abschnitt
  parse(text: string): MarkdownSection[] {
    const lines = text.split(/\r?\n/);
    const sections: MarkdownSection[] = [];
    let fence: string | null = null;
    for (let i = 0; i < lines.length; i++) {
      const line = lines[i] ?? '';
      const fenceMatch = FENCE.exec(line.trim());
      if (fenceMatch?.[1]) {
        const marker = fenceMatch[1];
        // Ein Zaun endet nur durch denselben Zeichentyp in mindestens derselben Länge – sonst schließt ``` einen ~~~-Block
        if (fence === null) fence = marker;
        else if (marker[0] === fence[0] && marker.length >= fence.length) fence = null;
        continue;
      }
      if (fence !== null) continue;
      const match = HEADING.exec(line);
      if (!match?.[1]) continue;
      const level = match[1].length;
      // Alle offenen Abschnitte gleicher oder tieferer Ebene enden in der Zeile davor
      for (const open of sections) if (open.endLine === 0 && open.level >= level) open.endLine = i;
      sections.push({ heading: (match[2] ?? '').trim(), level, startLine: i + 1, endLine: 0 });
    }
    for (const open of sections) if (open.endLine === 0) open.endLine = lines.length;
    return sections.sort((a, b) => a.startLine - b.startLine);
  }

  // Der erste Abschnitt mit genau dieser Überschrift, sonst null
  find(text: string, heading: string): MarkdownSection | null {
    const wanted = heading.trim();
    return this.parse(text).find((s) => s.heading === wanted) ?? null;
  }

  // Ersetzt den 1-basierten, einschließenden Zeilenbereich; Zeilenende-Stil und abschließender Umbruch bleiben erhalten
  replaceLines(text: string, startLine: number, endLine: number, replacement: string): string {
    const eol = text.includes('\r\n') ? '\r\n' : '\n';
    const lines = text.split(/\r?\n/);
    if (startLine < 1 || endLine < startLine || endLine > lines.length) {
      throw new HarnessError(
        'doc-range-invalid',
        `Zeilenbereich ${startLine}–${endLine} liegt nicht im Dokument (1–${lines.length}). Lies das Dokument neu und nimm die Zeilen daraus.`,
      );
    }
    const replaced = replacement.split(/\r?\n/);
    lines.splice(startLine - 1, endLine - startLine + 1, ...replaced);
    return lines.join(eol);
  }
}
