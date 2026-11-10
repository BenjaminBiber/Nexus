// Macht aus der von opentype.js erzeugten OpenType-Datei eine TrueType-Datei: opentype.js schreibt die Umrisse
// nur als CFF („OTTO“), die Pixel-Rechtecke des Spiels gehören aber in `glyf`/`loca` (Dateikennung 00 01 00 00).
// Alle übrigen Tabellen (head, hhea, hmtx, cmap, name, OS/2, post) bleiben wie von opentype.js gebaut.

/** Eine Kontur ist ein geschlossener Zug aus Punkten (x, y) in Schrifteinheiten; alle Punkte liegen auf der Kurve. */
export type Contour = Array<[number, number]>;

const SFNT_TRUE_TYPE = 0x00010000;
const CHECKSUM_MAGIC = 0xb1b0afba;
const HEAD_CHECKSUM_ADJUSTMENT = 8;
const HEAD_INDEX_TO_LOC_FORMAT = 50;
const ON_CURVE = 0x01;

interface SfntTable {
  tag: string;
  data: Uint8Array;
}

/** Summe über alle 32-Bit-Wörter, mit Nullen auf ein Vielfaches von 4 aufgefüllt */
function checkSum(data: Uint8Array): number {
  let sum = 0;
  for (let i = 0; i < data.length; i += 4) {
    const word =
      (data[i] << 24) | ((data[i + 1] ?? 0) << 16) | ((data[i + 2] ?? 0) << 8) | (data[i + 3] ?? 0);
    sum = (sum + (word >>> 0)) >>> 0;
  }
  return sum;
}

/** Tabellen der Datei in der Reihenfolge des Verzeichnisses */
function readTables(file: ArrayBuffer): SfntTable[] {
  const view = new DataView(file);
  const bytes = new Uint8Array(file);
  const count = view.getUint16(4);
  const tables: SfntTable[] = [];
  for (let i = 0; i < count; i++) {
    const at = 12 + i * 16;
    const tag = String.fromCharCode(bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]);
    const offset = view.getUint32(at + 8);
    const length = view.getUint32(at + 12);
    tables.push({ tag, data: bytes.slice(offset, offset + length) });
  }
  return tables;
}

/** Eine einfache Glyphe: Konturen mit Punkten auf der Kurve, ohne Anweisungen; leere Konturen ergeben 0 Bytes */
function glyphData(contours: Contour[]): Uint8Array {
  const points = contours.flat();
  if (points.length === 0) return new Uint8Array(0);
  const xs = points.map(([x]) => x);
  const ys = points.map(([, y]) => y);
  const size = 10 + contours.length * 2 + 2 + points.length * 5;
  const out = new Uint8Array(size + ((4 - (size % 4)) % 4));
  const view = new DataView(out.buffer);
  view.setInt16(0, contours.length);
  view.setInt16(2, Math.min(...xs));
  view.setInt16(4, Math.min(...ys));
  view.setInt16(6, Math.max(...xs));
  view.setInt16(8, Math.max(...ys));
  let at = 10;
  let end = 0;
  for (const contour of contours) {
    end += contour.length;
    view.setUint16(at, end - 1);
    at += 2;
  }
  view.setUint16(at, 0); // instructionLength
  at += 2;
  for (let i = 0; i < points.length; i++) out[at + i] = ON_CURVE;
  at += points.length;
  let previous = 0;
  for (const x of xs) {
    view.setInt16(at, x - previous);
    previous = x;
    at += 2;
  }
  previous = 0;
  for (const y of ys) {
    view.setInt16(at, y - previous);
    previous = y;
    at += 2;
  }
  return out;
}

/** `maxp` in Fassung 1.0, wie TrueType sie verlangt (opentype.js schreibt die kurze Fassung 0.5 für CFF) */
function makeMaxp(outlines: Contour[][]): Uint8Array {
  const out = new Uint8Array(32);
  const view = new DataView(out.buffer);
  view.setUint32(0, 0x00010000);
  view.setUint16(4, outlines.length);
  view.setUint16(6, Math.max(0, ...outlines.map((g) => g.flat().length))); // maxPoints
  view.setUint16(8, Math.max(0, ...outlines.map((g) => g.length))); // maxContours
  view.setUint16(14, 1); // maxZones
  return out;
}

/**
 * Setzt die Umrisse als `glyf`/`loca` in die Datei und schreibt sie als TrueType neu.
 * `outlines` nennt die Konturen je Glyphe in der Reihenfolge der Glyphen (Index 0 ist `.notdef`).
 */
export function toTrueType(otf: ArrayBuffer, outlines: Contour[][]): ArrayBuffer {
  const parts = outlines.map(glyphData);
  const glyf = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
  const loca = new Uint8Array((parts.length + 1) * 4);
  const locaView = new DataView(loca.buffer);
  let at = 0;
  parts.forEach((part, i) => {
    locaView.setUint32(i * 4, at);
    glyf.set(part, at);
    at += part.length;
  });
  locaView.setUint32(parts.length * 4, at);

  const tables = readTables(otf).filter((t) => t.tag !== 'CFF ' && t.tag !== 'maxp');
  const head = tables.find((t) => t.tag === 'head');
  if (head) {
    const view = new DataView(head.data.buffer, head.data.byteOffset, head.data.byteLength);
    view.setUint32(HEAD_CHECKSUM_ADJUSTMENT, 0);
    view.setInt16(HEAD_INDEX_TO_LOC_FORMAT, 1); // lange `loca`-Einträge
  }
  tables.push({ tag: 'maxp', data: makeMaxp(outlines) }, { tag: 'glyf', data: glyf }, { tag: 'loca', data: loca });
  tables.sort((a, b) => (a.tag < b.tag ? -1 : 1));

  const directory = 12 + tables.length * 16;
  const offsets: number[] = [];
  let total = directory;
  for (const table of tables) {
    offsets.push(total);
    total += table.data.length + ((4 - (table.data.length % 4)) % 4);
  }
  const file = new Uint8Array(total);
  const view = new DataView(file.buffer);
  const highest = 2 ** Math.floor(Math.log2(tables.length));
  view.setUint32(0, SFNT_TRUE_TYPE);
  view.setUint16(4, tables.length);
  view.setUint16(6, highest * 16);
  view.setUint16(8, Math.log2(highest));
  view.setUint16(10, tables.length * 16 - highest * 16);
  tables.forEach((table, i) => {
    const at = 12 + i * 16;
    for (let c = 0; c < 4; c++) file[at + c] = table.tag.charCodeAt(c);
    view.setUint32(at + 4, checkSum(table.data));
    view.setUint32(at + 8, offsets[i]);
    view.setUint32(at + 12, table.data.length);
    file.set(table.data, offsets[i]);
  });
  const headAt = offsets[tables.findIndex((t) => t.tag === 'head')];
  if (headAt !== undefined) {
    view.setUint32(headAt + HEAD_CHECKSUM_ADJUSTMENT, (CHECKSUM_MAGIC - checkSum(file)) >>> 0);
  }
  return file.buffer;
}
