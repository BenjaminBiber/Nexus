import { readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { join, relative, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { JsonText } from './JsonText.mjs';

// Anfang einer Anmeldung in axum: `.route("<muster>", …)`; die Methoden stehen im Rest des Aufrufs
const REGISTRATION = /\.route\(\s*"([^"]+)"\s*,/g;
const METHOD = /\b(get|post|put|patch|delete|head|options)\s*\(/g;
// Rückfall des Routers in `mod.rs`: beantwortet alle übrigen Adressen (Seiten der Oberfläche,
// fehlende Dateien)
const NOT_FOUND = /\.fallback\(static_or_not_found\)\s*\n\s*\.layer\(/;
// Muster des Rückfalls; nur lesend, weil Seiten der Oberfläche und Dateien nur mit GET geholt werden
const NOT_FOUND_ROUTES = [
  { method: 'GET', pattern: '/' },
  { method: 'GET', pattern: '/*' },
];

// Liest die Routen aus `crates/nexus-server/src/routes/*.rs` (Methode, Muster, Quelldatei) für
// `contract/api/routes.json`. Die Platzhalter von axum (`{pid}`) stehen in der Liste wie im
// Vertrag in der Schreibweise von Fastify (`:pid`).
export class RouteExtractor {
  // `repoRoot`: Wurzel des Projekts
  constructor(repoRoot) {
    this.repoRoot = repoRoot;
    this.dir = join(repoRoot, 'crates', 'nexus-server', 'src', 'routes');
  }

  // Alle Routen, nach Quelldatei, Muster und Methode sortiert
  extract() {
    const routes = [];
    for (const name of readdirSync(this.dir).sort()) {
      if (!name.endsWith('.rs')) continue;
      const file = relative(this.repoRoot, join(this.dir, name)).split(sep).join('/');
      const source = readFileSync(join(this.dir, name), 'utf8');
      for (const match of source.matchAll(REGISTRATION)) {
        const pattern = match[1].replace(/\{([^}]+)\}/g, ':$1');
        const call = RouteExtractor.callBody(source, match.index + match[0].length - 1);
        for (const method of call.matchAll(METHOD)) {
          routes.push({ method: method[1].toUpperCase(), pattern, file });
        }
      }
      // Der Rückfall hat kein Muster im Quellcode; er steht als `GET /` und `GET /*` in der Liste
      if (NOT_FOUND.test(source)) for (const route of NOT_FOUND_ROUTES) routes.push({ ...route, file });
    }
    return routes.sort((a, b) => a.file.localeCompare(b.file) || a.pattern.localeCompare(b.pattern) || a.method.localeCompare(b.method));
  }

  // Text eines Aufrufs ab dem Komma hinter dem Muster bis zur schließenden Klammer von `.route(`
  static callBody(source, from) {
    let depth = 1;
    for (let i = from; i < source.length; i += 1) {
      if (source[i] === '(') depth += 1;
      else if (source[i] === ')') {
        depth -= 1;
        if (depth === 0) return source.slice(from, i);
      }
    }
    return source.slice(from);
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
