import { HarnessError, StatusParser, type BoardColumn, type BoardTicketStatus } from '@harness/core';

// Übersetzt CLI-Texte in Werte für die Board-Dienste.
export class BoardOptionParser {
  private readonly statuses = new StatusParser();

  // Ein Status (deutsche Aliase erlaubt)
  status(value: string): BoardTicketStatus {
    return this.statuses.parse(value);
  }

  // Kommagetrennte Spalten; „wartet“/„waiting“ ist hier erlaubt
  columns(value: string): BoardColumn[] {
    return value.split(',').map((v) => v.trim()).filter(Boolean).map((v) =>
      ['wartet', 'waiting'].includes(v.toLowerCase()) ? 'waiting' : this.statuses.parse(v));
  }

  // Kommagetrennte Liste oder undefined
  list(value: string | undefined): string[] | undefined {
    return value === undefined ? undefined : value.split(',').map((v) => v.trim()).filter(Boolean);
  }

  // Voraussetzungen; „none“/„keine“ leert die Liste
  dependsOn(value: string): string[] {
    return ['none', 'keine'].includes(value.trim().toLowerCase()) ? [] : this.list(value)!;
  }

  // true/false (alles außer „false“ gilt als true, wie im alten Board)
  bool(value: string): boolean {
    return value.trim().toLowerCase() !== 'false';
  }

  // Erwartete Version
  rev(value: string | undefined): number | undefined {
    if (value === undefined) return undefined;
    const rev = Number(value);
    if (!Number.isInteger(rev) || rev < 1) throw new HarnessError('rev-invalid', `„${value}“ ist keine Versionsnummer.`);
    return rev;
  }
}

// Sammelt wiederholbare Optionen (z. B. --label a --label b)
export function collect(value: string, previous?: string[]): string[] {
  return [...(previous ?? []), value];
}
