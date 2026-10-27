import { existsSync, readdirSync } from 'node:fs';
import { readFile } from 'node:fs/promises';
import { randomBytes } from 'node:crypto';
import { join } from 'node:path';
import { HarnessError } from '../errors/HarnessError.js';
import { AtomicWriter } from '../locking/AtomicWriter.js';
import type { DocComment, DocCommentPatch, NewDocComment, OpenDocComment } from './DocComment.js';

const SUFFIX = '.comments.json';
const ROOT_SIDECAR = 'GLOSSARY.comments.json';
// Ordner der Superpowers-Dokumente (Spec und Plan je Thema), wie SuperpowersDocScanner sie liest
const SUPERPOWERS_DIRS = ['docs/superpowers/specs', 'docs/superpowers/plans'];

// Thema eines Dokuments: Dateiname ohne Datum, ohne Endung -design und ohne .md (wie SuperpowersDocScanner)
function topicOf(docPath: string): string {
  const slug = docPath.slice(docPath.lastIndexOf('/') + 1).replace(/\.md$/, '');
  const rest = /^\d{4}-\d{2}-\d{2}-(.+)$/.exec(slug)?.[1] ?? slug;
  return rest.replace(/-design$/, '');
}

// Liest und schreibt Kommentare zu Dokumenten in Sidecar-Dateien (…comments.json) neben dem Dokument.
// Der Store prüft nicht, ob ein Dokument freigegeben ist: Der Aufrufer (Server) muss `docPath` gegen die
// erlaubten Dokumente prüfen. Lesen-Ändern-Schreiben läuft ohne Sperre und ist für einen Einzelnutzer gedacht.
export class DocCommentStore {
  private readonly writer = new AtomicWriter();

  constructor(private readonly root: string) {}

  // Sidecar-Pfad zu einem Dokumentpfad (spec.md → spec.comments.json); andere Pfade werden abgelehnt
  static sidecarOf(docPath: string): string {
    const escapes = docPath.startsWith('/') || docPath.includes('\\') || docPath.split('/').includes('..') || /^[A-Za-z]:/.test(docPath);
    if (!docPath.endsWith('.md') || escapes) {
      throw new HarnessError('doc-not-found', `Das Dokument ${docPath} gibt es nicht (nur .md-Dateien im Projekt sind erlaubt).`);
    }
    return docPath.replace(/\.md$/, SUFFIX);
  }

  // Alle Kommentare eines Dokuments (leer, wenn es noch keine Datei gibt)
  async list(docPath: string): Promise<DocComment[]> {
    const sidecar = DocCommentStore.sidecarOf(docPath);
    const file = join(this.root, sidecar);
    if (!existsSync(file)) return [];
    const raw = await readFile(file, 'utf8');
    try {
      const data = JSON.parse(raw) as { comments?: DocComment[] };
      return Array.isArray(data.comments) ? data.comments : [];
    } catch {
      throw new HarnessError('comments-corrupt', `Die Kommentardatei ${sidecar} ist beschädigt.`);
    }
  }

  // Fügt einen offenen Kommentar hinzu
  async add(docPath: string, input: NewDocComment): Promise<DocComment> {
    const comments = await this.list(docPath);
    const comment: DocComment = {
      id: `K-${randomBytes(4).toString('hex')}`,
      status: 'open',
      createdAt: new Date().toISOString(),
      resolution: null,
      ...input,
    };
    await this.save(docPath, [...comments, comment]);
    return comment;
  }

  // Ändert Text, Status oder Antwort eines Kommentars
  async update(docPath: string, id: string, patch: DocCommentPatch): Promise<DocComment> {
    const comments = await this.list(docPath);
    const index = this.indexOf(comments, id);
    const defined = Object.fromEntries(Object.entries(patch).filter(([, value]) => value !== undefined));
    comments[index] = { ...comments[index], ...defined };
    await this.save(docPath, comments);
    return comments[index];
  }

  // Löscht einen Kommentar
  async remove(docPath: string, id: string): Promise<void> {
    const comments = await this.list(docPath);
    comments.splice(this.indexOf(comments, id), 1);
    await this.save(docPath, comments);
  }

  // Offene Kommentare aller Dokumente; `scope` begrenzt auf ein Dokument (Pfad) oder ein Thema (Spec und Plan)
  async listOpen(scope?: string): Promise<OpenDocComment[]> {
    const result: OpenDocComment[] = [];
    for (const docPath of this.documentsWithComments(scope)) {
      for (const comment of await this.list(docPath)) {
        if (comment.status === 'open') result.push({ path: docPath, comment });
      }
    }
    return result;
  }

  // Erledigt einen Kommentar über seine ID in beliebigem Dokument
  async resolve(id: string, resolution: string): Promise<OpenDocComment> {
    for (const docPath of this.documentsWithComments()) {
      if ((await this.list(docPath)).some((c) => c.id === id)) {
        const comment = await this.update(docPath, id, { status: 'resolved', resolution });
        return { path: docPath, comment };
      }
    }
    throw new HarnessError('comment-not-found', `Kommentar ${id} gibt es nicht.`);
  }

  // Dokumentpfade (mit "/"), zu denen eine Sidecar-Datei existiert: die Superpowers-Specs und -Pläne, ohne `scope`
  // zusätzlich das Glossar im Stamm und die ADRs. `scope` ist entweder der Pfad eines Dokuments (genau dieses) oder
  // ein Thema – der Dateiname ohne Datum und ohne die Endung -design, der Spec und Plan desselben Themas trifft.
  private documentsWithComments(scope?: string): string[] {
    const superpowers = SUPERPOWERS_DIRS.flatMap((dir) => this.sidecarsIn(dir));
    if (!scope) return [...superpowers, ...this.sidecarsIn(''), ...this.sidecarsIn('docs/adr')];
    const normalized = scope.replace(/\\/g, '/');
    return superpowers.filter((docPath) => docPath === normalized || topicOf(docPath) === normalized);
  }

  // Dokumentpfade der Sidecar-Dateien in einem Ordner (relativ zum Projekt); im Stamm nur das Glossar
  private sidecarsIn(dir: string): string[] {
    const full = join(this.root, dir);
    if (!existsSync(full)) return [];
    return readdirSync(full)
      .filter((name) => name.endsWith(SUFFIX))
      .filter((name) => dir !== '' || name === ROOT_SIDECAR)
      .map((name) => (dir ? `${dir}/` : '') + name.replace(SUFFIX, '.md'));
  }

  // Position eines Kommentars oder Fehler
  private indexOf(comments: DocComment[], id: string): number {
    const index = comments.findIndex((c) => c.id === id);
    if (index < 0) throw new HarnessError('comment-not-found', `Kommentar ${id} gibt es nicht.`);
    return index;
  }

  // Schreibt die Sidecar-Datei atomar
  private async save(docPath: string, comments: DocComment[]): Promise<void> {
    const file = join(this.root, DocCommentStore.sidecarOf(docPath));
    await this.writer.write(file, JSON.stringify({ comments }, null, 2) + '\n');
  }
}
