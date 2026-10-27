import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { AtomicWriter, DOC_FILE_NAME, DocHash, HarnessError, MarkdownSections, SuperpowersDocScanner, type MarkdownSection } from '@harness/core';
import type { DocsSource } from './DocsRootResolver.js';

export interface DocFileDto {
  path: string;
  title: string;
  group: 'spec' | 'plan' | 'glossary' | 'adr';
  /** Sorte des Superpowers-Dokuments; bei Glossar und ADRs null */
  kind: 'spec' | 'plan' | null;
  topic: string | null;
  date: string | null;
  mtimeMs: number;
  /** bleibt erhalten, damit Web und Liste ohne Umbau weiterlaufen; neue Quellen liefern immer true */
  exists: boolean;
  /** Zur Datei sind Ticket-Entwürfe gespeichert (gesetzt von der Docs-Route) */
  hasDrafts?: boolean;
}

export interface DocsListDto {
  source: DocsSource;
  docs: DocFileDto[];
}

export interface DocFileContentDto {
  path: string;
  content: string;
  hash: string;
  sections: MarkdownSection[];
}

export interface WriteSectionInput {
  path: string;
  /** 1-basiert und einschließlich, wie `MarkdownSection` und `DocComment.lines` */
  startLine: number;
  endLine: number;
  content: string;
  /** Hash aus dem letzten Lesen; weicht er ab, hat jemand anders geschrieben */
  expectedHash: string;
}

export interface WriteSectionResult {
  path: string;
  hash: string;
  sections: MarkdownSection[];
}

export interface SearchHitDto {
  path: string;
  line: number;
  excerpt: string;
}

// Dateiname ohne Anker – dasselbe Muster, nach dem der Scanner auflistet, damit jedes gelistete Dokument auch lesbar ist
const NAME = DOC_FILE_NAME.source.slice(1, -1);
// Nur diese Dateien sind lesbar (Superpowers-Specs und -Pläne, Glossar, ADRs); alles andere ist gesperrt.
const ALLOWED = [
  new RegExp(`^docs/superpowers/(specs|plans)/${NAME}$`),
  /^GLOSSARY\.md$/,
  new RegExp(`^docs/adr/${NAME}$`),
];
// Geschrieben werden nur Superpowers-Specs und -Pläne; Glossar und ADRs sind lesbar, aber nicht bearbeitbar.
const WRITABLE = new RegExp(`^docs/superpowers/(specs|plans)/${NAME}$`);
const MAX_HITS = 100;

// Liest und durchsucht die freigegebenen Dokumente im Repo des Boards und ersetzt einzelne Abschnitte.
export class DocsService {
  private readonly scanner = new SuperpowersDocScanner();
  private readonly sections = new MarkdownSections();
  private readonly writer = new AtomicWriter();
  // Laufende Schreibvorgänge je Datei: Hash-Prüfung und Schreiben einer Datei laufen nacheinander, sonst bestünden zwei
  // gleichzeitige Änderungen gegen denselben Stand beide die Prüfung und die zweite überschriebe die erste
  private readonly writes = new Map<string, Promise<unknown>>();

  // Liste der Dokumente samt Quelle; ohne Quelle bleibt die Liste leer und `source.reason` nennt den Grund
  list(source: DocsSource): DocsListDto {
    const root = source.root;
    if (!root) return { source, docs: [] };
    const docs: DocFileDto[] = [];
    for (const doc of this.scanner.scan(root)) {
      docs.push({
        path: doc.path, title: doc.title, group: doc.kind, kind: doc.kind,
        topic: doc.topic, date: doc.date, mtimeMs: doc.mtimeMs, exists: true,
      });
    }
    if (existsSync(join(root, 'GLOSSARY.md'))) docs.push(this.plain(root, 'GLOSSARY.md', 'Glossar', 'glossary'));
    for (const name of this.files(join(root, 'docs', 'adr'))) {
      docs.push(this.plain(root, `docs/adr/${name}`, name.replace(/\.md$/, ''), 'adr'));
    }
    return { source, docs };
  }

  // Inhalt einer freigegebenen Datei mit Hash und Abschnitten; alles andere (auch Ausbruchsversuche) ergibt „nicht gefunden“
  read(source: DocsSource, path: string | undefined): DocFileContentDto {
    const full = this.fullPath(source, path);
    const content = readFileSync(full, 'utf8');
    return { path: path!, content, hash: DocHash.of(content), sections: this.sections.parse(content) };
  }

  // Voller Pfad einer freigegebenen, vorhandenen Datei (zum Lesen und zum Öffnen in Explorer oder VS Code)
  fullPath(source: DocsSource, path: string | undefined): string {
    if (!source.root || !path || !this.isAllowed(path)) {
      throw new HarnessError('doc-not-found', 'Diese Datei gibt es nicht oder sie ist nicht freigegeben.');
    }
    const full = join(source.root, path);
    if (!existsSync(full) || !statSync(full).isFile()) {
      throw new HarnessError('doc-not-found', `Die Datei ${path} existiert noch nicht.`);
    }
    return full;
  }

  // Ersetzt einen Zeilenbereich einer Spec oder eines Plans, wenn der Hash noch zum gelesenen Stand passt
  writeSection(source: DocsSource, input: WriteSectionInput): Promise<WriteSectionResult> {
    const key = `${source.root ?? ''}|${input.path ?? ''}`.toLowerCase();
    const previous = this.writes.get(key) ?? Promise.resolve();
    const run = previous.then(() => this.replaceSection(source, input));
    const settled = run.catch(() => undefined);
    this.writes.set(key, settled);
    // Ist niemand mehr dahinter eingereiht, wird der Eintrag entfernt
    void settled.then(() => { if (this.writes.get(key) === settled) this.writes.delete(key); });
    return run;
  }

  // Prüft den Hash gegen den Stand auf der Platte und schreibt; läuft je Datei nur einmal gleichzeitig
  private async replaceSection(source: DocsSource, input: WriteSectionInput): Promise<WriteSectionResult> {
    // Erst lesen: ein nicht freigegebener oder fehlender Pfad bleibt so „nicht gefunden“ und verrät nichts über das Dateisystem
    const current = this.read(source, input.path);
    if (!WRITABLE.test(current.path)) {
      throw new HarnessError('doc-not-writable', 'Glossar und ADRs lassen sich hier nicht bearbeiten.');
    }
    if (!input.expectedHash || input.expectedHash !== current.hash) {
      // Der aktuelle Hash geht mit, damit der Client den Unterschied anzeigen kann, ohne erneut zu lesen
      throw new HarnessError(
        'doc-changed',
        'Das Dokument wurde zwischenzeitlich geändert. Lade es neu und wiederhole die Änderung.',
        { hash: current.hash },
      );
    }
    // replaceLines prüft den Bereich und behält Zeilenende-Stil und abschließenden Umbruch bei
    const next = this.sections.replaceLines(current.content, input.startLine, input.endLine, input.content);
    await this.writer.write(join(source.root as string, current.path), next);
    return { path: current.path, hash: DocHash.of(next), sections: this.sections.parse(next) };
  }

  // Wirft „nicht gefunden“, wenn das Dokument nicht freigegeben ist oder fehlt (für Kommentare)
  assertDocument(source: DocsSource, path: string | undefined): string {
    return this.read(source, path).path;
  }

  // Teilstringsuche ohne Beachtung der Schreibweise über alle freigegebenen Dateien
  search(source: DocsSource, query: string): SearchHitDto[] {
    const needle = query.trim().toLowerCase();
    if (needle.length < 2) throw new HarnessError('query-short', 'Bitte mindestens zwei Zeichen eingeben.');
    const hits: SearchHitDto[] = [];
    const root = source.root;
    if (!root) return hits;
    for (const doc of this.list(source).docs) {
      const lines = readFileSync(join(root, doc.path), 'utf8').split(/\r?\n/);
      lines.forEach((text, index) => {
        if (hits.length < MAX_HITS && text.toLowerCase().includes(needle)) {
          hits.push({ path: doc.path, line: index + 1, excerpt: text.trim().slice(0, 160) });
        }
      });
    }
    return hits;
  }

  // Eintrag für eine Datei ohne Thema und Datum (Glossar, ADR)
  private plain(root: string, path: string, title: string, group: 'glossary' | 'adr'): DocFileDto {
    return { path, title, group, kind: null, topic: null, date: null, mtimeMs: statSync(join(root, path)).mtimeMs, exists: true };
  }

  // Erlaubt nur genau passende Pfade ohne Rückwärtsstriche und ohne „..“
  private isAllowed(path: string): boolean {
    if (path.includes('\\') || path.includes('..') || path.startsWith('/')) return false;
    return ALLOWED.some((pattern) => pattern.test(path));
  }

  // Markdown-Dateien eines Ordners, deren Name auch lesbar ist (DOC_FILE_NAME)
  private files(dir: string): string[] {
    if (!existsSync(dir)) return [];
    return readdirSync(dir, { withFileTypes: true }).filter((e) => e.isFile() && DOC_FILE_NAME.test(e.name)).map((e) => e.name).sort();
  }
}
