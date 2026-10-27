import { HarnessError } from '../errors/HarnessError.js';

// Projektkey prüfen und Ticket-IDs der Form „ARC-0042“ bilden und zerlegen.
export class TicketIds {
  // Großbuchstaben und Ziffern, 2–6 Zeichen, beginnt mit einem Buchstaben
  normalizeKey(input: string): string {
    const key = input.toUpperCase().replace(/[^A-Z0-9]/g, '');
    if (!/^[A-Z][A-Z0-9]{1,5}$/.test(key)) {
      throw new HarnessError('project-key-invalid', `Projektkey „${input}“ ist ungültig: 2–6 Buchstaben oder Ziffern, beginnend mit einem Buchstaben.`);
    }
    return key;
  }

  // Bildet die ID mit mindestens vier Ziffern
  format(key: string, n: number): string {
    return `${key}-${String(n).padStart(4, '0')}`;
  }

  // Zerlegt eine ID in Key und Nummer
  parse(id: string): { key: string; number: number } {
    const match = /^([A-Za-z][A-Za-z0-9]{1,5})-(\d+)$/.exec(id.trim());
    if (!match) throw new HarnessError('ticket-id-invalid', `„${id}“ ist keine Ticket-ID (erwartet z. B. ARC-0042).`);
    return { key: match[1].toUpperCase(), number: Number(match[2]) };
  }

  // Schreibweise vereinheitlichen (Großbuchstaben, vier Ziffern)
  canonical(id: string): string {
    const { key, number } = this.parse(id);
    return this.format(key, number);
  }
}
