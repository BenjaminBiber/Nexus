import { describe, expect, it, vi } from 'vitest';
import {
  boardKey, commonPrefix, completions, parentPath, parseLine, runCommand, type OutputBlock, type TerminalActions, type TerminalBoard,
} from '../src/terminalCommands.js';
import { pathUnderProject } from '../src/useTerminalActions.js';

const BOARDS: TerminalBoard[] = [
  { id: 1, name: 'Sprint', project: 'Arches', projectKey: 'ARC', active: true },
  { id: 2, name: 'Nächster Schritt', project: 'Arches', projectKey: 'ARC', active: false },
  { id: 3, name: 'Sprint', project: 'Other', projectKey: 'OTH', active: false },
];

// Aktionen mit Aufzeichnung; path ist der Pfad unter /p/:pid
function fakeActions(path = 'board', theme = 'terminal') {
  const actions = {
    path: () => path,
    sections: () => [
      { id: 'board' as const, label: 'Board' },
      { id: 'inbox' as const, label: 'Meine Aufgaben', badge: 3 },
      { id: 'sessions' as const, label: 'Sessions' },
      { id: 'settings' as const, label: 'Einstellungen' },
    ],
    go: vi.fn(),
    openTicket: vi.fn(),
    search: vi.fn(),
    boards: vi.fn(() => BOARDS),
    switchBoard: vi.fn(),
    theme: () => theme,
    setTheme: vi.fn(async () => undefined),
  } satisfies TerminalActions;
  return actions;
}

// Ausgabe als Text, Zeile für Zeile (Tabellenspalten mit „|“ getrennt)
function plain(blocks: OutputBlock[] | null): string[] {
  return (blocks ?? []).flatMap((b) => (b.kind === 'line' ? [b.cells.map((c) => c.text).join('')] : b.rows.map((r) => r.map((c) => c.text).join('|'))));
}

describe('Terminal-Befehle', () => {
  it('zerlegt Eingaben, Text in Anführungszeichen bleibt ein Wort', () => {
    expect(parseLine('new "Export bauen"  jetzt')).toEqual(['new', 'Export bauen', 'jetzt']);
    expect(parseLine("grep 'a b'")).toEqual(['grep', 'a b']);
    expect(parseLine('   ')).toEqual([]);
  });

  it('wechselt mit cd den Bereich, auch über deutsche Namen und Anfänge', async () => {
    const actions = fakeActions('board/t/ARC-0001');
    expect(await runCommand('cd sess', actions)).toBeNull();
    expect(actions.go).toHaveBeenLastCalledWith('sessions');
    await runCommand('cd einstellungen', actions);
    expect(actions.go).toHaveBeenLastCalledWith('settings');
    await runCommand('cd ~/fragen/', actions);
    expect(actions.go).toHaveBeenLastCalledWith('questions');
    await runCommand('cd ..', actions);
    expect(actions.go).toHaveBeenLastCalledWith('board');
    await runCommand('cd', actions);
    expect(actions.go).toHaveBeenLastCalledWith('board');
  });

  it('geht mit cd .. eine Stufe nach oben, vom Ticket aufs Board', () => {
    expect(parentPath('board/t/ARC-0001')).toBe('board');
    expect(parentPath('docs/handbuch/start')).toBe('docs/handbuch');
    expect(parentPath('sessions')).toBe('board');
  });

  it('meldet unbekannte Bereiche und Befehle, ohne etwas auszulösen', async () => {
    const actions = fakeActions();
    expect(plain(await runCommand('cd keller', actions))[0]).toBe('cd: kein solcher Bereich: keller');
    expect(plain(await runCommand('rm -rf /', actions))).toEqual(['command not found: rm', 'help zeigt alle Befehle']);
    expect(actions.go).not.toHaveBeenCalled();
  });

  it('öffnet Tickets über open oder die ID allein, in Großbuchstaben', async () => {
    const actions = fakeActions();
    expect(await runCommand('arc-0002', actions)).toBeNull();
    expect(actions.openTicket).toHaveBeenLastCalledWith('ARC-0002');
    await runCommand('open ARC-0001', actions);
    expect(actions.openTicket).toHaveBeenLastCalledWith('ARC-0001');
    expect(plain(await runCommand('open hallo', actions))).toEqual(['open: keine Ticket-ID: hallo']);
    expect(plain(await runCommand('open', actions))).toEqual(['open: Ticket-ID fehlt, z. B. open ARC-0042']);
  });

  it('listet mit ls die Bereiche der Navigation samt Zählern', async () => {
    expect(plain(await runCommand('ls', fakeActions()))).toEqual([
      'board/|Board',
      'inbox/|Meine Aufgaben|[3]',
      'sessions/|Sessions',
      'settings/|Einstellungen',
    ]);
  });

  it('öffnet mit find die globale Suche', async () => {
    const actions = fakeActions();
    expect(await runCommand('find versand', actions)).toBeNull();
    expect(actions.search).toHaveBeenCalledTimes(1);
  });

  it('zeigt und wechselt Boards; gleiche Namen bekommen das Projekt davor', async () => {
    const actions = fakeActions();
    expect(plain(await runCommand('board', actions))).toEqual(['* arches/sprint|Arches', '  nächster-schritt|Arches', '  other/sprint|Other']);
    expect(plain(await runCommand('board nächster', actions))).toEqual(['board → nächster-schritt']);
    expect(actions.switchBoard).toHaveBeenCalledWith(BOARDS[1]);
    expect(plain(await runCommand('board keins', actions))[0]).toBe('board: unbekannt: keins');
    expect(boardKey(BOARDS[1]!, BOARDS)).toBe('nächster-schritt');
  });

  it('zeigt und wechselt das Farbschema, auch mit deutschen Namen und installierten IDs', async () => {
    const actions = fakeActions();
    expect(plain(await runCommand('theme', actions))).toContain('* terminal');
    expect(plain(await runCommand('theme dunkel', actions))).toEqual(['Farbschema → dark']);
    expect(actions.setTheme).toHaveBeenLastCalledWith('dark');
    expect(plain(await runCommand('theme nacht-blau', actions))).toEqual(['Farbschema → nacht-blau']);
    expect(actions.setTheme).toHaveBeenLastCalledWith('nacht-blau');
    expect(plain(await runCommand('theme Ä!', actions))[0]).toBe('theme: unbekannt: Ä!');
  });

  it('meldet Fehler der Oberfläche als Ausgabe statt abzubrechen', async () => {
    const actions = { ...fakeActions(), setTheme: vi.fn(async () => Promise.reject(new Error('theme muss ein eingebautes oder installiertes Theme sein.'))) };
    expect(plain(await runCommand('theme xy', actions))).toEqual(['theme: theme muss ein eingebautes oder installiertes Theme sein.']);
  });

  it('zeigt mit help alle Befehle', async () => {
    const names = plain(await runCommand('help', fakeActions())).map((row) => row.split(/[ |]/)[0]);
    expect(names).toEqual(['help', 'cd', 'ls', 'open', 'find', 'board', 'theme', 'clear']);
  });

  it('ergänzt Befehle und Argumente des Befehls', () => {
    const data = { boards: ['arches/sprint', 'nächster-schritt'] };
    expect(completions('he', data)).toEqual(['help']);
    expect(completions('cd se', data)).toEqual(['cd sessions', 'cd settings']);
    expect(completions('board n', data)).toEqual(['board nächster-schritt']);
    expect(completions('theme cat', data)).toEqual(['theme catppuccin']);
    expect(completions('find x', data)).toEqual([]);
    expect(commonPrefix(['cd sessions', 'cd settings'])).toBe('cd se');
  });

  it('liest den Pfad unter dem Projekt aus der Adresse', () => {
    expect(pathUnderProject('/p/p1/board/t/ARC-0001', 'p1')).toBe('board/t/ARC-0001');
    expect(pathUnderProject('/p/mein%20projekt/docs/', 'mein projekt')).toBe('docs');
    expect(pathUnderProject('/anderswo', 'p1')).toBe('');
  });
});
