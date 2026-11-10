import { describe, expect, it } from 'vitest';
import { isAutopilot } from '../src/dbboard/autopilot.js';

describe('isAutopilot', () => {
  it('ist aus, solange der Schalter aus ist oder fehlt', () => {
    expect(isAutopilot(undefined, [])).toBe(false);
    expect(isAutopilot({ enabled: false, exceptLabel: null }, [])).toBe(false);
  });

  it('gilt bei eingeschaltetem Autopilot für Tickets ohne Ausnahme-Label', () => {
    expect(isAutopilot({ enabled: true, exceptLabel: 'handarbeit' }, ['ui', 'api'])).toBe(true);
  });

  it('vergleicht das Ausnahme-Label getrimmt und ohne Groß-/Kleinschreibung', () => {
    expect(isAutopilot({ enabled: true, exceptLabel: ' Handarbeit ' }, ['ui', 'HANDARBEIT'])).toBe(false);
  });

  it('nimmt ohne Angabe die Vorgabe „interaktiv“, null oder leer heißt keine Ausnahme', () => {
    expect(isAutopilot({ enabled: true }, ['interaktiv'])).toBe(false);
    expect(isAutopilot({ enabled: true, exceptLabel: null }, ['interaktiv'])).toBe(true);
    expect(isAutopilot({ enabled: true, exceptLabel: '  ' }, ['interaktiv'])).toBe(true);
  });
});
