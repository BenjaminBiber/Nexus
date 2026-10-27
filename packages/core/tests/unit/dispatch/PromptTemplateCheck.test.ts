import { describe, it, expect } from 'vitest';
import { PromptTemplateCheck, type PromptKind } from '../../../src/dispatch/PromptTemplateCheck.js';

const check = new PromptTemplateCheck();

// Vollständige Beispielvorlagen – enthalten alle Pflichtteile der jeweiligen Laufart.
const SAMPLES: Record<PromptKind, string> = {
  work: [
    'Du bist {{agent}}. Setze Ticket {{ticketId}} um.',
    'erledigt: harness board done {{ticketId}} --acceptance "<woran man es sieht>" --by {{agent}};',
    'blockiert: harness board set {{ticketId}} --status blocked --blocked-reason "<Grund>" --by {{agent}};',
    'Frage: harness board question add "<Frage>" --description "<Kontext>" --ticket {{ticketId}} --author {{agent}}',
  ].join('\n'),
  plan: [
    'Du bist {{agent}}, ein Planungs-Agent. Plane Ticket {{ticketId}}.',
    'Plan als JSON in die Datei plan.json schreiben, dann speichern:',
    'harness board plan-json {{ticketId}} --file plan.json --by {{agent}}',
  ].join('\n'),
  research: [
    'Du bist {{agent}}, ein Recherche-Agent. Recherchiere Frage {{questionId}}.',
    'Ergebnis in die Datei research.json schreiben, dann speichern:',
    'harness board question research {{questionId}} --from research.json --author {{agent}}',
  ].join('\n'),
  merge: [
    'Du bist {{agent}}. Der Branch {{branch}} von Ticket {{ticketId}} laesst sich nicht nach {{target}} mergen.',
    'Gelingt es nicht: git merge --abort und den Grund als Kommentar.',
  ].join('\n'),
};

describe('PromptTemplateCheck', () => {
  it('kennt genau die vier Laufarten', () => {
    expect(PromptTemplateCheck.KINDS).toEqual(['work', 'plan', 'research', 'merge']);
  });

  for (const kind of PromptTemplateCheck.KINDS) {
    it(`meldet fuer eine vollstaendige ${kind}-Vorlage nichts`, () => {
      expect(check.missing(kind, SAMPLES[kind])).toEqual([]);
    });

    it(`liefert fuer ${kind} eindeutige ids und nicht-leere Labels`, () => {
      const parts = check.parts(kind);
      expect(parts.length).toBeGreaterThan(0);
      expect(new Set(parts.map((p) => p.id)).size).toBe(parts.length);
      for (const part of parts) expect(part.label.trim()).not.toBe('');
    });

    it(`meldet fuer eine leere ${kind}-Vorlage alle Pflichtteile`, () => {
      expect(check.missing(kind, '').map((p) => p.id)).toEqual(check.parts(kind).map((p) => p.id));
    });
  }

  // Jeder Pflichtteil muss einzeln erkennbar sein: ohne ihn fehlt mindestens er selbst.
  for (const kind of PromptTemplateCheck.KINDS) {
    for (const part of new PromptTemplateCheck().parts(kind)) {
      it(`erkennt den fehlenden Pflichtteil ${kind}/${part.id}`, () => {
        const without = removePart(SAMPLES[kind], part.id);
        expect(check.missing(kind, without).map((p) => p.id)).toContain(part.id);
      });
    }
  }

  it('akzeptiert zusaetzlichen Leerraum und weitere Flags zwischen Befehl und Platzhalter', () => {
    const tolerant = SAMPLES.work.replace('harness board done {{ticketId}}', 'harness   board\tdone   {{ticketId}}');
    expect(check.missing('work', tolerant)).toEqual([]);
    const flags = SAMPLES.plan.replace('plan-json {{ticketId}}', 'plan-json --quiet {{ticketId}}');
    expect(check.missing('plan', flags)).toEqual([]);
  });
});

// Entfernt den vom Pflichtteil geprüften Text aus der Beispielvorlage (grob: die Zeilen, die ihn tragen).
function removePart(template: string, id: string): string {
  const marker: Record<string, RegExp> = {
    'ticketId': /\{\{ticketId\}\}/g,
    'agent': /\{\{agent\}\}/g,
    'questionId': /\{\{questionId\}\}/g,
    'branch': /\{\{branch\}\}/g,
    'target': /\{\{target\}\}/g,
    'done': /harness board done \{\{ticketId\}\}/g,
    'blocked': /harness board set \{\{ticketId\}\} --status blocked/g,
    'question': /harness board question add[\s\S]*?--ticket \{\{ticketId\}\}/g,
    'by-agent': /--by \{\{agent\}\}/g,
    'plan-json': /harness board plan-json \{\{ticketId\}\}/g,
    'plan-file': /plan\.json/g,
    'question-research': /harness board question research \{\{questionId\}\}/g,
    'research-file': /research\.json/g,
    'merge-abort': /git merge --abort/g,
  };
  const re = marker[id];
  if (!re) throw new Error(`Testlücke: kein Entferner für Pflichtteil ${id}`);
  return template.replace(re, 'X');
}
