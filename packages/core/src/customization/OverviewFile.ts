import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import type { HarnessHome } from '../registry/HarnessHome.js';
import type { CustomizationOverview } from './CustomizationOverview.js';

// Liest die Übersicht des letzten Anwendens aus `~/.harness/customizations/overview.json`.
export class OverviewFile {
  constructor(private readonly home: HarnessHome) {}

  // null, wenn die Datei fehlt, unlesbar ist oder nicht die erwartete Form hat (Harness wurde noch nicht eingerichtet)
  read(): CustomizationOverview | null {
    try {
      const parsed: unknown = JSON.parse(readFileSync(join(this.home.customizationsDir, 'overview.json'), 'utf8'));
      return this.isOverview(parsed) ? parsed : null;
    } catch {
      return null;
    }
  }

  // Prüft, ob der geparste Inhalt ein Objekt mit einem Feld `entries` als Liste ist
  private isOverview(value: unknown): value is CustomizationOverview {
    return typeof value === 'object' && value !== null && !Array.isArray(value)
      && Array.isArray((value as { entries?: unknown }).entries);
  }
}
