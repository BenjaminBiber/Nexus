import { describe, expect, it } from 'vitest';
import { SessionTitle } from '../../../src/sessions/SessionTitle.js';

describe('SessionTitle', () => {
  const title = new SessionTitle();

  it('stellt die dreistellige Nummer vor den Titel', () => {
    expect(title.format(1, 'Login bauen')).toBe('SE-001: Login bauen');
  });

  it('nutzt nur die Nummer, wenn es keinen Titel gibt', () => {
    expect(title.format(12)).toBe('SE-012');
  });

  it('nummeriert einen bereits nummerierten Titel nicht doppelt', () => {
    expect(title.format(3, 'SE-001: Login bauen')).toBe('SE-001: Login bauen');
  });

  it('leitet den Titel aus den ersten Wörtern des Prompts ab', () => {
    expect(title.fromPrompt('\n  Baue   bitte einen Login\nmit Passwort')).toBe('Baue bitte einen Login');
    expect(title.fromPrompt('eins zwei drei vier fünf sechs sieben')).toBe('eins zwei drei vier fünf sechs …');
    expect(title.fromPrompt('   ')).toBe('');
  });

  it('überspringt eingeschleuste Blöcke wie system-reminder und browser_instruction', () => {
    const prompt = [
      '<system-reminder>',
      'Codebase und Regeln',
      '</system-reminder>',
      '<browser_instruction># Claude in Chrome',
      'Nutze die Tools.',
      '</browser_instruction>Das Setzen der Session Namen funktioniert nicht',
    ].join('\n');
    expect(title.fromPrompt(prompt)).toBe('Das Setzen der Session Namen funktioniert …');
  });

  it('liefert leer, wenn der Prompt nur aus eingeschleusten Blöcken besteht', () => {
    expect(title.fromPrompt('<system-reminder>\nnur Kontext\n</system-reminder>')).toBe('');
  });
});
