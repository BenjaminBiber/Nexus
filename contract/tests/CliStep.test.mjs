import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { CliStep } from '../lib/CliStep.mjs';
import { Variables } from '../lib/Variables.mjs';

// Umgebung eines Szenarios, wie der ScenarioRunner sie an den Schritt gibt
const context = (extra = {}) => ({
  variables: new Variables({ REPO1: '/tmp/repo' }),
  cli: ['node', '/tmp/main.js'],
  home: '/tmp/home',
  claudeConfig: '/tmp/claude',
  port: 4711,
  envFile: null,
  ...extra,
});

describe('CliStep', () => {
  describe('command: Aufteilung von --cli-cmd', () => {
    it('trennt am Leerzeichen', () => {
      expect(CliStep.command('node packages/cli/dist/main.js')).toEqual(['node', 'packages/cli/dist/main.js']);
    });

    it('hält Anführungszeichen zusammen', () => {
      expect(CliStep.command('node "C:/Mein Ordner/main.js"')).toEqual(['node', 'C:/Mein Ordner/main.js']);
    });

    it('liest ein JSON-Array – der sichere Weg für Pfade mit Leerzeichen', () => {
      expect(CliStep.command('["node","/Mein Ordner/main.js"]')).toEqual(['node', '/Mein Ordner/main.js']);
    });

    it('meldet ein kaputtes JSON-Array', () => {
      expect(() => CliStep.command('["node",')).toThrow(/kein gültiges JSON-Array/);
    });

    it('meldet ein leeres Array und Einträge, die kein Text sind', () => {
      expect(() => CliStep.command('[]')).toThrow(/nicht leere Liste aus Texten/);
      expect(() => CliStep.command('["node",7]')).toThrow(/nicht leere Liste aus Texten/);
    });

    it('meldet einen leeren Befehl', () => {
      expect(() => CliStep.command('   ')).toThrow(/--cli-cmd ist leer/);
    });
  });

  describe('env: Umgebung des Aufrufs', () => {
    it('setzt Home, Port, Claude-Konfiguration und feste Ausgabe', () => {
      const env = CliStep.env(context(), {});
      expect(env).toMatchObject({
        HARNESS_HOME: '/tmp/home',
        HARNESS_PORT: '4711',
        CLAUDE_CONFIG_DIR: '/tmp/claude',
        NO_COLOR: '1',
        LANG: 'C.UTF-8',
      });
    });

    it('nimmt eine geerbte Session-, Ticket- und Umgebungsdatei-Zuordnung heraus', () => {
      const before = { ...process.env };
      process.env.HARNESS_SESSION = 's-geerbt';
      process.env.HARNESS_TICKET = 'ARC-9999';
      process.env.CLAUDE_ENV_FILE = '/tmp/geerbt.sh';
      try {
        const env = CliStep.env(context(), {});
        expect(env.HARNESS_SESSION).toBeUndefined();
        expect(env.HARNESS_TICKET).toBeUndefined();
        expect(env.CLAUDE_ENV_FILE).toBeUndefined();
      } finally {
        process.env = before;
      }
    });

    it('setzt CLAUDE_ENV_FILE nur mit der Fixture claudeEnvFile', () => {
      expect(CliStep.env(context({ envFile: '/tmp/claude/claude-env.sh' }), {}).CLAUDE_ENV_FILE).toBe('/tmp/claude/claude-env.sh');
    });

    it('lässt die Variablen des Schritts zuletzt gewinnen', () => {
      const env = CliStep.env(context(), { HARNESS_TICKET: 'ARC-0001', NO_COLOR: '0' });
      expect(env.HARNESS_TICKET).toBe('ARC-0001');
      expect(env.NO_COLOR).toBe('0');
    });
  });

  describe('input: Standardeingabe', () => {
    it('schickt ein Objekt als JSON', () => {
      expect(CliStep.input({ session_id: 's1' })).toBe('{"session_id":"s1"}');
    });

    it('schickt einen Text roh – auch kaputtes JSON', () => {
      expect(CliStep.input('{kaputt')).toBe('{kaputt');
    });
  });

  describe('clean: Normalisierung der Ausgabe', () => {
    it('macht \\r\\n zu \\n', () => {
      expect(CliStep.clean('eins\r\nzwei\r\n')).toBe('eins\nzwei');
    });

    it('lässt abschließende Leerzeilen weg, innere bleiben', () => {
      expect(CliStep.clean('eins\n\nzwei\n\n\n')).toBe('eins\n\nzwei');
    });

    it('lässt einen leeren Text leer', () => {
      expect(CliStep.clean('\n\n')).toBe('');
    });
  });

  describe('stdout: Text oder JSON', () => {
    it('zeichnet ein JSON-Objekt als json auf', () => {
      expect(CliStep.stdout('{"hookSpecificOutput":{"hookEventName":"SessionStart"}}\n'))
        .toEqual({ json: { hookSpecificOutput: { hookEventName: 'SessionStart' } } });
    });

    it('zeichnet eine JSON-Liste als json auf', () => {
      expect(CliStep.stdout('[1,2]')).toEqual({ json: [1, 2] });
    });

    it('zeichnet Text als stdout auf', () => {
      expect(CliStep.stdout('Aufruf: harness\r\n')).toEqual({ stdout: 'Aufruf: harness' });
    });

    it('zeichnet eine einzelne Zahl als Text auf – nur Objekt und Liste gelten als JSON', () => {
      expect(CliStep.stdout('42')).toEqual({ stdout: '42' });
    });

    it('zeichnet leere Ausgabe als leeren Text auf', () => {
      expect(CliStep.stdout('')).toEqual({ stdout: '' });
    });
  });

  describe('envFileContent: Umgebungsdatei nach dem Schritt', () => {
    let dir;

    beforeEach(() => {
      dir = mkdtempSync(join(tmpdir(), 'harness-contract-clistep-'));
    });

    afterEach(() => {
      rmSync(dir, { recursive: true, force: true });
    });

    it('liefert leeren Text, solange kein Hook geschrieben hat', () => {
      expect(CliStep.envFileContent(join(dir, 'gibtsnicht.sh'))).toBe('');
    });

    it('behält den abschließenden Zeilenumbruch und vereinheitlicht die Zeilenenden', () => {
      const file = join(dir, 'claude-env.sh');
      writeFileSync(file, "export HARNESS_SESSION='s1'\r\n");
      expect(CliStep.envFileContent(file)).toBe("export HARNESS_SESSION='s1'\n");
    });
  });

  describe('run: ein echter Aufruf', () => {
    it('zeichnet Argumente, Arbeitsordner, Eingabe und Ergebnis auf', async () => {
      const dir = mkdtempSync(join(tmpdir(), 'harness-contract-clirun-'));
      try {
        const script = join(dir, 'echo.mjs');
        writeFileSync(script, [
          'let input = "";',
          'process.stdin.on("data", (chunk) => { input += chunk; });',
          'process.stdin.on("end", () => {',
          '  process.stdout.write(JSON.stringify({ eingabe: JSON.parse(input), ticket: process.env.HARNESS_TICKET, port: process.env.HARNESS_PORT }));',
          '  process.stderr.write("ein Hinweis\\r\\n");',
          '  process.exit(3);',
          '});',
        ].join('\n'));
        const step = { args: ['--still'], cwd: dir, stdin: { session_id: 's1' }, env: { HARNESS_TICKET: 'ARC-0001' } };
        const entry = await new CliStep().run(step, context({ cli: ['node', script], home: dir }));
        expect(entry.request).toEqual({ args: ['--still'], cwd: dir, stdin: { session_id: 's1' }, env: { HARNESS_TICKET: 'ARC-0001' } });
        expect(entry.response).toEqual({
          code: 3,
          json: { eingabe: { session_id: 's1' }, ticket: 'ARC-0001', port: '4711' },
          stderr: 'ein Hinweis',
        });
      } finally {
        rmSync(dir, { recursive: true, force: true });
      }
    });

    it('läuft ohne cwd im Home des Szenarios und zeichnet ohne env kein env auf', async () => {
      const dir = mkdtempSync(join(tmpdir(), 'harness-contract-clihome-'));
      try {
        const script = join(dir, 'cwd.mjs');
        writeFileSync(script, 'process.stdout.write(process.cwd());');
        const entry = await new CliStep().run({ args: ['x'] }, context({ cli: ['node', script], home: dir }));
        expect(entry.request).toEqual({ args: ['x'], cwd: dir });
        expect(entry.response.code).toBe(0);
      } finally {
        rmSync(dir, { recursive: true, force: true });
      }
    });

    it('meldet ein Programm, das sich nicht starten lässt', async () => {
      const step = { args: ['x'] };
      await expect(new CliStep().run(step, context({ cli: ['gibt-es-nicht-harness-vertrag'] })))
        .rejects.toThrow(/lässt sich nicht starten/);
    });
  });
});
