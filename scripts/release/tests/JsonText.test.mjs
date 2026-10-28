import { describe, expect, it } from 'vitest';
import { JsonText } from '../lib/JsonText.mjs';

const TEXT = `{
  "name": "fix-harness",
  "version": "0.1.0",
  "author": { "name": "Fixture" },
  "keywords": ["fixture", "release"],
  "plugins": [
    { "name": "eins", "version": "1.0.0" },
    { "name": "zwei" }
  ],
  "zahl": 7,
  "an": true,
  "nichts": null
}
`;

describe('JsonText', () => {
  it('liest Werte über ihren Pfad', () => {
    const doc = new JsonText(TEXT);
    expect(doc.value(['version'])).toBe('0.1.0');
    expect(doc.value(['author', 'name'])).toBe('Fixture');
    expect(doc.value(['keywords', 1])).toBe('release');
    expect(doc.value(['plugins', 0, 'version'])).toBe('1.0.0');
    expect(doc.value(['zahl'])).toBe(7);
    expect(doc.value(['an'])).toBe(true);
    expect(doc.value(['nichts'])).toBeNull();
  });

  it('kennt vorhandene und fehlende Stellen', () => {
    const doc = new JsonText(TEXT);
    expect(doc.has(['plugins', 1, 'name'])).toBe(true);
    expect(doc.has(['plugins', 1, 'version'])).toBe(false);
    expect(doc.value(['gibtsnicht'])).toBeUndefined();
  });

  it('liefert den Inhalt auch als gewöhnliches Objekt', () => {
    expect(new JsonText(TEXT).data).toEqual(JSON.parse(TEXT));
  });

  it('ersetzt einen Wert und lässt den Rest der Datei Zeichen für Zeichen stehen', () => {
    const doc = new JsonText(TEXT);
    doc.setString(['version'], '0.2.0');
    expect(doc.text).toBe(TEXT.replace('"0.1.0"', '"0.2.0"'));
    // Die einzeiligen Stellen bleiben einzeilig – anders als bei JSON.stringify
    expect(doc.text).toContain('"author": { "name": "Fixture" },');
    expect(doc.text).toContain('"keywords": ["fixture", "release"],');
  });

  it('ersetzt mehrere Werte nacheinander', () => {
    const doc = new JsonText(TEXT);
    doc.setString(['version'], '0.2.0').setString(['plugins', 0, 'version'], '0.2.0').setString(['author', 'name'], 'Neu');
    expect(doc.value(['version'])).toBe('0.2.0');
    expect(doc.value(['plugins', 0, 'version'])).toBe('0.2.0');
    expect(doc.value(['author', 'name'])).toBe('Neu');
    expect(JSON.parse(doc.text).plugins[1]).toEqual({ name: 'zwei' });
  });

  it('maskiert Sonderzeichen beim Schreiben', () => {
    const doc = new JsonText(TEXT);
    doc.setString(['author', 'name'], 'A"B\\C');
    expect(JSON.parse(doc.text).author.name).toBe('A"B\\C');
  });

  it('fügt ein Mitglied hinter einem vorhandenen ein, mit derselben Einrückung', () => {
    const doc = new JsonText(TEXT);
    doc.insertStringAfter(['plugins', 1, 'name'], 'version', '0.2.0');
    expect(doc.value(['plugins', 1, 'version'])).toBe('0.2.0');
    expect(JSON.parse(doc.text).plugins[1]).toEqual({ name: 'zwei', version: '0.2.0' });
  });

  it('fügt auch hinter dem letzten Mitglied eines Objekts ein', () => {
    const doc = new JsonText('{\n  "name": "eins"\n}\n');
    doc.insertStringAfter(['name'], 'version', '0.2.0');
    expect(doc.text).toBe('{\n  "name": "eins",\n  "version": "0.2.0"\n}\n');
  });

  it('entfernt ein Mitglied samt seinem Komma', () => {
    const doc = new JsonText(TEXT);
    doc.remove(['version']);
    expect(doc.has(['version'])).toBe(false);
    expect(doc.text).toBe(TEXT.replace('  "version": "0.1.0",\n', ''));
  });

  it('entfernt auch das erste und das letzte Mitglied', () => {
    const first = new JsonText('{\n  "version": "1.0.0",\n  "name": "eins"\n}\n');
    first.remove(['version']);
    expect(JSON.parse(first.text)).toEqual({ name: 'eins' });
    const last = new JsonText('{\n  "name": "eins",\n  "version": "1.0.0"\n}\n');
    last.remove(['version']);
    expect(JSON.parse(last.text)).toEqual({ name: 'eins' });
    const only = new JsonText('{\n  "version": "1.0.0"\n}\n');
    only.remove(['version']);
    expect(JSON.parse(only.text)).toEqual({});
  });

  it('wirft bei einer Stelle, die es nicht gibt', () => {
    expect(() => new JsonText(TEXT).setString(['gibtsnicht'], 'x')).toThrow(/gibt es in dieser Datei nicht/);
  });

  it('wirft bei Text, der kein JSON ist', () => {
    expect(() => new JsonText('{ "a": }')).toThrow(/lässt sich nicht lesen/);
    expect(() => new JsonText('{ "a": 1 } überzählig')).toThrow(/lässt sich nicht lesen/);
    expect(() => new JsonText('{ "a": "offen }')).toThrow(/lässt sich nicht lesen/);
  });

  it('nimmt für eingefügte Zeilen das Zeilenende der Datei', () => {
    const doc = new JsonText('{\r\n  "name": "eins"\r\n}\r\n');
    doc.insertStringAfter(['name'], 'version', '0.2.0');
    expect(doc.text).toBe('{\r\n  "name": "eins",\r\n  "version": "0.2.0"\r\n}\r\n');
  });
});
