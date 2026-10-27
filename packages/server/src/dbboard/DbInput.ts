import { HarnessError, StatusParser, type BoardTicketStatus, type Priority } from '@harness/core';

export type Body = Record<string, unknown>;

// Liest und prüft Eingaben der /api/db-Routen; Fehler sind HarnessError (422) mit deutscher Meldung.
export class DbInput {
  private readonly statuses = new StatusParser();

  // Body einer Anfrage als Objekt (fehlend oder kein Objekt = leer)
  body(raw: unknown): Body {
    return raw && typeof raw === 'object' && !Array.isArray(raw) ? (raw as Body) : {};
  }

  // Pflichttext
  text(body: Body, key: string): string {
    const value = body[key];
    if (typeof value !== 'string') throw new HarnessError('input-invalid', `Feld „${key}“ fehlt oder ist kein Text.`);
    return value;
  }

  // Optionaler Text
  optionalText(body: Body, key: string): string | undefined {
    return body[key] === undefined ? undefined : this.text(body, key);
  }

  // Optionaler Text, der auch null sein darf (Feld leeren)
  nullableText(body: Body, key: string): string | null | undefined {
    return body[key] === null ? null : this.optionalText(body, key);
  }

  // Optionaler Schalter
  optionalBool(body: Body, key: string): boolean | undefined {
    const value = body[key];
    if (value === undefined) return undefined;
    if (typeof value !== 'boolean') throw new HarnessError('input-invalid', `Feld „${key}“ muss true oder false sein.`);
    return value;
  }

  // Optionale Liste von Texten
  optionalList(body: Body, key: string): string[] | undefined {
    const value = body[key];
    if (value === undefined) return undefined;
    if (!Array.isArray(value) || value.some((v) => typeof v !== 'string')) {
      throw new HarnessError('input-invalid', `Feld „${key}“ muss eine Liste von Texten sein.`);
    }
    return value as string[];
  }

  // Pflichtliste von Texten mit mindestens einem und höchstens max Einträgen
  requiredList(body: Body, key: string, max: number): string[] {
    const value = this.optionalList(body, key);
    if (!value || value.length === 0 || value.length > max) {
      throw new HarnessError('input-invalid', `Feld „${key}“ muss eine Liste mit 1 bis ${max} Einträgen sein.`);
    }
    return value;
  }

  // Optionale ganze Zahl
  optionalInt(body: Body, key: string): number | undefined {
    const value = body[key];
    if (value === undefined) return undefined;
    if (typeof value !== 'number' || !Number.isInteger(value)) throw new HarnessError('input-invalid', `Feld „${key}“ muss eine ganze Zahl sein.`);
    return value;
  }

  // Optionaler Status (deutsche Aliase erlaubt, „Wartet“ abgelehnt)
  optionalStatus(body: Body, key: string): BoardTicketStatus | undefined {
    const value = this.optionalText(body, key);
    return value === undefined ? undefined : this.statuses.parse(value);
  }

  // Optionale Priorität; gültige Werte prüft der Dienst
  optionalPriority(body: Body, key: string): Priority | undefined {
    return this.optionalText(body, key) as Priority | undefined;
  }

  // Positive Nummer aus der Adresse (Board, Notiz)
  id(raw: string, what: string): number {
    const id = Number(raw);
    if (!Number.isInteger(id) || id < 1) throw new HarnessError('id-invalid', `„${raw}“ ist keine gültige ${what}-Nummer.`);
    return id;
  }
}
