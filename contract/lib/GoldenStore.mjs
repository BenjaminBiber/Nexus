import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname } from 'node:path';
import { JsonText } from './JsonText.mjs';

// Liest, schreibt und vergleicht die Golden-Dateien (`*.golden.json`).
export class GoldenStore {
  // Liest die Datei; null, wenn es sie nicht gibt
  read(file) {
    if (!existsSync(file)) return null;
    const text = readFileSync(file, 'utf8');
    let value;
    try {
      value = JSON.parse(text);
    } catch (error) {
      throw new Error(`${file} ist kein gültiges JSON (${error.message}).`);
    }
    if (!Array.isArray(value)) throw new Error(`${file} muss eine Liste von Schritten enthalten.`);
    return value;
  }

  // Schreibt die Datei nur, wenn sich ihr Inhalt ändert; liefert true bei Änderung
  write(file, entries) {
    const text = `${JsonText.canonical(entries)}\n`;
    if (existsSync(file) && readFileSync(file, 'utf8') === text) return false;
    mkdirSync(dirname(file), { recursive: true });
    writeFileSync(file, text);
    return true;
  }

  // Vergleicht erwartete und aufgezeichnete Schritte; liefert die Abweichungen als lesbare Texte
  static compare(expected, actual) {
    const findings = [];
    for (let i = 0; i < Math.max(expected.length, actual.length); i += 1) {
      const want = expected[i];
      const got = actual[i];
      if (want === undefined) {
        findings.push(`Schritt „${got.name}“ ist neu – die Golden-Datei kennt ihn nicht (mit --update aufnehmen).`);
        continue;
      }
      if (got === undefined) {
        findings.push(`Schritt „${want.name}“ steht in der Golden-Datei, wurde aber nicht aufgezeichnet.`);
        continue;
      }
      if (want.name !== got.name) {
        findings.push(`An Stelle ${i + 1} steht „${want.name}“ in der Golden-Datei, aufgezeichnet wurde „${got.name}“.`);
        continue;
      }
      const diff = JsonText.diff(want, got);
      if (diff) findings.push(`Schritt „${want.name}“ weicht ab (- Golden-Datei, + aufgezeichnet):\n${diff}`);
    }
    return findings;
  }
}
