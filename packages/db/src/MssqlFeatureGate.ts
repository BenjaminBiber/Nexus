import { HarnessError } from '@harness/core';
import { SchemaVersionGuard } from './SchemaVersionGuard.js';

// Sperrt genau die Zugriffe, die eine noch nicht eingespielte Erweiterung brauchen. Alles andere am Board läuft weiter,
// damit eine laufende Agenten-Session ihren Meldeweg behält.
export class MssqlFeatureGate {
  constructor(
    private readonly missingFeatures: string[],
    private readonly database: string,
  ) {}

  // Wirft mit Hinweis auf das Skript, sobald eine der verlangten Erweiterungen fehlt (in der Reihenfolge der Skripte)
  require(...features: string[]): void {
    const fehlend = features.find((feature) => this.missingFeatures.includes(feature));
    if (fehlend === undefined) return;
    throw new HarnessError('board-schema-outdated', SchemaVersionGuard.featureMessage(fehlend, this.database));
  }

  // Ob die Erweiterung da ist (für Zugriffe, die die Quelle lieber auslassen als zu scheitern)
  has(feature: string): boolean {
    return !this.missingFeatures.includes(feature);
  }
}
