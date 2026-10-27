import { describe, expect, it } from 'vitest';
import { FrontMatter } from '../../../src/customization/FrontMatter.js';
describe('FrontMatter', () => {
  const front = new FrontMatter();
  it('liest die Felder des Kopfes, auch mit CRLF', () => {
    expect(front.read('---\nname: x\ndescription: Kurz\n---\nText')).toEqual({ name: 'x', description: 'Kurz' });
    expect(front.read('---\r\nname: y\r\n---\r\n')).toEqual({ name: 'y' });
  });
  it('liefert ohne oder mit kaputtem Kopf ein leeres Objekt', () => {
    expect(front.read('# Nur Text')).toEqual({});
    expect(front.read('---\nname: [offen\n---\n')).toEqual({});
  });
  it('meldet einen YAML-Fehler im Kopf statt ihn zu verschweigen', () => {
    const broken = front.parse('---\ndescription: Review: prüft vor dem Abschluss\n---\n');
    expect(broken.fields).toEqual({});
    expect(broken.error).toEqual(expect.any(String));
    expect(broken.error).not.toContain('\n');
    // Ohne Kopf, mit leerem und mit gültigem Kopf gibt es keinen Fehler
    expect(front.parse('# Nur Text')).toEqual({ fields: {}, error: null });
    expect(front.parse('---\n\n---\n')).toEqual({ fields: {}, error: null });
    expect(front.parse('---\nname: x\n---\n')).toEqual({ fields: { name: 'x' }, error: null });
  });
  it('liefert Textfelder getrimmt, leere als null', () => {
    expect(front.text({ description: ' Kurz ' }, 'description')).toBe('Kurz');
    expect(front.text({ description: '  ' }, 'description')).toBeNull();
    expect(front.text({ description: 3 }, 'description')).toBeNull();
    expect(front.text({}, 'name')).toBeNull();
  });
});
