import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { ClaudeImportApproval } from '../../../src/install/ClaudeImportApproval.js';

const APPROVED = 'hasClaudeMdExternalIncludesApproved';
const SHOWN = 'hasClaudeMdExternalIncludesWarningShown';

describe('ClaudeImportApproval', () => {
  let base: string;
  let home: string;
  let configDir: string;

  const read = (file: string) => JSON.parse(readFileSync(file, 'utf8')) as { projects: Record<string, Record<string, unknown>> } & Record<string, unknown>;
  const write = (file: string, value: unknown) => writeFileSync(file, JSON.stringify(value, null, 2));

  beforeEach(() => {
    base = mkdtempSync(join(tmpdir(), 'harness-approval-'));
    home = join(base, 'home');
    configDir = join(base, 'config');
    mkdirSync(home);
    mkdirSync(configDir);
  });

  afterEach(() => rmSync(base, { recursive: true, force: true }));

  describe('Fundort der Konfiguration', () => {
    it('nimmt ~/.claude.json im Benutzerordner', () => {
      expect(new ClaudeImportApproval({}, home).configFile()).toBe(join(home, '.claude.json'));
    });

    it('folgt CLAUDE_CONFIG_DIR wie Claude Code selbst', () => {
      expect(new ClaudeImportApproval({ CLAUDE_CONFIG_DIR: configDir }, home).configFile()).toBe(join(configDir, '.claude.json'));
    });

    it('nimmt die ältere ~/.claude/.config.json, wenn es sie gibt', () => {
      mkdirSync(join(home, '.claude'));
      write(join(home, '.claude', '.config.json'), {});
      expect(new ClaudeImportApproval({}, home).configFile()).toBe(join(home, '.claude', '.config.json'));
    });
  });

  describe('Schlüssel des Projekts', () => {
    it('schreibt unter Windows den Pfad mit Schrägstrichen, Laufwerk groß und klein', () => {
      const approval = new ClaudeImportApproval({}, home, 'win32');
      expect(approval.keys('C:\\Dev\\Mein Projekt', {})).toEqual(['C:/Dev/Mein Projekt', 'c:/Dev/Mein Projekt']);
    });

    it('nimmt unter Windows vorhandene Schreibweisen desselben Pfads mit', () => {
      const approval = new ClaudeImportApproval({}, home, 'win32');
      const projects = { 'C:\\Dev\\Mein Projekt': {}, 'c:/dev/mein projekt/': {}, 'C:/Dev/Anderes': {} };
      expect(approval.keys('C:/Dev/Mein Projekt', projects).sort()).toEqual(['C:/Dev/Mein Projekt', 'C:\\Dev\\Mein Projekt', 'c:/Dev/Mein Projekt', 'c:/dev/mein projekt/'].sort());
    });

    it('lässt den Pfad sonst unverändert', () => {
      expect(new ClaudeImportApproval({}, home, 'linux').keys('/home/x/Projekt', {})).toEqual(['/home/x/Projekt']);
    });
  });

  describe('Freigabe setzen', () => {
    it('setzt beide Werte und lässt alles andere unverändert', async () => {
      const file = join(home, '.claude.json');
      write(file, { userID: 'u', projects: { '/p': { allowedTools: ['Bash(ls)'], [APPROVED]: false, [SHOWN]: false }, '/q': { x: 1 } } });
      const result = await new ClaudeImportApproval({}, home, 'linux').approve('/p');
      expect(result).toEqual({ status: 'approved', file, keys: ['/p'] });
      expect(read(file)).toEqual({ userID: 'u', projects: { '/p': { allowedTools: ['Bash(ls)'], [APPROVED]: true, [SHOWN]: true }, '/q': { x: 1 } } });
    });

    it('legt einen fehlenden Projekteintrag an', async () => {
      const file = join(home, '.claude.json');
      write(file, { projects: {} });
      await new ClaudeImportApproval({}, home, 'linux').approve('/neu');
      expect(read(file).projects['/neu']).toEqual({ [APPROVED]: true, [SHOWN]: true });
    });

    it('ist wiederholbar und meldet, wenn schon alles gesetzt ist', async () => {
      const file = join(home, '.claude.json');
      write(file, { projects: { '/p': { [APPROVED]: true, [SHOWN]: true } } });
      const before = readFileSync(file, 'utf8');
      const result = await new ClaudeImportApproval({}, home, 'linux').approve('/p');
      expect(result.status).toBe('unchanged');
      expect(readFileSync(file, 'utf8')).toBe(before);
    });

    it('setzt unter Windows alle Schreibweisen des Projekts', async () => {
      const file = join(home, '.claude.json');
      write(file, { projects: { 'C:\\Dev\\P': { [APPROVED]: false } } });
      await new ClaudeImportApproval({}, home, 'win32').approve('C:/Dev/P');
      const projects = read(file).projects;
      for (const key of ['C:\\Dev\\P', 'C:/Dev/P', 'c:/Dev/P']) expect(projects[key][APPROVED], key).toBe(true);
    });

    it('meldet fehlende Konfiguration und legt keine Datei an (Claude Code lief hier noch nie)', async () => {
      const result = await new ClaudeImportApproval({}, home, 'linux').approve('/p');
      expect(result).toEqual({ status: 'no-config', file: join(home, '.claude.json'), keys: [] });
      expect(existsSync(join(home, '.claude.json'))).toBe(false);
    });

    it('überschreibt kaputtes JSON nie', async () => {
      const file = join(home, '.claude.json');
      writeFileSync(file, '{ kaputt');
      await expect(new ClaudeImportApproval({}, home, 'linux').approve('/p')).rejects.toThrow(/kein gültiges JSON/);
      expect(readFileSync(file, 'utf8')).toBe('{ kaputt');
    });
  });
});
