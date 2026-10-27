// @vitest-environment jsdom
import { describe, it, expect, afterEach } from 'vitest';
import type { DocFileDto } from '../src/api/types.js';
import { ancestorIds, buildDocTree, countLabel, filterDocTree, kindsOf, splitDocTitle } from '../src/board/docsTree.js';
import { loadDocsTreeOpen, saveDocsTreeOpen, loadDocsWidths, saveDocsWidths } from '../src/board/docsTreeOpen.js';

// Dokument der Liste; nicht genannte Felder bekommen harmlose Standardwerte
const doc = (over: Partial<DocFileDto> & { path: string }): DocFileDto => ({
  title: over.path, group: 'spec', kind: null, topic: null, date: null, mtimeMs: 0, exists: true, ...over,
});

const SPEC = doc({ path: 'specs/2026-10-22-a-design.md', title: 'Thema A – Design', group: 'spec', kind: 'spec', topic: 'a', date: '2026-10-22', mtimeMs: 300 });
const PLAN = doc({ path: 'plans/2026-10-22-a-plan.md', title: 'Thema A – Plan', group: 'plan', kind: 'plan', topic: 'a', date: '2026-10-22', mtimeMs: 500 });
const NUR_PLAN = doc({ path: 'plans/2026-09-01-b-plan.md', title: 'Thema B – Plan', group: 'plan', kind: 'plan', topic: 'b', date: '2026-09-01', mtimeMs: 120 });
const GLOSSAR = doc({ path: 'GLOSSARY.md', title: 'Glossar', group: 'glossary', mtimeMs: 50 });
const ADR = doc({ path: 'docs/adr/0001-eins.md', title: '0001-eins', group: 'adr', mtimeMs: 60 });

afterEach(() => window.localStorage.clear());

describe('splitDocTitle', () => {
  it('trennt die bekannten Arten vom Titel', () => {
    expect(splitDocTitle('Nachrichten-Seite: Gruppierung — Design-Spec')).toEqual({ title: 'Nachrichten-Seite: Gruppierung', kind: 'spec' });
    expect(splitDocTitle('Kunden-Detail Increment 2 (Design-Spec)')).toEqual({ title: 'Kunden-Detail Increment 2', kind: 'spec' });
    expect(splitDocTitle('Bell-Popover vergrößern — Mini-Spec')).toEqual({ title: 'Bell-Popover vergrößern', kind: 'mini' });
    expect(splitDocTitle('Emoji-Eingabe – Design')).toEqual({ title: 'Emoji-Eingabe', kind: 'design' });
    expect(splitDocTitle('Design: TFS CI-Pipeline als PR-Gate')).toEqual({ title: 'TFS CI-Pipeline als PR-Gate', kind: 'design' });
    expect(splitDocTitle('Thema B – Plan')).toEqual({ title: 'Thema B', kind: 'plan' });
  });

  it('lässt Titel ohne Art und Bindestriche im Wort unverändert', () => {
    expect(splitDocTitle('Benutzermenü am Avatar')).toEqual({ title: 'Benutzermenü am Avatar', kind: 'other' });
    expect(splitDocTitle('Ticket-Design')).toEqual({ title: 'Ticket-Design', kind: 'other' });
  });
});

describe('buildDocTree', () => {
  it('baut die Ordner aus den Pfaden, Ordner vor Dateien, je alphabetisch', () => {
    const tree = buildDocTree([ADR, GLOSSAR, NUR_PLAN, SPEC, PLAN]);
    // „docs“ enthält nur den Ordner „adr“ und entfällt als Hüllordner
    expect(tree.map((n) => n.label)).toEqual(['adr', 'plans', 'specs', 'GLOSSARY.md']);
    expect(tree.map((n) => n.kind)).toEqual(['group', 'group', 'group', 'file']);
  });

  it('lässt Hüllordner wie docs/superpowers weg und legt gleichnamige Ordner zusammen', () => {
    const tief = doc({ path: 'docs/superpowers/specs/2026-10-01-c-design.md', mtimeMs: 900 });
    const neben = doc({ path: 'docs/superpowers/plans/2026-10-01-c-plan.md', group: 'plan' });
    const tree = buildDocTree([tief, neben, SPEC]);
    expect(tree.map((n) => n.label)).toEqual(['plans', 'specs']);
    expect(tree[1].children.map((c) => c.doc?.path)).toEqual([tief.path, SPEC.path]);
    expect(tree[1].mtimeMs).toBe(900);
    // Ein Ordner mit Dateien bleibt stehen, auch wenn er daneben Ordner enthält
    const gemischt = buildDocTree([doc({ path: 'docs/README.md' }), ADR]);
    expect(gemischt.map((n) => n.label)).toEqual(['docs']);
  });

  it('liefert ohne Dateien keinen Ordner', () => {
    expect(buildDocTree([])).toEqual([]);
  });

  it('beschriftet Dateien mit dem Dateinamen und trägt Titel und Art', () => {
    const [plans, specs] = buildDocTree([SPEC, PLAN, NUR_PLAN]);
    expect(plans.id).toBe('dir:plans');
    expect(plans.children.map((c) => c.label)).toEqual(['2026-09-01-b-plan.md', '2026-10-22-a-plan.md']);
    const spec = specs.children[0];
    expect(spec.kind).toBe('file');
    expect(spec.id).toBe(`file:${SPEC.path}`);
    expect(spec.title).toBe('Thema A – Design');
    expect(spec.tag).toBe('design');
    expect(spec.doc?.path).toBe(SPEC.path);
    expect(plans.children[0].tag).toBe('plan');
  });

  it('sortiert Zahlen in Namen natürlich', () => {
    const tree = buildDocTree([doc({ path: 'n/10.md' }), doc({ path: 'n/9.md' })]);
    expect(tree[0].children.map((c) => c.label)).toEqual(['9.md', '10.md']);
  });

  it('reicht die jüngste Änderung von den Dateien nach oben', () => {
    const [plans, specs] = buildDocTree([SPEC, PLAN, NUR_PLAN]);
    expect(plans.mtimeMs).toBe(500);
    expect(specs.mtimeMs).toBe(300);
  });

  it('stellt mit byBoard jedes Board als obersten Ordner, das Board des Ordners zuerst', () => {
    const fremd = { ...SPEC, board: 3, boardName: 'Fremd' };
    const tree = buildDocTree([fremd, { ...SPEC, boardName: 'Eigen' }], { byBoard: true });
    expect(tree.map((n) => [n.id, n.label])).toEqual([['board:home', 'Eigen'], ['board:3', 'Fremd']]);
    expect(tree[1].children[0].id).toBe('dir:3:specs');
    expect(tree[1].children[0].children[0].id).toBe(`file:3:${SPEC.path}`);
  });
});

describe('filterDocTree', () => {
  const tree = buildDocTree([SPEC, PLAN, NUR_PLAN, GLOSSAR]);

  it('filtert nach Titel oder Dateiname ohne Groß/Klein und lässt leere Ordner weg', () => {
    const shown = filterDocTree(tree, { query: 'THEMA b', kind: 'all' });
    expect(shown.map((n) => n.label)).toEqual(['plans']);
    expect(shown[0].children.map((c) => c.doc?.path)).toEqual([NUR_PLAN.path]);
    expect(countLabel(tree, shown)).toBe('1 von 4');
    expect(filterDocTree(tree, { query: 'glossary', kind: 'all' }).map((n) => n.label)).toEqual(['GLOSSARY.md']);
  });

  it('filtert nach Art; Glossar und ADRs haben keine', () => {
    const shown = filterDocTree(tree, { query: '', kind: 'design' });
    expect(shown.flatMap((g) => g.children.map((c) => c.doc?.path))).toEqual([SPEC.path]);
  });

  it('zählt ungefiltert alle Dateien und nennt die vorhandenen Arten', () => {
    expect(countLabel(tree, filterDocTree(tree, { query: ' ', kind: 'all' }))).toBe('4 Docs');
    expect(kindsOf(tree)).toEqual(['design', 'plan']);
  });
});

describe('ancestorIds', () => {
  it('liefert die Ordner über einer Datei', () => {
    const tree = buildDocTree([SPEC, PLAN, ADR]);
    expect(ancestorIds(tree, PLAN.path)).toEqual(['dir:plans']);
    expect(ancestorIds(tree, ADR.path)).toEqual(['dir:docs/adr']);
    expect(ancestorIds(tree, 'gibt/es/nicht.md')).toEqual([]);
  });
});

describe('docsTreeOpen', () => {
  it('liefert ohne gemerkten Stand null', () => {
    expect(loadDocsTreeOpen('p1')).toBeNull();
  });

  it('merkt die offenen Knoten je Projekt', () => {
    saveDocsTreeOpen('p1', ['group:spec', 'topic:a']);
    expect(loadDocsTreeOpen('p1')).toEqual(['group:spec', 'topic:a']);
    expect(loadDocsTreeOpen('p2')).toBeNull();
  });

  it('wertet einen kaputten Eintrag wie keinen Stand', () => {
    window.localStorage.setItem('harness.docsTreeOpen.p1', '{kaputt');
    expect(loadDocsTreeOpen('p1')).toBeNull();
    window.localStorage.setItem('harness.docsTreeOpen.p1', '{"a":1}');
    expect(loadDocsTreeOpen('p1')).toBeNull();
  });
});

describe('docsWidths', () => {
  it('liefert ohne gemerkten Stand null und merkt sonst beide Breiten', () => {
    expect(loadDocsWidths('p1')).toBeNull();
    saveDocsWidths('p1', { explorer: 320, tools: 280 });
    expect(loadDocsWidths('p1')).toEqual({ explorer: 320, tools: 280 });
  });

  it('wertet unbrauchbare Werte wie keinen Stand', () => {
    window.localStorage.setItem('harness.docsWidths.p1', '{"explorer":"breit","tools":280}');
    expect(loadDocsWidths('p1')).toBeNull();
  });
});
