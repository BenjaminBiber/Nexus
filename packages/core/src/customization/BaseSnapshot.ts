import { spawnSync } from 'node:child_process';
import { cpSync, existsSync, mkdirSync, readdirSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import type { BaseState, Problem } from './CustomizationOverview.js';

export interface BaseCheck {
  state: BaseState;
  warnings: Problem[];
  diff?: string;
}

// Basis einer Anpassung: Kopie des Standards zum Zeitpunkt des Kopierens; vergleicht sie mit dem aktuellen Standard.
export class BaseSnapshot {
  // Einheitliches JSON für Workflow und Basis (zwei Leerzeichen, Zeilenende am Schluss)
  static formatJson(value: unknown): string {
    return `${JSON.stringify(value, null, 2)}\n`;
  }

  // Legt die Basis als Kopie des Standards (Datei oder Ordner) an; eine vorhandene wird ersetzt
  save(standard: string, base: string): void {
    rmSync(base, { recursive: true, force: true });
    mkdirSync(dirname(base), { recursive: true });
    cpSync(standard, base, { recursive: true });
  }

  // Legt die Basis aus einem Text an (Workflow als JSON)
  saveText(text: string, base: string): void {
    mkdirSync(dirname(base), { recursive: true });
    writeFileSync(base, text);
  }

  // unknown ohne Basis, current bei gleichem Inhalt, sonst outdated
  state(base: string, standard: string): BaseState {
    if (!existsSync(base)) return 'unknown';
    return this.same(this.tree(base), this.tree(standard)) ? 'current' : 'outdated';
  }

  // Unterschiede Basis → Standard über `git diff --no-index` (Exit-Code 1 bedeutet „unterschiedlich“, kein Fehler)
  diff(base: string, standard: string): string {
    const result = spawnSync('git', ['diff', '--no-index', '--no-color', '--', base, standard], { encoding: 'utf8', windowsHide: true });
    return (result.stdout ?? '').trim();
  }

  // Zustand mit Warnung: veraltet mit Diff, unbekannt mit Hinweis, aktuell ohne
  check(base: string, standard: string): BaseCheck {
    const state = this.state(base, standard);
    if (state === 'outdated') {
      return { state, warnings: [{ code: 'base-outdated', message: 'Der Standard hat sich seit deiner Kopie geändert.' }], diff: this.diff(base, standard) };
    }
    if (state === 'unknown') {
      return { state, warnings: [{ code: 'base-unknown', message: 'Keine Basis vermerkt; ein geänderter Standard wird nicht erkannt.' }] };
    }
    return { state, warnings: [] };
  }

  // Alle Dateien unter dem Pfad mit Inhalt; Schlüssel relativ zum Pfad mit `/`
  private tree(path: string): Map<string, string> {
    const files = new Map<string, string>();
    if (!existsSync(path)) return files;
    if (statSync(path).isFile()) return files.set('', this.read(path));
    for (const rel of readdirSync(path, { recursive: true }) as string[]) {
      const full = join(path, rel);
      if (statSync(full).isFile()) files.set(rel.split('\\').join('/'), this.read(full));
    }
    return files;
  }

  // Liest eine Datei mit einheitlichen Zeilenenden
  private read(file: string): string {
    return readFileSync(file, 'utf8').replace(/\r\n/g, '\n');
  }

  // Gleiche Dateien mit gleichem Inhalt?
  private same(a: Map<string, string>, b: Map<string, string>): boolean {
    if (a.size !== b.size) return false;
    for (const [key, value] of a) if (b.get(key) !== value) return false;
    return true;
  }
}
