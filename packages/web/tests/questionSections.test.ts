import { describe, expect, it } from 'vitest';
import { findSection, followUpDraft, parseFindings, splitEffort, splitSections } from '../src/dbquestion/questionSections.js';

describe('splitSections', () => {
  it('zerlegt fett gesetzte Überschriften der Schreibregeln in Abschnitte', () => {
    const parsed = splitSections('**Worum es geht**\nZwei Sätze.\n\n**Heute**\n- eins\n- zwei\n\n**Zu entscheiden**\nWelches Zielbild?');
    expect(parsed.intro).toBe('');
    expect(parsed.sections).toEqual([
      { title: 'Worum es geht', body: 'Zwei Sätze.' },
      { title: 'Heute', body: '- eins\n- zwei' },
      { title: 'Zu entscheiden', body: 'Welches Zielbild?' },
    ]);
  });

  it('kennt auch #-Überschriften und „**Titel:** Text“ auf einer Zeile, nicht aber fetten Text im Satz', () => {
    const parsed = splitSections('Vorweg.\n## Heute\nso\n**Empfehlung:** Karten, weil gleich\n**Wichtig** ist das hier');
    expect(parsed.intro).toBe('Vorweg.');
    expect(parsed.sections).toEqual([
      { title: 'Heute', body: 'so' },
      { title: 'Empfehlung', body: 'Karten, weil gleich\n**Wichtig** ist das hier' },
    ]);
  });

  it('ignoriert Überschriften in Codeblöcken und lässt Text ohne Gliederung als Einleitung', () => {
    expect(splitSections('```\n**Heute**\n```').sections).toEqual([]);
    expect(splitSections('Heute: Datei')).toEqual({ intro: 'Heute: Datei', sections: [] });
  });

  it('findet Abschnitte unabhängig von Groß- und Kleinschreibung und Zusätzen, leere zählen nicht', () => {
    const parsed = splitSections('**heute (Stand Oktober)**\nx\n**Zu entscheiden**\n');
    expect(findSection(parsed, 'Heute')?.body).toBe('x');
    expect(findSection(parsed, 'Zu entscheiden')).toBeUndefined();
  });
});

describe('splitEffort', () => {
  it('trennt den Aufwand vom Text der Option', () => {
    expect(splitEffort('Jeder Schritt wird eine Karte. Aufwand: standard')).toEqual({ text: 'Jeder Schritt wird eine Karte.', effort: 'standard' });
    expect(splitEffort('Kurz (Aufwand: Klein).')).toEqual({ text: 'Kurz', effort: 'klein' });
    expect(splitEffort('Ohne Angabe')).toEqual({ text: 'Ohne Angabe', effort: null });
  });
});

describe('parseFindings', () => {
  it('liest „Pfad:Zeile – was dort passiert“ mit und ohne Aufzählung und Backticks', () => {
    expect(parseFindings('- `packages/web/src/a.tsx:101` – Reiter mit Liste\npackages/web/src/b.ts:21 - Stufen\n`c.css:5`: Stil\nd/e.ts')).toEqual([
      { path: 'packages/web/src/a.tsx:101', note: 'Reiter mit Liste' },
      { path: 'packages/web/src/b.ts:21', note: 'Stufen' },
      { path: 'c.css:5', note: 'Stil' },
      { path: 'd/e.ts', note: '' },
    ]);
  });

  it('gibt null für Text, der keine Fundstellen-Liste ist', () => {
    expect(parseFindings('Keine weiteren Fundstellen.')).toBeNull();
    expect(parseFindings('')).toBeNull();
  });
});

describe('followUpDraft', () => {
  it('nimmt die Frage als Titel und den ganzen Satz als Beschreibung', () => {
    expect(followUpDraft('- Soll ein Schritt „in Arbeit“ kennen? (betrifft Datenmodell)')).toEqual({
      title: 'Soll ein Schritt „in Arbeit“ kennen?',
      description: 'Soll ein Schritt „in Arbeit“ kennen? (betrifft Datenmodell)',
    });
    expect(followUpDraft('Welche Farbe?')).toEqual({ title: 'Welche Farbe?', description: '' });
  });
});
