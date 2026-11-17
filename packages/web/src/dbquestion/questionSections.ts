// Abschnitte von Beschreibung und Recherche-Kontext einer Frage. Die Schreibregeln (Skill board-question, Prompts für Planung
// und Recherche) gliedern beide in fett gesetzte Überschriften auf eigener Zeile: **Worum es geht**, **Heute**,
// **Zu entscheiden**, **Varianten**, **Empfehlung**, **Fundstellen**, **Weitere Frage nötig**. Die Fragen-Seite verteilt
// diese Abschnitte auf Entscheidungskarte, Reiter und Seitenleiste; Text ohne Gliederung bleibt als Ganzes stehen.

export interface MdSection {
  title: string;
  body: string;
}

export interface MdSections {
  /** Text vor der ersten Überschrift */
  intro: string;
  sections: MdSection[];
}

// Überschrift-Zeile: „## Titel“, „**Titel**“ oder „**Titel:** Text“ (mit Doppelpunkt darf Text auf derselben Zeile folgen)
function headingOf(line: string): { title: string; rest: string } | null {
  const md = /^\s{0,3}#{1,6}\s+(.+?)\s*#*\s*$/.exec(line);
  if (md) return { title: clean(md[1]), rest: '' };
  const bold = /^\s*\*\*([^*\n]+?)\*\*\s*(:?)\s*(.*)$/.exec(line);
  if (!bold) return null;
  const [, inner, colonAfter, rest] = bold;
  const colon = inner.trimEnd().endsWith(':') || colonAfter === ':';
  if (rest.trim() && !colon) return null;
  return { title: clean(inner), rest: rest.trim() };
}

function clean(title: string): string {
  return title.trim().replace(/:$/, '').trim();
}

// Zerlegt Markdown in Einleitung und Abschnitte; Überschriften in Codeblöcken zählen nicht
export function splitSections(source: string): MdSections {
  const intro: string[] = [];
  const sections: Array<{ title: string; lines: string[] }> = [];
  let fence = false;
  for (const line of source.replace(/\r\n?/g, '\n').split('\n')) {
    if (/^\s*(```|~~~)/.test(line)) fence = !fence;
    const head = fence ? null : headingOf(line);
    if (head) {
      sections.push({ title: head.title, lines: head.rest ? [head.rest] : [] });
      continue;
    }
    (sections.at(-1)?.lines ?? intro).push(line);
  }
  return { intro: intro.join('\n').trim(), sections: sections.map((s) => ({ title: s.title, body: s.lines.join('\n').trim() })) };
}

// Vergleichsform eines Titels: klein, ohne Satzzeichen
function norm(title: string): string {
  return title.toLowerCase().replace(/[^\p{L}\p{N} ]/gu, ' ').replace(/\s+/g, ' ').trim();
}

// Passt ein Abschnittstitel zu einem der Namen? Gleich oder mit Zusatz dahinter („Heute (Stand Oktober)“)
export function isSection(title: string, ...names: string[]): boolean {
  const t = norm(title);
  return names.some((name) => {
    const n = norm(name);
    return t === n || t.startsWith(`${n} `);
  });
}

// Erster Abschnitt mit einem der Namen; leere Abschnitte zählen nicht
export function findSection(parsed: MdSections, ...names: string[]): MdSection | undefined {
  return parsed.sections.find((s) => s.body !== '' && isSection(s.title, ...names));
}

export const SECTION = {
  about: ['Worum es geht'],
  decide: ['Zu entscheiden'],
  today: ['Heute'],
  variants: ['Varianten'],
  recommendation: ['Empfehlung'],
  findings: ['Fundstellen'],
  followUp: ['Weitere Frage nötig', 'Weitere Frage', 'Folgefrage'],
} as const;

// Abschnitt mit Herkunft: aus der Beschreibung der Frage oder aus dem Kontext der neuesten Recherche
export interface SourcedSection {
  body: string;
  from: 'description' | 'research';
}

// Was die Fragen-Seite aus Beschreibung und neuester Recherche zieht: „Zu entscheiden“ und „Worum es geht“ (Beschreibung vor
// Recherche), „Heute“ und „Weitere Frage nötig“ aus der Recherche
export interface QuestionParts {
  description: MdSections;
  context: MdSections | null;
  decide: SourcedSection | null;
  about: SourcedSection | null;
  today: string | null;
  followUp: string | null;
}

export function questionParts(question: { description: string; research: Array<{ context: string }> }): QuestionParts {
  const description = splitSections(question.description);
  const latest = question.research.at(-1);
  const context = latest ? splitSections(latest.context) : null;
  const sourced = (names: readonly string[]): SourcedSection | null => {
    const own = findSection(description, ...names);
    if (own) return { body: own.body, from: 'description' };
    const researched = context ? findSection(context, ...names) : undefined;
    return researched ? { body: researched.body, from: 'research' } : null;
  };
  return {
    description,
    context,
    decide: sourced(SECTION.decide),
    about: sourced(SECTION.about),
    today: (context && findSection(context, ...SECTION.today)?.body) ?? null,
    followUp: (context && findSection(context, ...SECTION.followUp)?.body) ?? null,
  };
}

// Aufwand am Ende des Optionstexts („… Aufwand: klein“) als eigenes Merkmal; der Text bleibt ohne ihn
export function splitEffort(text: string): { text: string; effort: string | null } {
  const match = /\(?\bAufwand:\s*(klein|standard|schwer)\b\)?\.?/i.exec(text);
  if (!match) return { text, effort: null };
  const before = text.slice(0, match.index).trimEnd();
  const after = text.slice(match.index + match[0].length).trimStart();
  return { text: [before, after].filter(Boolean).join(' '), effort: match[1].toLowerCase() };
}

export interface Finding {
  path: string;
  note: string;
}

// Zeile der Fundstellen: „Pfad:Zeile – was dort passiert“, wahlweise als Aufzählung und mit Pfad in Backticks
const FINDING = /^(?:[-*+]\s+|\d+\.\s+)?(?:`([^`]+)`|(\S+))(?:\s*(?:\s[–—-]+|:)\s+(.*))?$/;

// Fundstellen als Liste; null, wenn eine Zeile nicht dem Muster folgt (dann zeigt die Seite den Abschnitt als Markdown)
export function parseFindings(body: string): Finding[] | null {
  const lines = body.split('\n').map((l) => l.trim()).filter(Boolean);
  const found: Finding[] = [];
  for (const line of lines) {
    const match = FINDING.exec(line);
    const path = (match?.[1] ?? match?.[2] ?? '').trim();
    if (!match || !/[/\\]|\.\w+/.test(path)) return null;
    found.push({ path, note: (match[3] ?? '').trim() });
  }
  return found.length > 0 ? found : null;
}

// Entwurf einer Folgefrage aus dem Abschnitt „Weitere Frage nötig“: Titel bis zum ersten Fragezeichen, der ganze Satz als Beschreibung
export function followUpDraft(text: string): { title: string; description: string } {
  const plain = text
    .split('\n')
    .map((l) => l.replace(/^\s*(?:[-*+]|\d+\.)\s+/, '').trim())
    .filter(Boolean)
    .join(' ')
    .replace(/\*\*|__/g, '')
    .trim();
  const end = plain.indexOf('?');
  const title = end > 0 ? plain.slice(0, end + 1) : plain;
  return { title, description: title === plain ? '' : plain };
}
