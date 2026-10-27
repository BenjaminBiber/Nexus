import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { mkdirSync, mkdtempSync, rmSync, statSync, utimesSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { TempRepo } from '../helpers/TempRepo.js';
import { SuperpowersDocScanner } from '../../../src/docs/SuperpowersDocScanner.js';

describe('SuperpowersDocScanner', () => {
  let repo: TempRepo;
  let scanner: SuperpowersDocScanner;

  // Legt eine Datei unter docs/superpowers/<ordner>/ an
  function write(kind: 'specs' | 'plans', name: string, content = ''): string {
    const dir = join(repo.path, 'docs', 'superpowers', kind);
    mkdirSync(dir, { recursive: true });
    const path = join(dir, name);
    writeFileSync(path, content, 'utf8');
    return path;
  }

  beforeEach(() => {
    repo = new TempRepo();
    scanner = new SuperpowersDocScanner();
  });
  afterEach(() => repo.dispose());

  it('liest Datum und Thema aus dem Dateinamen', () => {
    write('specs', '2026-01-02-login-design.md');
    write('plans', '2026-01-02-login.md');
    const docs = scanner.scan(repo.path);
    expect(docs.map((d) => [d.kind, d.date, d.topic, d.slug])).toEqual([
      ['plan', '2026-01-02', 'login', '2026-01-02-login'],
      ['spec', '2026-01-02', 'login', '2026-01-02-login-design'],
    ]);
    expect(docs[0]?.path).toBe('docs/superpowers/plans/2026-01-02-login.md');
    expect(docs[0]?.size).toBe(0);
    expect(docs[0]?.mtimeMs).toBeGreaterThan(0);
  });

  it('nimmt den Titel aus der ersten H1, sonst den Dateinamen', () => {
    write('specs', '2026-01-02-login-design.md', 'Vorspann\n\n# Anmeldung neu\n\n# Zweite\n');
    write('plans', '2026-01-02-login.md', 'ohne Überschrift\n');
    const byTopic = new Map(scanner.scan(repo.path).map((d) => [d.kind, d.title]));
    expect(byTopic.get('spec')).toBe('Anmeldung neu');
    expect(byTopic.get('plan')).toBe('2026-01-02-login');
  });

  it('paart Spec und Plan über das Thema und lässt Fehlendes null', () => {
    write('specs', '2026-01-02-login-design.md');
    write('plans', '2026-01-02-login.md');
    write('specs', '2026-01-03-suche-design.md');
    const topics = scanner.topics(repo.path);
    expect(topics.map((t) => t.topic)).toEqual(['suche', 'login']);
    expect(topics[0]?.plan).toBeNull();
    expect(topics[0]?.spec?.slug).toBe('2026-01-03-suche-design');
    expect(topics[1]?.spec?.kind).toBe('spec');
    expect(topics[1]?.plan?.kind).toBe('plan');
    expect(topics[1]?.date).toBe('2026-01-02');
  });

  it('nimmt Dateien ohne Datum mit date null auf und sortiert sie nach hinten', () => {
    write('specs', 'notizen.md');
    write('specs', '2026-01-02-login-design.md');
    const docs = scanner.scan(repo.path);
    expect(docs.map((d) => d.topic)).toEqual(['login', 'notizen']);
    expect(docs[1]?.date).toBeNull();
    expect(scanner.topics(repo.path).map((t) => t.topic)).toEqual(['login', 'notizen']);
  });

  it('kommt ohne die Ordner aus', () => {
    expect(scanner.scan(repo.path)).toEqual([]);
    expect(scanner.topics(repo.path)).toEqual([]);
    expect(() => scanner.signature(repo.path)).not.toThrow();
  });

  it('ändert die Signatur bei geänderter und bei neuer Datei', () => {
    const path = write('specs', '2026-01-02-login-design.md', 'a');
    const first = scanner.signature(repo.path);
    expect(scanner.signature(repo.path)).toBe(first);

    writeFileSync(path, 'anderer Inhalt', 'utf8');
    const später = new Date(statSync(path).mtimeMs + 2000);
    utimesSync(path, später, später);
    const second = scanner.signature(repo.path);
    expect(second).not.toBe(first);

    write('plans', '2026-01-02-login.md');
    expect(scanner.signature(repo.path)).not.toBe(second);
  });

  it('übergeht Sidecars und andere Endungen', () => {
    write('specs', '2026-01-02-login-design.md');
    write('specs', '2026-01-02-login-design.comments.json');
    expect(scanner.scan(repo.path).map((d) => d.slug)).toEqual(['2026-01-02-login-design']);
  });

  it('listet nur Dateinamen, die sich über die Docs-Seite auch lesen lassen', () => {
    write('specs', '2026-01-02-login-design.md');
    write('specs', '2026-01-03 mit Leerzeichen.md');
    write('specs', '2026-01-04-groß.md');
    write('plans', '2026-01-05-laut.MD');
    expect(scanner.scan(repo.path).map((d) => d.slug)).toEqual(['2026-01-02-login-design']);
  });

  it('liest im angegebenen Ordner, auch unterhalb der Repo-Wurzel und ohne Git', () => {
    write('specs', '2026-01-02-oben-design.md');
    const sub = join(repo.path, 'app');
    mkdirSync(join(sub, 'docs', 'superpowers', 'plans'), { recursive: true });
    writeFileSync(join(sub, 'docs', 'superpowers', 'plans', '2026-01-03-unten.md'), '', 'utf8');
    expect(scanner.scan(sub).map((d) => d.path)).toEqual(['docs/superpowers/plans/2026-01-03-unten.md']);
    const plain = mkdtempSync(join(tmpdir(), 'harness-docs-'));
    try {
      expect(scanner.scan(plain)).toEqual([]);
      expect(scanner.signature(plain)).toMatch(/^[0-9a-f]{16}$/);
    } finally {
      rmSync(plain, { recursive: true, force: true });
    }
  });
});
