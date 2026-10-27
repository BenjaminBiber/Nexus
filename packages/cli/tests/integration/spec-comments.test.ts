import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { TempProject } from './helpers/TempProject.js';

const SPEC = 'docs/superpowers/specs/2026-10-22-login-design.md';
const PLAN = 'docs/superpowers/plans/2026-10-22-login.md';

// Sidecar mit einem offenen und einem erledigten Kommentar neben dem Dokument
function writeSidecar(root: string, docPath: string, id: string): void {
  const file = join(root, docPath.replace(/\.md$/, '.comments.json'));
  mkdirSync(join(root, docPath.slice(0, docPath.lastIndexOf('/'))), { recursive: true });
  writeFileSync(file, JSON.stringify({ comments: [
    { id, status: 'open', text: 'Bitte präzisieren', createdAt: '2026-10-10T10:00:00Z', quote: 'wird angelegt', heading: 'Ziel', lines: [5, 5], resolution: null },
    { id: 'K-99999999', status: 'resolved', text: 'Alt', createdAt: '2026-10-09T10:00:00Z', quote: 'x', heading: null, lines: null, resolution: 'ok' },
  ] }));
}

describe('harness spec comments und spec resolve', () => {
  let p: TempProject;
  beforeEach(() => {
    p = new TempProject();
    p.run(['init']);
    writeSidecar(p.path, SPEC, 'K-1a2b3c4d');
    writeSidecar(p.path, PLAN, 'K-5e6f7a8b');
  });
  afterEach(() => p.dispose());

  it('listet nur offene Kommentare mit Stelle und Text', () => {
    const result = p.run(['spec', 'comments']);
    expect(result.code).toBe(0);
    expect(result.stdout).toContain('K-1a2b3c4d');
    expect(result.stdout).toContain(SPEC);
    expect(result.stdout).toContain('Bitte präzisieren');
    expect(result.stdout).not.toContain('K-99999999');
  });

  it('liefert mit --json eine Liste aus path und comment', () => {
    const result = p.run(['spec', 'comments', '--json']);
    expect(result.code).toBe(0);
    const list = JSON.parse(result.stdout) as Array<{ path: string; comment: { id: string } }>;
    expect(list.map((e) => e.path).sort()).toEqual([PLAN, SPEC]);
    expect(list.find((e) => e.path === SPEC)?.comment.id).toBe('K-1a2b3c4d');
  });

  it('begrenzt auf ein Thema (Spec und Plan) oder genau ein Dokument', () => {
    const topic = p.run(['spec', 'comments', 'login']);
    expect(topic.stdout).toContain('K-1a2b3c4d');
    expect(topic.stdout).toContain('K-5e6f7a8b');
    const doc = p.run(['spec', 'comments', SPEC]);
    expect(doc.stdout).toContain('K-1a2b3c4d');
    expect(doc.stdout).not.toContain('K-5e6f7a8b');
  });

  it('meldet zu einem unbekannten Thema keine offenen Kommentare', () => {
    const result = p.run(['spec', 'comments', 'gibt-es-nicht']);
    expect(result.code).toBe(0);
    expect(result.stdout).toContain('Keine offenen Kommentare.');
  });

  it('findet ein Dokument, das nur im Hauptordner liegt, auch aus einem Worktree', () => {
    // Das Dokument ist nicht committet und fehlt deshalb im Worktree
    const worktree = join(p.base, 'wt');
    p.git('worktree', 'add', '-q', '-b', 'wt-test', worktree);
    const result = p.run(['spec', 'comments', 'login'], worktree);
    expect(result.code).toBe(0);
    expect(result.stdout).toContain('K-1a2b3c4d');
  });

  it('erledigt einen Kommentar mit Antwort', () => {
    expect(p.run(['spec', 'resolve', 'K-1a2b3c4d', 'Ziel präzisiert']).code).toBe(0);
    expect(p.run(['spec', 'comments', SPEC]).stdout).toContain('Keine offenen Kommentare.');
  });

  it('meldet einen unbekannten Kommentar mit Rückgabecode 1', () => {
    expect(p.run(['spec', 'resolve', 'K-00000000', 'x']).code).toBe(1);
  });
});
