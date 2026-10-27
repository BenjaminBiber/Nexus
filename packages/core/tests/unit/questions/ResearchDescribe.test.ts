import { describe, expect, it } from 'vitest';
import { DecisionPolicy } from '../../../src/questions/DecisionPolicy.js';
import { ResearchValidator } from '../../../src/questions/ResearchValidator.js';

const option = (id: string, recommended = false) => ({ id, title: id === 'O1' ? 'Datei' : id, text: 't', pros: [], cons: [], recommended });

describe('ResearchValidator.describe', () => {
  const validator = new ResearchValidator();

  it('beschreibt Optionen mit Bereich und Empfehlung', () => {
    expect(validator.describe({ options: [option('O1'), option('O2', true), option('O3')] })).toBe('3 Optionen (O1 bis O3), O2 empfohlen');
    expect(validator.describe({ options: [option('O4')] })).toBe('1 Option (O4)');
    expect(validator.describe({ options: [] })).toBe('Kontext ergänzt');
  });
});

describe('DecisionPolicy mit schlanker Frage', () => {
  it('braucht nur Nummer, Zustand und Recherchen', () => {
    const policy = new DecisionPolicy();
    const subject = { id: 'ARC-F0001', state: 'open' as const, research: [{ at: 'x', author: 'claude', kind: 'session' as const, context: '', options: [option('O1')] }] };
    const decision = policy.answer(subject, { option: 'O1', reason: 'passt' }, { author: 'Florian', kind: 'human' }, 'jetzt');
    expect(policy.describe(subject, decision)).toBe('Option O1 „Datei“ gewählt. Begründung: passt');
  });
});
