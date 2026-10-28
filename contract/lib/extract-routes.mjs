import { readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { join, relative, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { JsonText } from './JsonText.mjs';

const REGISTRATION = /app\.(get|post|put|patch|delete|head|options)\(\s*'([^']+)'/g;
// Rückfall einer Datei: beantwortet alle übrigen Adressen (Seiten der Oberfläche, fehlende Dateien)
const NOT_FOUND = /app\.setNotFoundHandler\(/;
// Muster des Rückfalls; nur lesend, weil Seiten der Oberfläche und Dateien nur mit GET geholt werden
const NOT_FOUND_ROUTE = { method: 'GET', pattern: '/*' };

// Liest die Routen aus `packages/server/src/routes/*.ts` (Methode, Muster, Quelldatei) für `contract/api/routes.json`.
export class RouteExtractor {
  // `repoRoot`: Wurzel des Projekts
  constructor(repoRoot) {
    this.repoRoot = repoRoot;
    this.dir = join(repoRoot, 'packages', 'server', 'src', 'routes');
  }

  // Alle Routen, nach Quelldatei, Muster und Methode sortiert
  extract() {
    const routes = [];
    for (const name of readdirSync(this.dir).sort()) {
      if (!name.endsWith('.ts')) continue;
      const file = relative(this.repoRoot, join(this.dir, name)).split(sep).join('/');
      const source = readFileSync(join(this.dir, name), 'utf8');
      for (const match of source.matchAll(REGISTRATION)) {
        routes.push({ method: match[1].toUpperCase(), pattern: match[2], file });
      }
      // Der Rückfall hat kein Muster im Quellcode; er steht als `GET /*` in der Liste
      if (NOT_FOUND.test(source)) routes.push({ ...NOT_FOUND_ROUTE, file });
    }
    return routes.sort((a, b) => a.file.localeCompare(b.file) || a.pattern.localeCompare(b.pattern) || a.method.localeCompare(b.method));
  }

  // Schreibt die Liste nach `contract/api/routes.json`
  write(target) {
    const routes = this.extract();
    writeFileSync(target, `${JsonText.canonical(routes)}\n`);
    return routes;
  }
}

// Aufruf: node contract/lib/extract-routes.mjs
if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  const contractDir = join(fileURLToPath(new URL('..', import.meta.url)));
  const repoRoot = join(contractDir, '..');
  const target = join(contractDir, 'api', 'routes.json');
  const routes = new RouteExtractor(repoRoot).write(target);
  console.log(`${routes.length} Routen nach ${relative(repoRoot, target).split(sep).join('/')} geschrieben.`);
}
