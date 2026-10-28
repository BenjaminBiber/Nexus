// Semantische Versionen: lesen, schreiben, erhöhen, vergleichen. Reine Funktionen ohne Zustand, damit sie prüfbar bleiben.

// X.Y.Z mit optionaler Vorabversion (-rc.1) und optionalem Build-Anhang (+abc)
const PATTERN = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-([0-9A-Za-z.-]+))?(?:\+([0-9A-Za-z.-]+))?$/;
// Erhöhbare Stellen
export const KINDS = ['major', 'minor', 'patch'];

// Zerlegt eine Version; wirft bei allem, was keine Version ist
export function parse(text) {
  if (typeof text !== 'string') throw new Error(`„${text}“ ist keine Version (X.Y.Z erwartet).`);
  const match = PATTERN.exec(text.trim());
  if (!match) throw new Error(`„${text}“ ist keine Version (X.Y.Z erwartet).`);
  return {
    major: Number(match[1]),
    minor: Number(match[2]),
    patch: Number(match[3]),
    prerelease: match[4] ?? null,
    build: match[5] ?? null,
  };
}

// Wie parse, aber `null` statt eines Fehlers – für Tags, die niemand geprüft hat
export function tryParse(text) {
  try {
    return parse(text);
  } catch {
    return null;
  }
}

// Schreibt eine zerlegte Version wieder als Text
export function format(version) {
  const base = `${version.major}.${version.minor}.${version.patch}`;
  return `${base}${version.prerelease ? `-${version.prerelease}` : ''}${version.build ? `+${version.build}` : ''}`;
}

// Nächste Version. Eine Vorabversion gilt als Weg zu ihrer eigenen Version:
// 1.4.2-rc.1 + patch ist 1.4.2, nicht 1.4.3 (so hält es auch `npm version`).
export function bump(text, kind) {
  if (!KINDS.includes(kind)) throw new Error(`„${kind}“ ist keine Stelle (${KINDS.join(', ')} erwartet).`);
  const { major, minor, patch, prerelease } = parse(text);
  if (kind === 'major') return `${minor === 0 && patch === 0 && prerelease ? major : major + 1}.0.0`;
  if (kind === 'minor') return `${major}.${patch === 0 && prerelease ? minor : minor + 1}.0`;
  return `${major}.${minor}.${prerelease ? patch : patch + 1}`;
}

// Vergleicht die Teile einer Vorabversion: Zahlen zählen als Zahlen und stehen vor Text (SemVer 11.4)
function comparePrerelease(a, b) {
  if (a === b) return 0;
  // Keine Vorabversion ist die höhere Version: 1.0.0 steht über 1.0.0-rc.1
  if (!a) return 1;
  if (!b) return -1;
  const left = a.split('.');
  const right = b.split('.');
  for (let index = 0; index < Math.max(left.length, right.length); index += 1) {
    const one = left[index];
    const other = right[index];
    // Die kürzere Reihe ist die kleinere, wenn alles davor gleich ist
    if (one === undefined) return -1;
    if (other === undefined) return 1;
    const oneIsNumber = /^\d+$/.test(one);
    const otherIsNumber = /^\d+$/.test(other);
    if (oneIsNumber && otherIsNumber) {
      if (Number(one) !== Number(other)) return Number(one) < Number(other) ? -1 : 1;
    } else if (oneIsNumber !== otherIsNumber) return oneIsNumber ? -1 : 1;
    else if (one !== other) return one < other ? -1 : 1;
  }
  return 0;
}

// -1, 0 oder 1. Der Build-Anhang zählt nicht mit (SemVer 10)
export function compare(a, b) {
  const left = parse(a);
  const right = parse(b);
  for (const part of ['major', 'minor', 'patch']) {
    if (left[part] !== right[part]) return left[part] < right[part] ? -1 : 1;
  }
  return comparePrerelease(left.prerelease, right.prerelease);
}

// Höchste Version einer Liste; `null`, wenn die Liste leer ist. Unlesbare Einträge gehören nicht hierher (siehe tryParse)
export function highest(versions) {
  let best = null;
  for (const version of versions) {
    if (best === null || compare(version, best) > 0) best = version;
  }
  return best;
}
