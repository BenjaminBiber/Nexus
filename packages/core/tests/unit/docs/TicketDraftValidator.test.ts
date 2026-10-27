import { describe, expect, it } from 'vitest';
import { HarnessError, TicketDraftValidator } from '../../../src/index.js';

const DOC = { lines: 100 };

// Ein vollständiger Entwurf mit überschreibbaren Feldern
const draft = (title: string, extra: Record<string, unknown> = {}) => ({
  title, description: `Beschreibung ${title}`, acceptance: `Abnahme ${title}`,
  steps: ['Schritt'], dependsOn: [], labels: ['server'],
  source: { heading: 'Ziel', startLine: 3, endLine: 7 },
  ...extra,
});

describe('TicketDraftValidator', () => {
  const validator = new TicketDraftValidator();

  it('übernimmt eine gültige Liste unverändert', () => {
    const result = validator.check([draft('A'), draft('B', { dependsOn: [0] })], DOC);
    expect(result.dropped).toEqual([]);
    expect(result.drafts.map((d) => d.title)).toEqual(['A', 'B']);
    expect(result.drafts[1].dependsOn).toEqual([0]);
    expect(result.drafts[0].source).toEqual({ heading: 'Ziel', startLine: 3, endLine: 7 });
  });

  it('verwirft Entwürfe ohne Pflichtfelder und rechnet dependsOn auf die neuen Positionen um', () => {
    const result = validator.check([
      draft('A'),
      draft('B', { title: '  ' }),
      draft('C', { acceptance: '' }),
      draft('D', { dependsOn: [0, 3] }),
    ], DOC);
    expect(result.drafts.map((d) => d.title)).toEqual(['A', 'D']);
    // 0 bleibt A, der Verweis auf den verworfenen Eintrag 3 (D selbst) fällt weg
    expect(result.drafts[1].dependsOn).toEqual([0]);
    expect(result.dropped.length).toBeGreaterThanOrEqual(2);
  });

  it('kappt die Liste bei 15 Entwürfen', () => {
    const many = Array.from({ length: 20 }, (_, i) => draft(`T${i}`));
    const result = validator.check(many, DOC);
    expect(result.drafts).toHaveLength(15);
    expect(result.dropped.join(' ')).toContain('15');
  });

  it('entfernt Selbstbezug, Indizes außerhalb und Dubletten', () => {
    const result = validator.check([draft('A', { dependsOn: [0, 5, -1, 1, 1] }), draft('B')], DOC);
    expect(result.drafts[0].dependsOn).toEqual([1]);
    expect(result.dropped.length).toBeGreaterThan(0);
  });

  it('bereinigt Kreise und behält alle Entwürfe', () => {
    const result = validator.check([
      draft('A', { dependsOn: [2] }),
      draft('B', { dependsOn: [0] }),
      draft('C', { dependsOn: [1] }),
    ], DOC);
    expect(result.drafts.map((d) => d.title)).toEqual(['A', 'B', 'C']);
    const edges = result.drafts.flatMap((d, to) => d.dependsOn.map((from) => [from, to]));
    expect(edges.length).toBe(2);
    expect(result.dropped.join(' ')).toContain('Kreis');
  });

  it('begrenzt source auf das Dokument und ersetzt unbrauchbare Angaben', () => {
    const result = validator.check([
      draft('A', { source: { heading: 42, startLine: 0, endLine: 9000 } }),
      draft('B', { source: { heading: 'Plan', startLine: 90, endLine: 9000 } }),
    ], DOC);
    expect(result.drafts[0].source).toEqual({ heading: '', startLine: 1, endLine: 1 });
    expect(result.drafts[1].source).toEqual({ heading: 'Plan', startLine: 90, endLine: 100 });
  });

  it('räumt steps und labels auf', () => {
    const result = validator.check([draft('A', { steps: ['x', '', 7], labels: 'server' })], DOC);
    expect(result.drafts[0].steps).toEqual(['x']);
    expect(result.drafts[0].labels).toEqual([]);
  });

  it('meldet analysis-failed, wenn die Antwort keine Liste ist oder nichts übrig bleibt', () => {
    expect(() => validator.check({ drafts: [] }, DOC)).toThrow(HarnessError);
    expect(() => validator.check([], DOC)).toThrow(/analysis-failed|Entw/);
    try {
      validator.check([draft('A', { description: '' })], DOC);
      throw new Error('kein Fehler');
    } catch (error) {
      expect((error as HarnessError).code).toBe('analysis-failed');
    }
  });
});
