import type { NavIconName } from '@nexus/theme-sdk';

/** Farbe eines Ausgabeteils; die Farben selbst stehen in theme.css */
export type OutputTone = 'plain' | 'muted' | 'error' | 'ok' | 'id' | 'dir' | 'highlight';

/** Teil einer Ausgabezeile */
export interface OutputCell {
  text: string;
  tone?: OutputTone;
}

/** Ausgabe: eine Zeile aus Teilen hintereinander oder eine Tabelle mit ausgerichteten Spalten */
export type OutputBlock = { kind: 'line'; cells: OutputCell[] } | { kind: 'table'; rows: OutputCell[][] };

/** Board, wie es die Befehle kennen */
export interface TerminalBoard {
  id: number;
  name: string;
  /** Anzeigename des Projekts */
  project: string;
  /** Schlüssel des Projekts für die Board-Wahl */
  projectKey: string;
  active: boolean;
}

/** Bereich der Hauptnavigation, wie ihn der Host gerade zeigt */
export interface TerminalSection {
  id: NavIconName;
  label: string;
  badge?: number;
}

/** Was die Befehle in der Oberfläche auslösen und lesen können (umgesetzt mit den Hooks des Theme-SDK) */
export interface TerminalActions {
  /** Pfad unter /p/:pid, z. B. „board/t/ARC-0001“ */
  path(): string;
  /** Bereiche der Hauptnavigation samt Zählern */
  sections(): TerminalSection[];
  go(path: string): void;
  openTicket(id: string): void;
  /** Öffnet die globale Suche des Hosts (ohne Suchtext, den reicht das SDK nicht weiter) */
  search(): void;
  boards(): TerminalBoard[];
  switchBoard(board: TerminalBoard): void;
  /** Eingestelltes Farbschema (data-theme-mode), z. B. „system“ oder „terminal“ */
  theme(): string | null;
  setTheme(mode: string): Promise<void>;
  /** Claude-Nutzung in Prozent (5 h und 7 Tage); null, wenn ausgeschaltet oder nicht verfügbar */
  usage(): TerminalUsage | null;
  /** Bisher abgeschickte Befehle, ältester zuerst */
  history(): string[];
}

/** Claude-Nutzung, wie sie fetch zeigt */
export interface TerminalUsage {
  fiveHour: number | null;
  sevenDay: number | null;
}

/** Vorschläge für die Ergänzung, die nicht fest im Code stehen */
export interface CompletionData {
  boards: string[];
}

interface Named {
  names: string[];
}

interface Section extends Named {
  route: NavIconName;
}

interface ThemeName extends Named {
  mode: string;
}

interface Command {
  name: string;
  /** Weitere Namen, z. B. neofetch für fetch */
  aliases?: string[];
  usage: string;
  summary: string;
  /** Längere Beschreibung und Beispiele für man */
  details?: string[];
  examples?: string[];
  /** Nicht in help und in der Ergänzung (Easter Eggs) */
  hidden?: boolean;
  run(args: string[], actions: TerminalActions): Promise<OutputBlock[] | null> | OutputBlock[] | null;
  pool?(data: CompletionData): string[];
}

// Bereiche der Hauptnavigation von Nexus; der erste Name ist der angezeigte, die übrigen sind deutsche Zweitnamen
export const TERMINAL_SECTIONS: Section[] = [
  { route: 'board', names: ['board'] },
  { route: 'inbox', names: ['inbox', 'aufgaben'] },
  { route: 'dashboard', names: ['dashboard'] },
  { route: 'sessions', names: ['sessions'] },
  { route: 'questions', names: ['questions', 'fragen'] },
  { route: 'docs', names: ['docs'] },
  { route: 'guide', names: ['guide', 'anleitung'] },
  { route: 'settings', names: ['settings', 'einstellungen'] },
];

// Eingebaute und mitgelieferte Farbschemata; installierte Themes mit anderer ID nimmt „theme <id>“ unverändert an
export const TERMINAL_THEMES: ThemeName[] = [
  { mode: 'system', names: ['system'] },
  { mode: 'light', names: ['light', 'hell'] },
  { mode: 'dark', names: ['dark', 'dunkel'] },
  { mode: 'signal', names: ['signal'] },
  { mode: 'pastel', names: ['pastel', 'paper'] },
  { mode: 'win95', names: ['win95'] },
  { mode: 'minecraft', names: ['minecraft'] },
  { mode: 'catppuccin', names: ['catppuccin'] },
  { mode: 'terminal', names: ['terminal'] },
];

const TICKET_ID = /^[a-z][a-z0-9]*-\d+$/i;

// Muster einer Theme-ID (wie im Manifest)
const THEME_ID = /^[a-z0-9-]{2,40}$/;

const text = (value: string, tone?: OutputTone): OutputCell => (tone ? { text: value, tone } : { text: value });

const line = (...cells: OutputCell[]): OutputBlock => ({ kind: 'line', cells });

const table = (rows: OutputCell[][]): OutputBlock => ({ kind: 'table', rows });

const fail = (message: string, hint?: string): OutputBlock[] => [line(text(message, 'error')), ...(hint ? [line(text(hint, 'muted'))] : [])];

// Kleinbuchstaben, Leerraum als Bindestrich: so lassen sich Namen tippen und ergänzen
export function slug(value: string): string {
  return value.trim().toLowerCase().replace(/\s+/g, '-');
}

// Zerlegt eine Eingabe in Wörter; Text in Anführungszeichen bleibt ein Wort
export function parseLine(input: string): string[] {
  return [...input.matchAll(/"([^"]*)"|'([^']*)'|(\S+)/g)].map((m) => m[1] ?? m[2] ?? m[3]!);
}

// Erster Eintrag, dessen Name genau passt, sonst der erste, dessen Name so anfängt
function findNamed<T extends Named>(list: T[], input: string): T | undefined {
  const wanted = input.toLowerCase();
  return list.find((item) => item.names.includes(wanted)) ?? list.find((item) => item.names.some((name) => name.startsWith(wanted)));
}

// Eindeutiger Tippname je Board; bei gleichen Namen mit dem Projekt davor
export function boardKey(board: TerminalBoard, all: TerminalBoard[]): string {
  const name = slug(board.name);
  return all.filter((b) => slug(b.name) === name).length > 1 ? `${slug(board.project)}/${name}` : name;
}

// Übergeordneter Pfad für „cd ..“; ganz oben bleibt das Board
export function parentPath(path: string): string {
  const parts = path.split('/').filter(Boolean);
  if (parts[0] === 'board' && parts[1] === 't') return 'board';
  return parts.length > 1 ? parts.slice(0, -1).join('/') : 'board';
}

const help: Command = {
  name: 'help',
  usage: 'help',
  summary: 'diese Hilfe',
  details: ['Listet alle Befehle mit Kurzbeschreibung. Ausführlich zu einem Befehl: man <befehl>.'],
  run: () => [table(COMMANDS.filter((c) => !c.hidden).map((c) => [text(c.usage, 'ok'), text(c.summary, 'muted')]))],
};

const man: Command = {
  name: 'man',
  usage: 'man <befehl>',
  summary: 'Anleitung zu einem Befehl',
  examples: ['man cd', 'man fetch'],
  pool: () => COMMANDS.filter((c) => !c.hidden).map((c) => c.name),
  run: ([name]) => {
    if (!name) return fail('man: welcher Befehl? z. B. man cd');
    const command = commandNamed(name);
    if (!command || command.hidden) return fail(`man: kein Eintrag für ${name}`, 'help zeigt alle Befehle');
    return [
      line(text(`${command.name.toUpperCase()}(1)`, 'highlight')),
      line(text('ÜBERSICHT', 'dir')),
      line(text(`  ${command.usage}`, 'ok')),
      ...(command.aliases?.length ? [line(text(`  auch: ${command.aliases.join(', ')}`, 'muted'))] : []),
      line(text('BESCHREIBUNG', 'dir')),
      ...[command.summary, ...(command.details ?? [])].map((d) => line(text(`  ${d}`))),
      ...(command.examples?.length ? [line(text('BEISPIELE', 'dir')), ...command.examples.map((e) => line(text('  $ ', 'muted'), text(e)))] : []),
    ];
  },
};

const cd: Command = {
  name: 'cd',
  usage: 'cd <bereich> · cd .. · cd ~',
  summary: 'Bereich wechseln',
  details: [
    'Bereiche: board, inbox (aufgaben), dashboard, sessions, questions (fragen), docs, guide (anleitung), settings (einstellungen).',
    'Ein Anfang genügt, solange er eindeutig ist; cd .. geht eine Stufe nach oben, cd ~ zurück aufs Board.',
  ],
  examples: ['cd inbox', 'cd sess', 'cd ..'],
  pool: () => TERMINAL_SECTIONS.flatMap((s) => s.names),
  run: ([target = '~'], actions) => {
    if (target === '~' || target === '/') {
      actions.go('board');
      return null;
    }
    if (target === '..') {
      actions.go(parentPath(actions.path()));
      return null;
    }
    const section = findNamed(TERMINAL_SECTIONS, target.replace(/^~?\//, '').replace(/\/$/, ''));
    if (!section) return fail(`cd: kein solcher Bereich: ${target}`, `Bereiche: ${TERMINAL_SECTIONS.map((s) => s.names[0]).join('  ')}`);
    actions.go(section.route);
    return null;
  },
};

const ls: Command = {
  name: 'ls',
  usage: 'ls',
  summary: 'Bereiche mit Zählern auflisten',
  run: (_args, actions) => {
    const shown = actions.sections();
    const rows = TERMINAL_SECTIONS.flatMap((s) => {
      const item = shown.find((n) => n.id === s.route);
      if (!item) return [];
      return [[text(`${s.names[0]}/`, 'dir'), text(item.label, 'muted'), ...(item.badge ? [text(`[${item.badge}]`, 'error')] : [])]];
    });
    return rows.length ? [table(rows)] : [line(text('Keine Bereiche.', 'muted'))];
  },
};

const open: Command = {
  name: 'open',
  usage: 'open <ID>',
  summary: 'Ticket öffnen (die ID allein genügt)',
  examples: ['open ARC-0042', 'ARC-0042'],
  run: ([id], actions) => {
    if (!id) return fail('open: Ticket-ID fehlt, z. B. open ARC-0042');
    if (!TICKET_ID.test(id)) return fail(`open: keine Ticket-ID: ${id}`);
    actions.openTicket(id.toUpperCase());
    return null;
  },
};

const find: Command = {
  name: 'find',
  usage: 'find',
  summary: 'Suche über alle Boards öffnen (auch Strg+K)',
  run: (_args, actions) => {
    actions.search();
    return null;
  },
};

const board: Command = {
  name: 'board',
  usage: 'board [name]',
  summary: 'Boards zeigen oder wechseln',
  details: ['Ohne Namen listet board alle Boards, das aktive mit *. Gleiche Namen bekommen das Projekt davor.'],
  examples: ['board', 'board sprint'],
  pool: (data) => data.boards,
  run: (args, actions) => {
    const boards = actions.boards();
    const wanted = args.join(' ');
    if (!wanted) {
      if (!boards.length) return [line(text('Keine Boards.', 'muted'))];
      return [table(boards.map((b) => [text(`${b.active ? '*' : ' '} ${boardKey(b, boards)}`, b.active ? 'ok' : 'plain'), text(b.project, 'muted')]))];
    }
    const named = boards.map((b) => ({ board: b, names: [boardKey(b, boards), slug(b.name)] }));
    const match = findNamed(named, slug(wanted));
    if (!match) return fail(`board: unbekannt: ${wanted}`, `Boards: ${named.map((n) => n.names[0]).join('  ')}`);
    actions.switchBoard(match.board);
    return [line(text(`board → ${match.names[0]}`, 'ok'))];
  },
};

const theme: Command = {
  name: 'theme',
  usage: 'theme [name]',
  summary: 'Farbschema zeigen oder wechseln',
  details: ['Wechselt das Theme der ganzen Oberfläche. Farben, Schrift und Effekte des Terminals stehen unter Einstellungen → Darstellung.'],
  examples: ['theme', 'theme dunkel'],
  pool: () => TERMINAL_THEMES.flatMap((t) => t.names),
  run: async ([name], actions) => {
    const current = actions.theme();
    if (!name) return [table(TERMINAL_THEMES.map((t) => [text(`${t.mode === current ? '*' : ' '} ${t.names[0]}`, t.mode === current ? 'ok' : 'plain')]))];
    const match = findNamed(TERMINAL_THEMES, name);
    const mode = match?.mode ?? (THEME_ID.test(name) ? name : null);
    if (!mode) return fail(`theme: unbekannt: ${name}`, `Farbschemata: ${TERMINAL_THEMES.map((t) => t.names[0]).join('  ')}`);
    await actions.setTheme(mode);
    return [line(text(`Farbschema → ${match?.names[0] ?? mode}`, 'ok'))];
  },
};

const clear: Command = {
  name: 'clear',
  usage: 'clear',
  summary: 'Ausgabe schließen (auch Esc oder Strg+L)',
  run: () => null,
};

const top: Command = {
  name: 'top',
  usage: 'top',
  summary: 'laufende Sessions zeigen',
  details: ['Springt zur Seite Sessions.'],
  run: (_args, actions) => {
    actions.go('sessions');
    return null;
  },
};

// Schriftzug „nexus“ (figlet, Schrift „standard“)
const LOGO = [
  String.raw` _ __   _____  ___   _ ___ `,
  String.raw`| '_ \ / _ \ \/ / | | / __|`,
  String.raw`| | | |  __/>  <| |_| \__ \ `,
  String.raw`|_| |_|\___/_/\_\\__,_|___/`,
];

const percent = (value: number | null) => (value == null ? '–' : `${Math.round(value)} %`);

const fetch: Command = {
  name: 'fetch',
  aliases: ['neofetch'],
  usage: 'fetch',
  summary: 'Überblick: Board, offene Zähler, Claude-Nutzung',
  examples: ['fetch'],
  run: (_args, actions) => {
    const active = actions.boards().find((b) => b.active);
    const counts = actions.sections().filter((s) => s.badge).map((s) => `${s.id} ${s.badge}`);
    const usage = actions.usage();
    const info: [string, string][] = [
      ['board', active ? `${slug(active.name)} · ${active.project}` : 'keins'],
      ['pfad', `~/${actions.path()}`],
      ['offen', counts.length ? counts.join(' · ') : 'nichts offen'],
      ['claude', usage ? `5h ${percent(usage.fiveHour)} · 7d ${percent(usage.sevenDay)}` : 'aus'],
      ['theme', actions.theme() ?? 'system'],
      ['shell', 'nexus-terminal 2.0.0'],
    ];
    const width = Math.max(...LOGO.map((l) => l.length));
    const rows = Array.from({ length: Math.max(LOGO.length, info.length) }, (_, i) => {
      const [key, value] = info[i] ?? ['', ''];
      return [text((LOGO[i] ?? '').padEnd(width), 'ok'), text(key, 'dir'), text(value)];
    });
    return [table(rows)];
  },
};

const history: Command = {
  name: 'history',
  usage: 'history',
  summary: 'bisherige Befehle (↑↓ blättert)',
  run: (_args, actions) => {
    const entries = actions.history();
    if (!entries.length) return [line(text('Noch keine Befehle.', 'muted'))];
    return [table(entries.map((entry, i) => [text(String(i + 1).padStart(4), 'muted'), text(entry)]))];
  },
};

// Kuh von cowsay; die Sprechblase davor ist so breit wie der Text
const COW = [
  String.raw`        \   ^__^`,
  String.raw`         \  (oo)\_______`,
  String.raw`            (__)\       )\/\ `,
  String.raw`                ||----w |`,
  String.raw`                ||     ||`,
];

const cowsay: Command = {
  name: 'cowsay',
  usage: 'cowsay [text]',
  summary: 'eine Kuh sagt etwas',
  examples: ['cowsay Muh!'],
  run: (args) => {
    const said = args.join(' ') || 'Muh.';
    const bubble = [` ${'_'.repeat(said.length + 2)}`, `< ${said} >`, ` ${'-'.repeat(said.length + 2)}`];
    return [...bubble, ...COW].map((l) => line(text(l)));
  },
};

const sudo: Command = {
  name: 'sudo',
  usage: 'sudo',
  summary: 'Root-Rechte',
  hidden: true,
  run: () => [line(text('Nice try.', 'error')), line(text('Hier gibt es keine Root-Rechte – nur Tickets.', 'muted'))],
};

const COMMANDS: Command[] = [help, man, cd, ls, open, find, board, theme, top, fetch, history, cowsay, clear, sudo];

// Befehl zu einem getippten Namen oder Zweitnamen
function commandNamed(name: string): Command | undefined {
  const wanted = name.toLowerCase();
  return COMMANDS.find((c) => c.name === wanted || c.aliases?.includes(wanted));
}

// Führt eine Eingabe aus; null schließt die Ausgabe, sonst die Zeilen der Ausgabe
export async function runCommand(input: string, actions: TerminalActions): Promise<OutputBlock[] | null> {
  const [name, ...args] = parseLine(input);
  if (!name) return null;
  if (TICKET_ID.test(name) && args.length === 0) return open.run([name], actions);
  const command = commandNamed(name);
  if (!command) return fail(`command not found: ${name}`, 'help zeigt alle Befehle');
  try {
    return await command.run(args, actions);
  } catch (error) {
    return fail(`${command.name}: ${error instanceof Error ? error.message : String(error)}`);
  }
}

// Vollständige Eingaben, die zum Getippten passen: erst Befehle, nach dem Befehl dessen Argumente
export function completions(value: string, data: CompletionData): string[] {
  const match = /^(\S*)(\s+)(.*)$/.exec(value);
  if (!match) {
    const prefix = value.toLowerCase();
    return COMMANDS.filter((c) => !c.hidden).flatMap((c) => [c.name, ...(c.aliases ?? [])]).filter((c) => c.startsWith(prefix));
  }
  const [, name, gap, rest] = match as unknown as [string, string, string, string];
  const pool = commandNamed(name)?.pool?.(data) ?? [];
  const prefix = rest.toLowerCase();
  return [...new Set(pool)].filter((c) => c.toLowerCase().startsWith(prefix)).map((c) => `${name}${gap}${c}`);
}

// Längster gemeinsamer Anfang mehrerer Vorschläge (ohne Groß-/Kleinschreibung), in der Schreibweise des ersten
export function commonPrefix(values: string[]): string {
  if (!values.length) return '';
  return values.reduce((common, value) => {
    let i = 0;
    while (i < common.length && common[i]!.toLowerCase() === value[i]?.toLowerCase()) i++;
    return common.slice(0, i);
  });
}
