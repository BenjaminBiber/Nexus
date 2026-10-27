import { describe, it, expect, afterAll } from 'vitest';
import { mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { TempProject } from './helpers/TempProject.js';

const FILE = ['.claude', 'settings.local.json'];
const RULE = ['.claude', 'rules', 'ai-harness-docs.md'];
const BOARD_RULE = ['.claude', 'rules', 'ai-harness-board.md'];

describe('Plugin je Projekt einschalten', () => {
  const project = new TempProject();
  // Eigene Konfiguration von Claude Code, damit kein Test die echte ~/.claude.json anfasst
  const claudeConfig = join(project.base, 'claude-config');
  mkdirSync(claudeConfig);
  afterAll(() => project.dispose());

  it('`harness init` schaltet das Plugin für das Projekt ein und weist auf das Neuladen hin', () => {
    const result = project.run(['init']);
    expect(result.code, result.stderr).toBe(0);
    expect(result.stdout).toMatch(/reload-plugins/);
    const settings = JSON.parse(readFileSync(join(project.path, ...FILE), 'utf8'));
    expect(settings.enabledPlugins['ai-harness@ai-harness-local']).toBe(true);
    // Wiederholung bleibt ohne Hinweis
    expect(project.run(['init']).stdout).not.toMatch(/reload-plugins/);
  });

  it('`harness init` legt die Rule für die Doku-Regeln an, ohne sie zu versionieren', () => {
    const result = project.run(['--json', 'init']);
    expect(result.code, result.stderr).toBe(0);
    // Nur das Ende vergleichen: Git liefert den langen Pfad, TEMP kann unter Windows ein 8.3-Kurzname sein
    const docsRule = (JSON.parse(result.stdout).docsRule as string).replace(/\\/g, '/');
    expect(docsRule).toMatch(/\/Demo Projekt ä\/\.claude\/rules\/ai-harness-docs\.md$/);
    expect(readFileSync(join(project.path, ...RULE), 'utf8')).toContain('@');
    expect(project.git('status', '--porcelain', '--untracked-files=all')).not.toContain('ai-harness-docs.md');
  });

  it('`harness init` legt die immer geladene Board-Regel an und ergänzt sie in schon eingerichteten Projekten', () => {
    const boardRule = join(project.path, ...BOARD_RULE);
    rmSync(boardRule, { force: true });
    const result = project.run(['init']);
    expect(result.code, result.stderr).toBe(0);
    expect(result.stdout).toContain('Board-Regel eingebunden: .claude/rules/ai-harness-board.md');
    const text = readFileSync(boardRule, 'utf8');
    expect(text).not.toContain('paths:');
    expect(text).toMatch(/\n@.*rules\/board\.md\n$/);
    expect(project.git('status', '--porcelain', '--untracked-files=all')).not.toContain('ai-harness-board.md');
    const json = JSON.parse(project.run(['--json', 'init']).stdout);
    expect((json.boardRule as string).replace(/\\/g, '/')).toMatch(/\/\.claude\/rules\/ai-harness-board\.md$/);
    expect(project.run(['init']).stdout).not.toContain('Board-Regel eingebunden');
  });

  it('`harness init` gibt den Import der Doku-Regeln in der Konfiguration von Claude Code frei', () => {
    const configFile = join(claudeConfig, '.claude.json');
    writeFileSync(configFile, JSON.stringify({ projects: {} }));
    const result = project.run(['--json', 'init'], project.path, { CLAUDE_CONFIG_DIR: claudeConfig });
    expect(result.code, result.stderr).toBe(0);
    expect(JSON.parse(result.stdout).importApproval.status).toBe('approved');
    const projects = JSON.parse(readFileSync(configFile, 'utf8')).projects as Record<string, Record<string, unknown>>;
    const entries = Object.values(projects);
    expect(entries.length).toBeGreaterThan(0);
    for (const entry of entries) expect(entry.hasClaudeMdExternalIncludesApproved).toBe(true);
  });
});
