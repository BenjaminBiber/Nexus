import { describe, expect, it } from 'vitest';
import type { RawDispatchConfigDto } from '../src/api/dispatchTypes.js';
import { PROMPT_KINDS, toForm, toRaw } from '../src/components/dispatch/dispatchForm.js';

describe('dispatchForm: Prompt-Vorlagen', () => {
  it('übernimmt Vorlagen aus der Einstellung ins Formular, fehlende als leeren Text', () => {
    const form = toForm({ prompts: { work: 'Hallo {{ticketId}}', plan: null } });
    expect(form.prompts.work).toBe('Hallo {{ticketId}}');
    expect(form.prompts.plan).toBe('');
    expect(form.prompts.research).toBe('');
    expect(form.prompts.merge).toBe('');
  });

  it('speichert ein leeres Feld als null (= Standardvorlage)', () => {
    const base: RawDispatchConfigDto = { prompts: { work: 'alt' } };
    const form = { ...toForm(base), prompts: { work: '', plan: '', research: '', merge: '' } };
    expect(toRaw(form, base).prompts).toEqual({ work: null, plan: null, research: null, merge: null });
  });

  it('lässt Einrückungen und Leerzeilen der Vorlage unverändert', () => {
    const template = '  Zeile eins\n\n    Zeile zwei  \n';
    const form = { ...toForm({}), prompts: { work: template, plan: '', research: '', merge: '' } };
    expect(toRaw(form, {}).prompts?.work).toBe(template);
  });

  it('legt ohne Vorlagen und ohne Abschnitt in der Datei kein Feld prompts an', () => {
    expect('prompts' in toRaw(toForm({}), {})).toBe(false);
  });

  it('kennt die vier Laufarten in der Reihenfolge der Oberfläche', () => {
    expect([...PROMPT_KINDS]).toEqual(['work', 'plan', 'research', 'merge']);
  });
});
