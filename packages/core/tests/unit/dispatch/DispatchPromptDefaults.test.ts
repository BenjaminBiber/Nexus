import { describe, expect, it } from 'vitest';
import { DISPATCH_PROMPT_DEFAULTS, DISPATCH_PROMPT_PLACEHOLDERS } from '../../../src/dispatch/DispatchPromptDefaults.js';
import { PromptTemplateCheck } from '../../../src/dispatch/PromptTemplateCheck.js';

const check = new PromptTemplateCheck();

describe('DispatchPromptDefaults', () => {
  for (const kind of PromptTemplateCheck.KINDS) {
    const template = DISPATCH_PROMPT_DEFAULTS[kind];
    const listed = DISPATCH_PROMPT_PLACEHOLDERS[kind];

    it(`${kind}: die Standardvorlage enthält alle Pflichtteile`, () => {
      expect(check.missing(kind, template)).toEqual([]);
    });

    it(`${kind}: die Platzhalterliste nennt genau die Platzhalter der Vorlage`, () => {
      const used = new Set(Array.from(template.matchAll(/\{\{(\w+)\}\}/g), (m) => m[1]));
      expect([...used].sort()).toEqual(listed.map((p) => p.name).sort());
    });

    it(`${kind}: jeder Platzhalter ist beschrieben`, () => {
      for (const placeholder of listed) expect(placeholder.description.trim()).not.toBe('');
    });
  }
});
