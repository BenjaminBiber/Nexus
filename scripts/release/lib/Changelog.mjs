// CHANGELOG.md: den Abschnitt „Unveröffentlicht“ in einen Versionsabschnitt umbenennen und den Abschnitt einer
// Version wieder auslesen (für die Release-Notes). Reine Funktionen auf dem Text, ohne Datei und ohne Zustand.

// Überschrift des noch nicht veröffentlichten Abschnitts
export const UNRELEASED = '## Unveröffentlicht';
// Zwischen Version und Datum steht ein Gedankenstrich (U+2013), wie in den Überschriften des Changelogs
const DASH = '–';
// YYYY-MM-DD
const DATE = /^\d{4}-\d{2}-\d{2}$/;

// Der leere Abschnitt, der nach einem Release über dem Versionsabschnitt steht
function emptySection(eol) {
  return [
    UNRELEASED, '',
    '### Vertrag', '',
    '- Keine Änderungen.', '',
    '### Angekündigt', '',
    '- Keine.', '',
    '### Sonstiges', '',
  ].join(eol);
}

// Überschrift eines Versionsabschnitts: `## 0.2.0 – 2026-11-01`
export function heading(version, date) {
  return `## ${version} ${DASH} ${date}`;
}

// Zeilenende des Texts; eingefügte Zeilen sollen dazu passen
function eolOf(text) {
  return text.includes('\r\n') ? '\r\n' : '\n';
}

// Nummer der Zeile mit `## Unveröffentlicht`; -1, wenn es sie nicht gibt
function unreleasedLine(lines) {
  return lines.findIndex((line) => line.trim() === UNRELEASED);
}

// Benennt „Unveröffentlicht“ in den Versionsabschnitt um und setzt darüber einen neuen leeren Abschnitt.
// Wirft, wenn es „Unveröffentlicht“ nicht gibt – dann wäre nicht klar, was in das Release gehört.
export function release(text, version, date) {
  if (!DATE.test(date)) throw new Error(`„${date}“ ist kein Datum (YYYY-MM-DD erwartet).`);
  const eol = eolOf(text);
  const lines = text.split(/\r?\n/);
  const index = unreleasedLine(lines);
  if (index === -1) throw new Error(`In CHANGELOG.md fehlt die Überschrift „${UNRELEASED}“.`);
  if (lines.some((line) => line.trim().startsWith(`## ${version} `) || line.trim() === `## ${version}`)) {
    throw new Error(`In CHANGELOG.md steht schon ein Abschnitt für ${version}.`);
  }
  lines.splice(index, 1, emptySection(eol), heading(version, date));
  return lines.join(eol);
}

// Text eines Versionsabschnitts ohne seine Überschrift; `null`, wenn es den Abschnitt nicht gibt
export function section(text, version) {
  const lines = text.split(/\r?\n/);
  const start = lines.findIndex((line) => {
    const trimmed = line.trim();
    return trimmed === `## ${version}` || trimmed.startsWith(`## ${version} `);
  });
  if (start === -1) return null;
  let end = lines.length;
  for (let index = start + 1; index < lines.length; index += 1) {
    if (lines[index].startsWith('## ')) {
      end = index;
      break;
    }
  }
  return lines.slice(start + 1, end).join(eolOf(text)).trim();
}
