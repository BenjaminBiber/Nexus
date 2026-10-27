import { describe, it, expect, afterAll } from 'vitest';
import { readdirSync, readFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { TempProject } from './helpers/TempProject.js';

// Alle Befehle mit Hilfe: Pfad im Befehlsbaum
const LEAVES: string[][] = [
  ['init'],
  ['board'],
  ['board', 'project', 'list'],
  ['board', 'project', 'create'],
  ['board', 'project', 'rename'],
  ['board', 'boards', 'list'],
  ['board', 'boards', 'create'],
  ['board', 'boards', 'rename'],
  ['board', 'boards', 'use'],
  ['board', 'boards', 'repo'],
  ['board', 'boards', 'delete'],
  ['board', 'list'],
  ['board', 'show'],
  ['board', 'create'],
  ['board', 'set'],
  ['board', 'move'],
  ['board', 'delete'],
  ['board', 'claim'],
  ['board', 'release'],
  ['board', 'comment'],
  ['board', 'start'],
  ['board', 'done'],
  ['board', 'step'],
  ['board', 'step', 'done'],
  ['board', 'step', 'add'],
  ['board', 'step', 'edit'],
  ['board', 'step', 'rm'],
  ['board', 'step', 'move'],
  ['board', 'note', 'add'],
  ['board', 'note', 'list'],
  ['board', 'note', 'show'],
  ['board', 'note', 'take'],
  ['board', 'note', 'drop'],
  ['board', 'question', 'add'],
  ['board', 'question', 'list'],
  ['board', 'question', 'show'],
  ['board', 'question', 'research'],
  ['board', 'question', 'answer'],
  ['board', 'question', 'discard'],
  ['board', 'question', 'reopen'],
  ['board', 'question', 'link'],
  ['board', 'question', 'depends'],
  ['board', 'question', 'comment'],
  ['board', 'dispatch', 'status'],
  ['board', 'dispatch', 'run'],
  ['board', 'dispatch', 'pause'],
  ['board', 'dispatch', 'resume'],
  ['board', 'dispatch', 'enable'],
  ['board', 'dispatch', 'disable'],
  ['board', 'merge'],
  ['board', 'accept'],
  ['board', 'plan'],
  ['board', 'plan-json'],
  ['board', 'next-planning'],
  ['install'],
  ['stop'],
  ['hook'],
  ['spec', 'comments'],
  ['spec', 'resolve'],
];

describe('Hilfe aller Befehle (SC-006)', () => {
  const project = new TempProject();
  afterAll(() => project.dispose());

  it.each(LEAVES)('%s: deutsche Hilfe mit Zweck, Parametern und Beispiel', (...path) => {
    const result = project.run([...path, '--help']);
    expect(result.code).toBe(0);
    // Eigener Aufrufkopf (ein unbekannter Befehl zeigt nur die Hilfe des Elternbefehls)
    expect(result.stdout).toContain(`Aufruf: harness ${path.join(' ')} `);
    expect(result.stdout).toContain('Beispiel');
    expect(result.stdout).not.toMatch(/Usage:|Options:|display help/);
    expect(result.stdout).toContain('Hilfe anzeigen');
  });

  it('die Hauptseite listet Befehle auf Deutsch', () => {
    const result = project.run(['--help']);
    expect(result.stdout).toContain('Befehle:');
    expect(result.stdout).toContain('Optionen:');
  });

  it('Fehlermeldungen von commander sind deutsch', () => {
    const unknown = project.run(['gibtesnicht']);
    expect(unknown.code).toBe(1);
    expect(unknown.stderr).toContain('Fehler: Unbekannter Befehl');
    const missing = project.run(['spec', 'resolve', 'K-1a2b3c4d']);
    expect(missing.code).toBe(1);
    expect(missing.stderr).toContain("Fehler: Das Pflichtargument 'antwort' fehlt.");
  });

  it('Gruppen zeigen ihre Unterbefehle', () => {
    expect(project.run(['board', '--help']).stdout).toContain('start');
    expect(project.run(['spec', '--help']).stdout).toContain('comments');
  });

  it('kennt die entfallenen Befehle nicht mehr', () => {
    for (const name of ['ticket', 'question', 'export']) {
      const result = project.run([name]);
      expect(result.code, name).toBe(1);
      expect(result.stderr, name).toContain('Unbekannter Befehl');
    }
  });

  it('kennt die entfallenen Spec-Kit-Befehle nicht mehr', () => {
    for (const name of ['new', 'import']) {
      const result = project.run(['spec', name]);
      expect(result.code, name).toBe(1);
      expect(result.stderr, name).toContain('Unbekannter Befehl');
    }
  });
});

describe('Schreiben nur über die Kernbibliothek (FR-015)', () => {
  it('das CLI-Paket importiert kein Dateisystem-Modul', () => {
    const files = collect(resolve(__dirname, '../../src'));
    expect(files.length).toBeGreaterThan(5);
    for (const file of files) {
      expect(readFileSync(file, 'utf8'), file).not.toMatch(/from 'node:fs|from "node:fs|from 'fs'|from "fs"/);
    }
  });
});

// Sammelt alle .ts-Dateien eines Ordners rekursiv
function collect(dir: string): string[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap((e) =>
    e.isDirectory() ? collect(join(dir, e.name)) : e.name.endsWith('.ts') ? [join(dir, e.name)] : [],
  );
}
