import { createHash } from 'node:crypto';
import { existsSync, readFileSync, readdirSync, statSync } from 'node:fs';
import { join } from 'node:path';

// Welche der beiden Sorten von Dokumenten vorliegt: die Spec (…-design.md) oder der Plan.
export type SuperpowersDocKind = 'spec' | 'plan';

// Ein gefundenes Markdown-Dokument samt der Angaben, die Anzeige und Cache-Prüfung brauchen.
export interface SuperpowersDoc {
  kind: SuperpowersDocKind;
  /** Pfad relativ zum Repo-Wurzelordner, immer mit / – z. B. docs/superpowers/specs/2026-10-22-x-design.md */
  path: string;
  /** Dateiname ohne .md */
  slug: string;
  /** Thema: Dateiname ohne Datum und ohne Endung -design */
  topic: string;
  /** JJJJ-MM-TT aus dem Dateinamen oder null */
  date: string | null;
  /** erste H1-Überschrift, sonst der Dateiname ohne Endung */
  title: string;
  mtimeMs: number;
  size: number;
}

// Spec und Plan eines Themas als ein Eintrag; fehlt eine der beiden Seiten, bleibt sie null.
export interface SuperpowersTopic {
  topic: string;
  date: string | null;
  spec: SuperpowersDoc | null;
  plan: SuperpowersDoc | null;
}

// Erlaubte Dateinamen der Dokumente: nur diese lassen sich über die Docs-Seite auch lesen und bearbeiten
export const DOC_FILE_NAME = /^[A-Za-z0-9._-]+\.md$/;

const DIRS: readonly { kind: SuperpowersDocKind; dir: string }[] = [
  { kind: 'spec', dir: 'specs' },
  { kind: 'plan', dir: 'plans' },
];

// Findet und paart die Dokumente unter <ordner>/docs/superpowers/{specs,plans}; <ordner> ist der Repo-Pfad des Boards.
// Die Dateien sind nicht versioniert, deshalb liest der Scanner das Dateisystem und nicht Git.
export class SuperpowersDocScanner {
  // Alle Dokumente beider Ordner; fehlende Ordner ergeben eine leere Liste. Sortiert nach Datum absteigend (ohne Datum zuletzt), dann Thema
  scan(root: string): SuperpowersDoc[] {
    const docs: SuperpowersDoc[] = [];
    for (const { kind, dir } of DIRS) {
      for (const entry of this.entries(root, dir)) {
        const slug = entry.name.slice(0, -'.md'.length);
        const { date, topic } = parseSlug(slug);
        const stat = statSync(entry.full);
        docs.push({
          kind, slug, date, topic,
          path: `docs/superpowers/${dir}/${entry.name}`,
          title: firstHeading(entry.full) ?? slug,
          mtimeMs: stat.mtimeMs,
          size: stat.size,
        });
      }
    }
    return docs.sort(compareDocs);
  }

  // Die Dokumente nach Thema gruppiert, in derselben Reihenfolge wie scan
  topics(root: string): SuperpowersTopic[] {
    const byTopic = new Map<string, SuperpowersTopic>();
    for (const doc of this.scan(root)) {
      const entry = byTopic.get(doc.topic) ?? { topic: doc.topic, date: null, spec: null, plan: null };
      if (doc.kind === 'spec') entry.spec = doc;
      else entry.plan = doc;
      entry.date = entry.spec?.date ?? entry.plan?.date ?? null;
      byTopic.set(doc.topic, entry);
    }
    return [...byTopic.values()].sort((a, b) => compareDate(a.date, b.date) || a.topic.localeCompare(b.topic));
  }

  // Kurzer Hash über Pfad, Änderungszeit und Größe aller Dokumente – erkennt Änderungen, ohne die Dateien zu lesen
  signature(root: string): string {
    const lines: string[] = [];
    for (const { dir } of DIRS) {
      for (const entry of this.entries(root, dir)) {
        const stat = statSync(entry.full);
        lines.push(`docs/superpowers/${dir}/${entry.name}|${stat.mtimeMs}|${stat.size}`);
      }
    }
    return createHash('sha256').update(lines.sort().join('\n')).digest('hex').slice(0, 16);
  }

  // Die Dokumente eines der beiden Ordner (Namen nach DOC_FILE_NAME), ohne Rekursion; fehlt der Ordner, ist die Liste leer
  private entries(root: string, dir: string): { name: string; full: string }[] {
    const abs = join(root, 'docs', 'superpowers', dir);
    if (!existsSync(abs)) return [];
    return readdirSync(abs, { withFileTypes: true })
      .filter((e) => e.isFile() && DOC_FILE_NAME.test(e.name))
      .map((e) => ({ name: e.name, full: join(abs, e.name) }));
  }
}

// Zerlegt den Dateinamen ohne Endung in Datum und Thema; das abschließende -design der Spec gehört nicht zum Thema
function parseSlug(slug: string): { date: string | null; topic: string } {
  const match = /^(\d{4}-\d{2}-\d{2})-(.+)$/.exec(slug);
  const date = match?.[1] ?? null;
  const rest = match?.[2] ?? slug;
  return { date, topic: rest.replace(/-design$/, '') };
}

// Erste H1-Zeile der Datei; gibt es keine, entscheidet der Aufrufer über den Ersatz
function firstHeading(path: string): string | null {
  for (const line of readFileSync(path, 'utf8').split(/\r?\n/)) {
    const match = /^#\s+(.+)$/.exec(line);
    if (match?.[1]) return match[1].trim();
  }
  return null;
}

// Neueres Datum zuerst, Dokumente ohne Datum ganz hinten
function compareDate(a: string | null, b: string | null): number {
  if (a === b) return 0;
  if (a === null) return 1;
  if (b === null) return -1;
  return b.localeCompare(a);
}

function compareDocs(a: SuperpowersDoc, b: SuperpowersDoc): number {
  return compareDate(a.date, b.date) || a.topic.localeCompare(b.topic) || a.kind.localeCompare(b.kind);
}
