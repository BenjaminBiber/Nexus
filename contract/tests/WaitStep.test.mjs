import { describe, expect, it } from 'vitest';
import { WaitStep } from '../lib/WaitStep.mjs';

// Erwartet eine Meldung beim Prüfen eines fehlerhaften Schritts
const failsWith = (value, pattern) => {
  const fail = (message) => {
    throw new Error(message);
  };
  expect(() => new WaitStep().parse(value, 0, fail)).toThrow(pattern);
};

describe('WaitStep', () => {
  it('nennt seine eigenen Felder', () => {
    expect(new WaitStep().keys).toEqual(['ms']);
  });

  it('übernimmt eine ganze Zahl von Millisekunden', () => {
    expect(new WaitStep().parse({ ms: 5 }, 0, () => {})).toEqual({ ms: 5 });
  });

  it('meldet fehlende Millisekunden', () => {
    failsWith({}, /steps\[0\]\.ms muss eine ganze Zahl von 1 bis 1000 sein/);
  });

  it('meldet Millisekunden als Text', () => {
    failsWith({ ms: '5' }, /steps\[0\]\.ms muss eine ganze Zahl von 1 bis 1000 sein/);
  });

  it('meldet Bruchteile', () => {
    failsWith({ ms: 1.5 }, /steps\[0\]\.ms muss eine ganze Zahl von 1 bis 1000 sein/);
  });

  it('meldet null und negative Werte', () => {
    failsWith({ ms: 0 }, /steps\[0\]\.ms muss eine ganze Zahl von 1 bis 1000 sein/);
    failsWith({ ms: -3 }, /steps\[0\]\.ms muss eine ganze Zahl von 1 bis 1000 sein/);
  });

  it('meldet Wartezeiten über einer Sekunde', () => {
    failsWith({ ms: 1001 }, /steps\[0\]\.ms muss eine ganze Zahl von 1 bis 1000 sein/);
  });

  it('wartet mindestens die genannte Zeit', async () => {
    const start = performance.now();
    await new WaitStep().run({ ms: 20 }, {});
    // setTimeout darf um Bruchteile einer Millisekunde zu früh feuern
    expect(performance.now() - start).toBeGreaterThanOrEqual(19);
  });

  it('zeichnet die Wartezeit als Anfrage und eine leere Antwort auf', async () => {
    expect(await new WaitStep().run({ ms: 1 }, {})).toEqual({ request: { kind: 'wait', ms: 1 }, response: {} });
  });
});
