import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { DocCommentStore } from '../../../src/docs/DocCommentStore.js';

const NEW = { text: 'Bitte präzisieren', quote: 'Das Ticket', heading: 'Ziel', lines: [3, 3] as [number, number] };
const SPEC = 'docs/superpowers/specs/2026-10-22-login-design.md';
const PLAN = 'docs/superpowers/plans/2026-10-22-login.md';

describe('DocCommentStore', () => {
  let root: string;
  let store: DocCommentStore;
  beforeEach(() => {
    root = mkdtempSync(join(tmpdir(), 'harness-comments-'));
    mkdirSync(join(root, 'docs', 'superpowers', 'specs'), { recursive: true });
    mkdirSync(join(root, 'docs', 'superpowers', 'plans'), { recursive: true });
    mkdirSync(join(root, 'docs', 'adr'), { recursive: true });
    store = new DocCommentStore(root);
  });
  afterEach(() => rmSync(root, { recursive: true, force: true }));

  it('leitet den Sidecar-Namen ab', () => {
    expect(DocCommentStore.sidecarOf(SPEC)).toBe('docs/superpowers/specs/2026-10-22-login-design.comments.json');
    expect(DocCommentStore.sidecarOf('GLOSSARY.md')).toBe('GLOSSARY.comments.json');
  });

  it('liefert ohne Datei eine leere Liste und legt beim Hinzufügen die Datei an', async () => {
    expect(await store.list(SPEC)).toEqual([]);
    const added = await store.add(SPEC, NEW);
    expect(added).toMatchObject({ ...NEW, status: 'open', resolution: null });
    expect(added.id).toMatch(/^K-[0-9a-f]{8}$/);
    const file = JSON.parse(readFileSync(join(root, 'docs/superpowers/specs/2026-10-22-login-design.comments.json'), 'utf8'));
    expect(file.comments).toHaveLength(1);
  });

  it('ändert Text, Status und Antwort', async () => {
    const { id } = await store.add('GLOSSARY.md', NEW);
    const changed = await store.update('GLOSSARY.md', id, { status: 'resolved', resolution: 'Ergänzt in Abschnitt Ziel' });
    expect(changed).toMatchObject({ status: 'resolved', resolution: 'Ergänzt in Abschnitt Ziel', text: NEW.text });
  });

  it('entfernt einen Kommentar und meldet unbekannte IDs', async () => {
    const { id } = await store.add('GLOSSARY.md', NEW);
    await store.remove('GLOSSARY.md', id);
    expect(await store.list('GLOSSARY.md')).toEqual([]);
    await expect(store.update('GLOSSARY.md', 'K-00000000', { text: 'x' })).rejects.toMatchObject({ code: 'comment-not-found' });
  });

  it('listet ohne Scope Dokumente, Glossar und ADRs und lässt erledigte weg', async () => {
    await store.add(SPEC, NEW);
    await store.add('GLOSSARY.md', NEW);
    await store.add('docs/adr/0001-eins.md', NEW);
    const done = await store.add(PLAN, NEW);
    await store.update(PLAN, done.id, { status: 'resolved' });
    expect((await store.listOpen()).map((o) => o.path).sort()).toEqual(['GLOSSARY.md', 'docs/adr/0001-eins.md', SPEC]);
  });

  it('nimmt als Scope ein Thema (Spec und Plan) oder genau einen Dokumentpfad', async () => {
    await store.add(SPEC, NEW);
    await store.add(PLAN, NEW);
    await store.add('docs/superpowers/specs/2026-10-20-export-design.md', NEW);
    await store.add('GLOSSARY.md', NEW);
    expect((await store.listOpen('login')).map((o) => o.path).sort()).toEqual([PLAN, SPEC]);
    expect((await store.listOpen(SPEC)).map((o) => o.path)).toEqual([SPEC]);
    expect(await store.listOpen('gibt-es-nicht')).toEqual([]);
  });

  it('erledigt einen Kommentar über die ID, ohne das Dokument zu kennen', async () => {
    const { id } = await store.add(PLAN, NEW);
    const resolved = await store.resolve(id, 'Schritt 3 ergänzt');
    expect(resolved.path).toBe(PLAN);
    expect(resolved.comment).toMatchObject({ status: 'resolved', resolution: 'Schritt 3 ergänzt' });
  });

  it('meldet eine beschädigte Sidecar-Datei verständlich', async () => {
    writeFileSync(join(root, 'GLOSSARY.comments.json'), '{kaputt');
    await expect(store.list('GLOSSARY.md')).rejects.toMatchObject({ code: 'comments-corrupt' });
  });

  it('lehnt Pfade ohne .md oder außerhalb des Projekts ab und legt nichts an', async () => {
    await expect(store.list('README.txt')).rejects.toMatchObject({ code: 'doc-not-found' });
    await expect(store.add('../x.md', NEW)).rejects.toMatchObject({ code: 'doc-not-found' });
    await expect(store.add('/abs.md', NEW)).rejects.toMatchObject({ code: 'doc-not-found' });
    await expect(store.add('docs/superpowers/specs/2026-10-22-login-design.txt', NEW)).rejects.toMatchObject({ code: 'doc-not-found' });
    expect(existsSync(join(root, 'docs/superpowers/specs/2026-10-22-login-design.txt'))).toBe(false);
    expect(existsSync(join(root, 'x.comments.json'))).toBe(false);
  });

  it('lehnt Pfade mit Backslash ab und schreibt nie außerhalb des Projekts', async () => {
    for (const bad of ['docs\\..\\..\\x.md', '..\\x.md', '\\\\server\\share\\x.md']) {
      await expect(store.add(bad, NEW)).rejects.toMatchObject({ code: 'doc-not-found' });
    }
    expect(existsSync(join(dirname(root), 'x.comments.json'))).toBe(false);
    expect(existsSync(join(root, 'x.comments.json'))).toBe(false);
  });

  it('ignoriert fremde comments-Dateien im Projektstamm', async () => {
    await store.add('GLOSSARY.md', NEW);
    await store.add('README.md', NEW);
    expect((await store.listOpen()).map((o) => o.path)).toEqual(['GLOSSARY.md']);
  });

  it('meldet beim Erledigen einer unbekannten ID comment-not-found', async () => {
    await expect(store.resolve('K-00000000', 'x')).rejects.toMatchObject({ code: 'comment-not-found' });
  });

  it('behandelt JSON ohne comments-Array als leer', async () => {
    writeFileSync(join(root, 'GLOSSARY.comments.json'), '{}');
    expect(await store.list('GLOSSARY.md')).toEqual([]);
  });

  it('ignoriert undefined-Werte im Patch', async () => {
    const { id } = await store.add('GLOSSARY.md', NEW);
    const changed = await store.update('GLOSSARY.md', id, { text: undefined, status: 'resolved' });
    expect(changed).toMatchObject({ text: NEW.text, status: 'resolved' });
  });
});
