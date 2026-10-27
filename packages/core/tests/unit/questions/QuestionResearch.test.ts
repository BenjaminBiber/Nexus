import { describe, it, expect } from 'vitest';
import { ResearchValidator } from '../../../src/questions/ResearchValidator.js';

const actor = { author: 'Claude', kind: 'session' as const };
const option = (title: string, extra: Record<string, unknown> = {}) => ({ title, text: `${title} Text`, pros: ['a'], cons: ['b'], ...extra });

describe('ResearchValidator', () => {
  const validator = new ResearchValidator();

  it('vergibt Optionsnummern ab O1 und übernimmt Vor- und Nachteile', () => {
    const research = validator.build({ context: 'Kontext', options: [option('A'), option('B', { recommended: true })] }, [], actor, '2026-10-10T10:00:00.000Z');
    expect(research.options.map((o) => o.id)).toEqual(['O1', 'O2']);
    expect(research.options[1]).toMatchObject({ title: 'B', pros: ['a'], cons: ['b'], recommended: true });
    expect(research).toMatchObject({ at: '2026-10-10T10:00:00.000Z', author: 'Claude', kind: 'session', context: 'Kontext' });
  });

  it('nummeriert über Recherchen hinweg fortlaufend weiter', () => {
    const first = validator.build({ options: [option('A'), option('B')] }, [], actor, 't1');
    const second = validator.build({ options: [option('C')] }, [first], actor, 't2');
    expect(second.options.map((o) => o.id)).toEqual(['O3']);
  });

  it('lässt Vor- und Nachteile weg, wenn sie fehlen', () => {
    const research = validator.build({ options: [{ title: 'A', text: 'x' }] }, [], actor, 't');
    expect(research.options[0]).toMatchObject({ pros: [], cons: [], recommended: false });
  });

  it('lehnt mehr als eine empfohlene Option ab', () => {
    expect(() => validator.build({ options: [option('A', { recommended: true }), option('B', { recommended: true })] }, [], actor, 't'))
      .toThrowError(expect.objectContaining({ code: 'research-recommended-multiple' }));
  });

  it('lehnt mehr als fünf Optionen ab', () => {
    const six = Array.from({ length: 6 }, (_, i) => option(`O${i}`));
    expect(() => validator.build({ options: six }, [], actor, 't')).toThrowError(expect.objectContaining({ code: 'research-invalid' }));
  });

  it('lehnt eine Recherche ohne Kontext und ohne Optionen ab', () => {
    expect(() => validator.build({}, [], actor, 't')).toThrowError(expect.objectContaining({ code: 'research-invalid' }));
    expect(() => validator.build({ context: '   ', options: [] }, [], actor, 't')).toThrowError(expect.objectContaining({ code: 'research-invalid' }));
  });

  it('lehnt Optionen ohne Titel oder Text ab', () => {
    expect(() => validator.build({ options: [{ title: '', text: 'x' }] }, [], actor, 't')).toThrowError(expect.objectContaining({ code: 'research-invalid' }));
    expect(() => validator.build({ options: [{ title: 'A', text: ' ' }] }, [], actor, 't')).toThrowError(expect.objectContaining({ code: 'research-invalid' }));
  });

  it('lässt Recherche nur mit Kontext zu', () => {
    const research = validator.build({ context: 'Nur Kontext' }, [], actor, 't');
    expect(research.options).toEqual([]);
  });
});
