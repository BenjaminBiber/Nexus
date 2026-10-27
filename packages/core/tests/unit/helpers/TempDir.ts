import { mkdtempSync, realpathSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

// Legt einen temporären Ordner an und liefert seinen kanonischen langen Pfad.
// Bewusst realpathSync.native: das JS-realpathSync löst unter Windows nur Junctions auf,
// aber keine 8.3-Kurznamen (TEMP steht oft als C:\Users\BENJAM~1.BIB\...). Git liefert
// Pfade immer lang zurück, sonst scheitern alle Pfadvergleiche in den Tests.
export function makeTempDir(prefix: string): string {
  return realpathSync.native(mkdtempSync(join(tmpdir(), prefix)));
}
