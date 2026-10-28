// Kopiert das Fixture-Repo in einen frischen Temp-Ordner. Die Tests arbeiten nur auf dieser Kopie – im echten
// Repo darf kein Test eine Version verschieben.

import { cpSync, mkdirSync, readFileSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

// Mini-Abbild eines Repos mit allen Versionsstellen
const FIXTURE = fileURLToPath(new URL('./fixtures/repo', import.meta.url));

// Eine Kopie des Fixture-Repos im Temp-Ordner.
export class RepoCopy {
  constructor() {
    this.root = mkdtempSync(join(tmpdir(), 'harness-release-'));
    cpSync(FIXTURE, this.root, { recursive: true });
  }

  // Pfad einer Datei in der Kopie (Pfad immer mit `/`)
  path(file) {
    return join(this.root, ...file.split('/'));
  }

  // Inhalt einer Datei der Kopie
  read(file) {
    return readFileSync(this.path(file), 'utf8');
  }

  // Inhalt derselben Datei im Fixture – für den Vergleich „unverändert“
  original(file) {
    return readFileSync(join(FIXTURE, ...file.split('/')), 'utf8');
  }

  // Schreibt eine Datei der Kopie (etwa um eine Stelle absichtlich falsch zu setzen)
  write(file, text) {
    const target = this.path(file);
    mkdirSync(dirname(target), { recursive: true });
    writeFileSync(target, text);
  }

  // Entfernt eine Datei der Kopie
  remove(file) {
    rmSync(this.path(file), { force: true });
  }

  dispose() {
    rmSync(this.root, { recursive: true, force: true });
  }
}
