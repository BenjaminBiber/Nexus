import { realpathSync } from 'node:fs';
import { resolve } from 'node:path';

// Vergleichbare Schreibweise eines Ordners: Pfade aus Git, aus der Registry und vom Dispatcher können denselben Ordner
// unterschiedlich nennen (unter Windows 8.3-Kurzname wie FLORIA~1.PAL, Groß-/Kleinschreibung, Rückstriche).
export class PathKey {
  constructor(private readonly platform: NodeJS.Platform = process.platform) {}

  // Schlüssel: aufgelöst, bei vorhandenem Ordner kanonisch (lange Namen), Schrägstriche, ohne Endstrich, unter Windows klein
  of(path: string): string {
    let full = resolve(path);
    try {
      full = realpathSync.native(full);
    } catch {
      // Ordner gibt es (noch) nicht: aufgelöster Pfad genügt
    }
    const slashed = full.replace(/\\/g, '/').replace(/\/+$/, '');
    return this.platform === 'win32' ? slashed.toLowerCase() : slashed;
  }

  // Gleicher Ordner?
  same(a: string, b: string): boolean {
    return this.of(a) === this.of(b);
  }
}
