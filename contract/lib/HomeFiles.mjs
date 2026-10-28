import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';

// Legt die Dateien der Fixture `homeFiles` im HARNESS_HOME des Szenarios an: Zustand, Läufe, Protokolle und
// Einstellung des Dispatchers, die der Server sonst erst durch echte Arbeit bekäme. Diese Dateien gehören zum
// Vertrag – jede Umsetzung des Servers muss sie lesen können.
export class HomeFiles {
  // `home`: HARNESS_HOME des Szenarios
  constructor(home) {
    this.home = home;
  }

  // Schreibt alle Dateien; `variables` setzt `{{HOME}}`, `{{NOW}}` und die Repo-Pfade ein. Liefert die Pfade
  write(files, variables) {
    const written = [];
    for (const [path, content] of Object.entries(files)) {
      const target = join(this.home, ...path.split('/'));
      mkdirSync(dirname(target), { recursive: true });
      writeFileSync(target, HomeFiles.text(content, variables));
      written.push(target);
    }
    return written;
  }

  // Inhalt einer Datei: ein Text kommt wie er ist, alles andere als JSON mit zwei Leerzeichen Einrückung
  static text(content, variables) {
    const value = variables.apply(content);
    return typeof value === 'string' ? value : `${JSON.stringify(value, null, 2)}\n`;
  }
}
