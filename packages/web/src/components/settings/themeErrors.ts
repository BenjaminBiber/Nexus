import { ApiError } from '../../api/ApiError.js';
import type { ManifestError } from '../../api/generated/themes.js';

// Macht aus einem Fehler beim Installieren oder Entfernen eines Themes eine deutsche Meldung; bei Manifestfehlern mit Feldliste
export function describeThemeError(err: unknown): { message: string; fields: string[] } {
  if (!(err instanceof ApiError)) return { message: err instanceof Error ? err.message : String(err), fields: [] };
  switch (err.code) {
    case 'theme-manifest-invalid': {
      const errors = (err.details?.errors ?? []) as ManifestError[];
      return { message: 'Die manifest.json des Themes ist fehlerhaft.', fields: errors.map((e) => (e.field ? `${e.field}: ${e.message}` : e.message)) };
    }
    case 'theme-package-invalid':
      return { message: `Die Datei ist kein gültiges Theme-Paket: ${err.message}`, fields: [] };
    case 'theme-not-found':
      return { message: 'Das Theme ist nicht (mehr) installiert.', fields: [] };
    case 'theme-id-invalid':
      return { message: `Die Kennung des Themes ist ungültig: ${err.message}`, fields: [] };
    case 'bad-request':
      return { message: `Die Datei konnte nicht übertragen werden: ${err.message}`, fields: [] };
    default:
      return { message: err.message, fields: [] };
  }
}
