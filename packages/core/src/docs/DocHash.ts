import { createHash } from 'node:crypto';

// Stabiler Fingerabdruck eines Dokumentinhalts. Zeilenenden und BOM hängen davon ab, wie ein
// Editor gespeichert hat – ohne Normalisierung meldete das Speichern sonst falsche Konflikte.
export const DocHash = {
  // Voller SHA-256 als Hex; voll statt gekürzt, weil der Wert Konflikte erkennen soll
  of(content: string): string {
    const normalized = content.replace(/^﻿/, '').replace(/\r\n/g, '\n');
    return createHash('sha256').update(normalized, 'utf8').digest('hex');
  },
};
