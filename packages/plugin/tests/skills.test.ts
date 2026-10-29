import { describe, it, expect, beforeAll } from 'vitest';
import { spawnSync } from 'node:child_process';
import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { parse } from 'yaml';
import { buildPlugin, PLUGIN_BIN, ROOT } from './helpers/pluginBuild.js';

const SKILLS_SRC = join(ROOT, 'packages/plugin/manifest/skills');
const EXPECTED = ['board', 'board-question', 'harness-customize', 'harness-grill', 'harness-implement', 'harness-setup', 'harness-tasks', 'harness-wayfinder'];
// Skills ohne nexus-Aufrufe (arbeiten mit npm im Clone)
const NPM_ONLY = ['harness-customize'];

interface Skill {
  dir: string;
  name: string;
  description: string;
  body: string;
  text: string;
}

// Liest eine SKILL.md und trennt den Kopf (name, description) vom Text
function load(dir: string): Skill {
  const text = readFileSync(join(SKILLS_SRC, dir, 'SKILL.md'), 'utf8').replace(/\r\n/g, '\n');
  const match = /^---\n([\s\S]*?)\n---\n([\s\S]*)$/.exec(text);
  if (!match) throw new Error(`${dir}: Kopf fehlt`);
  const field = (key: string) => new RegExp(`^${key}:\\s*(.+)$`, 'm').exec(match[1])?.[1].replace(/^["']|["']$/g, '') ?? '';
  return { dir, name: field('name'), description: field('description'), body: match[2], text };
}

// Führt das Binary des gebauten Plugins aus
function cli(args: string[]): { code: number; out: string } {
  const r = spawnSync(PLUGIN_BIN, args, { encoding: 'utf8', windowsHide: true });
  return { code: r.status ?? -1, out: `${r.stdout}${r.stderr}` };
}

// Hilfeausgaben je Unterbefehl, damit jeder Prozess nur einmal startet
const helpCache = new Map<string, { code: number; out: string }>();

// Führt `<pfad> --help` aus dem Cache oder startet das CLI einmalig
function helpOf(path: string[]): { code: number; out: string } {
  const key = path.join(' ');
  if (!helpCache.has(key)) helpCache.set(key, cli([...path, '--help']));
  return helpCache.get(key)!;
}

// Alle `nexus …`-Aufrufe in Code-Spannen und Codeblöcken
function nexusCalls(text: string): string[] {
  const calls: string[] = [];
  for (const m of text.matchAll(/`(nexus [^`\n]+)`/g)) calls.push(m[1]);
  for (const block of text.matchAll(/```[a-z]*\n([\s\S]*?)```/g)) {
    for (const line of block[1].split('\n')) if (/^\s*(\$ )?nexus /.test(line)) calls.push(line.replace(/^\s*\$? ?/, '').trim());
  }
  return calls;
}

describe('Skills des Plugins', () => {
  let skills: Skill[];

  beforeAll(() => {
    expect(existsSync(SKILLS_SRC), 'Ordner skills fehlt').toBe(true);
    skills = readdirSync(SKILLS_SRC).map(load);
    const built = buildPlugin();
    expect(built.ok, built.message).toBe(true);
  }, 120000);

  it('enthält genau die erwarteten Skills', () => {
    expect(skills.map((s) => s.dir).sort()).toEqual(EXPECTED);
  });

  it('hat gültige Köpfe: Name gleich Ordner, deutsche Beschreibung', () => {
    for (const s of skills) {
      expect(s.name, s.dir).toBe(s.dir);
      expect(s.name).toMatch(/^[a-z0-9-]+$/);
      expect(s.description.length, s.dir).toBeGreaterThan(30);
      expect(s.description.length, s.dir).toBeLessThanOrEqual(1024);
      expect(s.description, s.dir).toMatch(/Ticket|Anforderungen|Umsetz|Terminal|Anpass/);
      // Der Kopf muss gültiges YAML sein, sonst liest der Harness die Beschreibung nicht
      const head = /^---\n([\s\S]*?)\n---\n/.exec(s.text)?.[1] ?? '';
      expect(() => parse(head), s.dir).not.toThrow();
      expect(parse(head).description, s.dir).toBe(s.description);
    }
  });

  it('nennt nur existierende nexus-Befehle und Optionen', () => {
    const root = helpOf([]).out;
    for (const s of skills) {
      const calls = nexusCalls(s.text);
      if (!NPM_ONLY.includes(s.dir)) expect(calls.length, s.dir).toBeGreaterThan(2);
      for (const call of calls) {
        // Die globale Option --json darf vor dem Unterbefehl stehen
        const words = call.replace(/--json\b/g, ' ').split(/\s+/).filter(Boolean).slice(1);
        const path: string[] = [];
        for (const w of words) {
          if (/^[a-z][a-z-]*$/.test(w) && path.length < 3) path.push(w);
          else break;
        }
        const help = helpOf(path);
        expect(help.code, `${s.dir}: ${call}`).toBe(0);
        expect(help.out, `${s.dir}: ${call}`).toContain('Aufruf:');
        // Unbekannte Unterbefehle zeigen die Hilfe des Elternbefehls: der Kopf muss zum genauen Pfad passen
        if (path.length > 0) expect(help.out, `${s.dir}: ${call} (Hilfe gehört nicht zu diesem Befehl)`).toContain(`Aufruf: nexus ${path.join(' ')} `);
        for (const flag of call.match(/--[a-z-]+/g) ?? []) {
          expect(help.out.includes(flag) || root.includes(flag), `${s.dir}: ${call} → ${flag}`).toBe(true);
        }
      }
    }
  });

  it('verbietet das Ändern von Ticketdateien von Hand und das Anlegen von Worktrees', () => {
    for (const s of skills) {
      expect(s.text, s.dir).toMatch(/nie von Hand/);
      expect(s.text, s.dir).not.toMatch(/git worktree add/);
      for (const line of s.text.split('\n').filter((l) => l.includes('using-git-worktrees'))) {
        expect(line, s.dir).toMatch(/nicht|nie|ausgeschlossen/);
      }
    }
  });

  it('ist eigenständig: ruft keine anderen Skills auf', () => {
    for (const s of skills) {
      expect(s.text, s.dir).not.toMatch(/Skill-Werkzeug|Skill tool|Skill\(/i);
    }
  });

  describe('harness-customize', () => {
    const get = () => skills.find((s) => s.dir === 'harness-customize')!;

    // Findet den Clone über overview.json und ruft nexus customize im Clone auf
    it('findet den Clone über overview.json und nutzt nexus customize im Clone', () => {
      const t = get().text;
      for (const part of ['overview.json', 'cloneDir', 'nexus customize --clone', 'check', 'copy workflow', '--rebase', 'run build:plugin', 'bin/nexus install', 'planen', 'umsetzen']) {
        expect(t, part).toContain(part);
      }
    });

    // Ohne eingerichteten Harness wird nichts geändert, Zurücksetzen wird bestätigt
    it('ändert nichts ohne eingerichteten Harness und bestätigt das Zurücksetzen', () => {
      const t = get().text;
      expect(t).toMatch(/Ändere dann nichts/);
      expect(t).toMatch(/bestätigen lassen/);
    });

    // Das Anwenden baut alles neu und braucht mehr als die üblichen zwei Minuten: Zeitgrenze oder Hintergrund, dann auf das gebaute Plugin warten
    it('nennt bei jedem Aufruf des Plugin-Baus die Zeitgrenze, den Hintergrund und das Warten auf das Ergebnis', () => {
      const t = get().text;
      expect(t).toContain('600000');
      expect(t).toMatch(/im Hintergrund/);
      expect(t).toContain('Plugin gebaut:');
      expect(t).toMatch(/mehrere Minuten/);
      // Jede Zeile, die den Plugin-Bau aufruft, trägt den Hinweis selbst oder verweist auf die Regel dazu
      const calls = t.split('\n').filter((line) => line.includes('run build:plugin'));
      expect(calls.length).toBeGreaterThan(0);
      for (const line of calls) expect(line, line).toMatch(/600000|Plugin gebaut:|Zeitgrenze|siehe Regeln/);
    });
  });

  describe('harness-grill', () => {
    const get = () => skills.find((s) => s.dir === 'harness-grill')!;

    it('beschreibt Befragung, Glossar, ADR und die Übergabe an Superpowers (Spec, Plan, Tickets)', () => {
      const t = get().text;
      for (const part of ['Empfehlung', 'GLOSSARY.md', 'docs/adr', 'docs/superpowers/specs/', '/superpowers:writing-plans', 'Lege für die Tasks des Plans Tickets an']) {
        expect(t, part).toContain(part);
      }
      expect(t).not.toContain('/speckit-');
      expect(t).toMatch(/schwer rückgängig/);
    });

    it('legt keine Tickets, Specs oder Worktrees an', () => {
      const t = get().text;
      expect(t).toMatch(/Lege keine Tickets, Specs/);
      expect(t).not.toMatch(/nexus (--json )?ticket new|nexus spec new/);
    });
  });

  describe('harness-tasks', () => {
    const get = () => skills.find((s) => s.dir === 'harness-tasks')!;

    it('fordert kurze Titel und beschreibt Dateien, Schnittstellen und Schritte', () => {
      const t = get().text;
      for (const part of ['Dateien:', 'Nutzt:', 'Liefert:', '- [ ] Test schreiben', 'docs/superpowers/plans/', 'nexus board create', 'nexus board plan-json']) {
        expect(t, part).toContain(part);
      }
      expect(t).toMatch(/höchstens etwa 60 Zeichen/);
      expect(t).toMatch(/Keine Pfade/);
    });

    it('verlangt selbsttragende Aufgaben mit Beschreibung, Akzeptanz und konkreten Schritten', () => {
      const t = get().text;
      for (const part of ['Beschreibung:', 'Akzeptanz:', 'Vorgaben', 'Erwartet:', 'selbsttragend']) {
        expect(t, part).toContain(part);
      }
      expect(t).toMatch(/Nichts bleibt abstrakt/);
      expect(t).toMatch(/Codeblock/);
    });

    it('legt keine Worktrees an und hakt Kästchen nicht selbst ab', () => {
      const t = get().text;
      expect(t).toMatch(/Keine Worktrees und keine Branches/);
      expect(t).toMatch(/Setze keine Kästchen/);
    });
  });

  describe('harness-wayfinder', () => {
    const get = () => skills.find((s) => s.dir === 'harness-wayfinder')!;

    it('führt eine Karte unter docs/klaerung mit Klärungsfragen und übergibt an Superpowers (Spec, Plan, Tickets)', () => {
      const t = get().text;
      for (const part of ['docs/klaerung/', 'Klärungsfragen', 'Noch nicht festgelegt', 'docs/superpowers/specs/', '/superpowers:writing-plans', 'Lege für die Tasks des Plans Tickets an', 'GLOSSARY.md', 'docs/adr']) {
        expect(t, part).toContain(part);
      }
      expect(t).not.toContain('/speckit-');
    });

    it('legt keine Tickets, Specs, Branches oder Worktrees an und nennt die Quelle', () => {
      const t = get().text;
      expect(t).toMatch(/Lege keine Tickets, Specs, Branches/);
      expect(t).not.toMatch(/nexus (--json )?ticket new|nexus spec new/);
      expect(t).toMatch(/MIT/);
    });
  });

  describe('harness-implement', () => {
    const get = () => skills.find((s) => s.dir === 'harness-implement')!;

    it('nutzt die Befehle des Harness für Auswahl, Start, Schritte, Kommentar und Abschluss', () => {
      const t = get().text;
      for (const part of ['nexus board list --status geplant --md', 'nexus board show', 'nexus board claim', 'nexus board start', 'nexus board step', 'nexus board comment', 'nexus board done']) {
        expect(t, part).toContain(part);
      }
      expect(t).not.toMatch(/nexus ticket |nexus question /);
    });

    it('arbeitet test-first, schließt nach dem Review mit Done ab', () => {
      const t = get().text;
      expect(t).toMatch(/Test zuerst|test-first|zuerst (einen|den) Test/i);
      expect(t).toMatch(/Review/);
      expect(t).toMatch(/nexus board done/);
      expect(t).not.toMatch(/Done.{0,80}(setzt der Benutzer|ausdrücklich)/s);
    });

    it('behandelt Superpowers als optional und nennt den Rückfall', () => {
      const t = get().text;
      expect(t).toContain('superpowers:test-driven-development');
      expect(t).toMatch(/falls vorhanden|wenn vorhanden|sonst/);
    });
  });

  describe('board', () => {
    const get = () => skills.find((s) => s.dir === 'board')!;

    it('nennt die Regeln des Boards', () => {
      const t = get().text;
      for (const part of ['Ohne Abnahme kein', 'nexus board claim', '--agent', 'nexus board note add', 'Nicht mehr nötig', 'depends', 'Human Interaction', 'nexus board list']) {
        expect(t, part).toContain(part);
      }
    });

    it('kennt keine Dateipfade des alten Boards und keine entfallenen Befehle', () => {
      const t = get().text;
      expect(t).not.toMatch(/kanban\.mjs|C:\\Tools|8787|close-iteration|iterations |changelog|daily/);
    });

    it('nutzt Fragen statt „Blockiert/Rückfrage“', () => {
      const t = get().text;
      for (const part of ['nexus board question add', 'nexus board question show', 'board-question', 'wartet auf Frage', '--ticket']) {
        expect(t, part).toContain(part);
      }
      expect(t).not.toMatch(/blocked-reason "Rückfrage"/);
    });
  });

  describe('board-question', () => {
    const get = () => skills.find((s) => s.dir === 'board-question')!;

    it('recherchiert über nexus board question und entscheidet nie', () => {
      const t = get().text;
      for (const part of ['nexus board question show', 'nexus board question research', '--from', 'höchstens fünf', 'entscheidest nie', 'nexus board question answer']) {
        expect(t, part).toContain(part);
      }
      expect(t).not.toMatch(/nexus question /);
    });

    it('hat Schreibregeln für Außenstehende: eine Entscheidung je Frage, feste Abschnitte, Fundstellen getrennt', () => {
      const t = get().text;
      for (const part of [
        '## Schreibregeln', 'Außenstehender', 'Eine Frage, eine Entscheidung', '--depends', '--description',
        '**Worum es geht**', '**Heute**', '**Zu entscheiden**', '**Fundstellen**', '**Empfehlung**', '**Weitere Frage nötig**', 'Aufwand: klein',
      ]) expect(t, part).toContain(part);
    });
  });

  describe('alle Skills', () => {
    it('nennen keine entfallenen Befehle des Datei-Boards', () => {
      for (const s of skills) {
        expect(s.text, s.dir).not.toMatch(/harness (--json )?(ticket|question|export)/);
        // Platzhalter/);
        expect(s.text, s.dir).not.toMatch(/harness-open-question/);
      }
    });

    // Der alte Spec-Kit-Weg (tasks.md, spec import/new, .specify) ist entfallen
    it('nennt kein tasks.md und keinen Spec-Kit-Import mehr', () => {
      for (const s of skills) {
        expect(s.text, s.dir).not.toMatch(/tasks\.md|spec import|spec new|speckit|\.specify/i);
      }
    });
  });

  describe('board', () => {
    it('beschreibt den eigenen Worktree mit start, step und done', () => {
      const t = skills.find((s) => s.dir === 'board')!.text;
      for (const part of ['nexus board start', 'nexus board step', 'nexus board done', '--acceptance']) expect(t, part).toContain(part);
    });

    it('beschreibt den Dispatcher für Worker und Planer', () => {
      const t = skills.find((s) => s.dir === 'board')!.text;
      for (const part of [
        '## Vom Dispatcher gestartet', 'junior-', '[dispatch]', 'nicht erneut claimen', 'nexus board done', 'Human Interaction',
        'nexus board question add', '## Aufwand', 'klein', 'standard', 'schwer', 'nexus board dispatch status', 'nexus board merge',
      ]) expect(t, part).toContain(part);
      expect(t).not.toContain('Gemergt wird nie automatisch');
    });

    it('nennt für Tickets aus einem Superpowers-Plan den Zielbranch (--target-branch)', () => {
      const t = skills.find((s) => s.dir === 'board')!.text;
      expect(t).toContain('--target-branch');
      expect(t).toMatch(/Zielbranch/);
    });

    it('beschreibt Worktree je Ticket, Merge der Voraussetzungen, Abnahme und Leeren des Branches', () => {
      const t = skills.find((s) => s.dir === 'board')!.text;
      for (const part of ['--join', 'wartet auf Merge von', 'nexus board accept', 'Abnehmen', '--clear-branch', 'joinWorktrees', 'Keine Links, Junctions oder Symlinks', 'worktreeSetup', 'Einrichtung fehlgeschlagen']) expect(t, part).toContain(part);
    });
  });
});
