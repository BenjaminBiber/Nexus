import { describe, expect, it } from 'vitest';
import { HelpScenarios } from '../lib/gen-help-scenarios.mjs';

// Hilfe wie commander sie ausgibt: Überschrift, Einträge mit zwei Leerzeichen, Fortsetzungszeilen weiter rechts
const ROOT_HELP = `Aufruf: harness [options] [command]

Optionen:
  --json             Ausgabe als JSON

Befehle:
  init               Richtet den Harness ein (einmal pro Projekt,
                     wiederholbar).
  board [options]    Ohne Unterbefehl: öffnet das Board.
  hook <ereignis>    Wird von Claude Code aufgerufen.
  help [befehl]      Hilfe zu einem Befehl anzeigen`;

const BOARD_HELP = `Aufruf: harness board [options] [command]

Befehle:
  list [options]     Listet Tickets je Spalte.
  note               Notizen.
  help [befehl]      Hilfe zu einem Befehl anzeigen

Beispiele:
  harness board`;

const NOTE_HELP = `Aufruf: harness board note [options] [command]

Befehle:
  add [options] <titel>  Hält einen Befund fest.
  rm|remove <id>         Verwirft eine Notiz.`;

const LEAF_HELP = `Aufruf: harness board list [options]

Optionen:
  -h, --help  Hilfe anzeigen`;

const HELP_BY_PATH = {
  '': ROOT_HELP,
  init: LEAF_HELP,
  board: BOARD_HELP,
  'board list': LEAF_HELP,
  'board note': NOTE_HELP,
  'board note add': LEAF_HELP,
  'board note rm': LEAF_HELP,
  hook: LEAF_HELP,
};

// Generator mit festen Hilfetexten statt echter Aufrufe
class FakeHelp extends HelpScenarios {
  help(path) {
    const text = HELP_BY_PATH[path.join(' ')];
    if (text === undefined) throw new Error(`kein Hilfetext für „${path.join(' ')}“`);
    return text;
  }
}

describe('HelpScenarios', () => {
  const generator = new FakeHelp(['node', 'main.js']);

  describe('subcommands: Befehle aus dem Abschnitt „Befehle“', () => {
    it('liest die Namen und übergeht Fortsetzungszeilen', () => {
      expect(generator.subcommands([])).toEqual(['init', 'board', 'hook']);
    });

    it('lässt den eingebauten Befehl help weg – er zeigt die Hilfe des übergeordneten Befehls', () => {
      expect(generator.subcommands([])).not.toContain('help');
    });

    it('endet am Abschnitt der Beispiele', () => {
      expect(generator.subcommands(['board'])).toEqual(['list', 'note']);
    });

    it('schneidet Argumente und Aliasse ab', () => {
      expect(generator.subcommands(['board', 'note'])).toEqual(['add', 'rm']);
    });

    it('liefert für einen Befehl ohne Unterbefehle eine leere Liste', () => {
      expect(generator.subcommands(['board', 'list'])).toEqual([]);
    });

    it('liest auch die englische Überschrift', () => {
      class English extends HelpScenarios {
        help() {
          return 'Usage: harness\n\nCommands:\n  init  Richtet ein.';
        }
      }
      expect(new English(['node']).subcommands([])).toEqual(['init']);
    });
  });

  describe('paths: alle Befehlspfade', () => {
    it('beginnt mit dem Programm selbst und steigt in die Tiefe', () => {
      expect(generator.paths()).toEqual([
        [],
        ['init'],
        ['board'],
        ['board', 'list'],
        ['board', 'note'],
        ['board', 'note', 'add'],
        ['board', 'note', 'rm'],
        ['hook'],
      ]);
    });
  });

  describe('program: Name aus der Aufruf-Zeile', () => {
    it('nimmt den Namen, mit dem sich das geprüfte Programm selbst meldet', () => {
      expect(generator.program()).toBe('harness');
    });

    it('nimmt auch den neuen Namen, wenn das Rust-Binary geprüft wird', () => {
      class Rust extends HelpScenarios {
        help() {
          return 'Aufruf: nexus [options] [command]';
        }
      }
      expect(new Rust(['target/debug/nexus']).program()).toBe('nexus');
    });

    it('fällt ohne Aufruf-Zeile auf nexus zurück', () => {
      class Ohne extends HelpScenarios {
        help() {
          return 'Optionen:\n  -h, --help  Hilfe anzeigen';
        }
      }
      expect(new Ohne(['target/debug/nexus']).program()).toBe('nexus');
    });
  });

  describe('scenario: das fertige Szenario', () => {
    const scenario = generator.scenario();

    it('macht je Befehl einen Schritt mit --help als letztem Argument', () => {
      expect(scenario.steps[0]).toEqual({ name: 'harness', kind: 'cli', args: ['--help'] });
      expect(scenario.steps[4]).toEqual({ name: 'harness board note', kind: 'cli', args: ['board', 'note', '--help'] });
    });

    it('hängt die Fehlerfälle hinten an', () => {
      const names = scenario.steps.slice(8).map((step) => step.name);
      expect(names).toContain('unbekannter Befehl');
      expect(names).toContain('fehlendes Pflichtargument');
      expect(names).toContain('Hilfe über den Befehl help');
    });

    it('gibt jedem Schritt einen eindeutigen Namen (der Lader verlangt das)', () => {
      const names = scenario.steps.map((step) => step.name);
      expect(new Set(names).size).toBe(names.length);
    });

    it('nennt einen Titel', () => {
      expect(scenario.title).toMatch(/Hilfetexte/);
    });
  });
});
