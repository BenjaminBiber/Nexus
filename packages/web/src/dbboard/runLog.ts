const TAGGED = /^(\d\d:\d\d:\d\d) {2}\[(start|say|read|edit|search|test|run|todo|tool|ok|fail|done|abort)(?:#([^\]]+))?\] ?([^\t]*)(?:\t(".*"))?$/;
/** Zeile mit Uhrzeit, ohne Marke (ältere Protokolle) */
export const TIMED = /^(\d\d:\d\d:\d\d) {2}(.*)$/;
/** Meldung des Dispatchers, wahlweise mit ISO-Zeit */
export const DISPATCH = /^\[dispatch\] (?:(\d{4}-\d\d-\d\dT[\d:.]+Z) )?(.*)$/;

/** Eine getaggte Zeile des Lauf-Protokolls */
export interface Tagged {
  time: string;
  kind: string;
  ref?: string;
  text: string;
  detail?: string;
}

// Zerlegt eine getaggte Zeile („HH:MM:SS  [art#ref] text<TAB>"detail"“, siehe StreamEventFormatter im Core); sonst null
export function tagged(line: string): Tagged | null {
  const m = TAGGED.exec(line);
  if (!m) return null;
  let detail: string | undefined;
  if (m[5]) {
    try {
      detail = JSON.parse(m[5]) as string;
    } catch {
      detail = m[5];
    }
  }
  return { time: m[1]!, kind: m[2]!, ...(m[3] ? { ref: m[3] } : {}), text: m[4]!, ...(detail !== undefined ? { detail } : {}) };
}

// Kopf des Dispatchers kompakt: ohne Präfix, Zeitstempel und den langen Aufruf von Claude
export function compactRunHeader(header: string): string[] {
  return header
    .split('\n')
    .map((l) => l.replace(/\r$/, ''))
    .filter((l) => l.trim() && !/^\[dispatch\] \S+ (?:-p|--)/.test(l) && !/ --output-format /.test(l))
    .map((l) => {
      const text = l.replace(DISPATCH, (_m, _iso: string | undefined, rest: string) => rest);
      const start = /^(\S+) startet in (.+?) \((.+)\)$/.exec(text);
      if (start) return `${start[1]} · ${start[3]} · ${start[2]!.replace(/\\/g, '/').split('/').filter(Boolean).pop() ?? start[2]}`;
      return text.replace(/^Profil (\S+): /, 'Profil $1 · ').replace(/, (?![^(]*\))/g, ' · ');
    });
}
