import { execFileSync, spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { parseArgs, today } from '../bump.mjs';
import { RepoCopy } from './RepoCopy.mjs';

const SCRIPT = fileURLToPath(new URL('../bump.mjs', import.meta.url));

describe('bump.mjs', () => {
  let repo;

  beforeEach(() => {
    repo = new RepoCopy();
  });

  afterEach(() => {
    repo.dispose();
  });

  // Jeder Aufruf bekommt --root: die Tests arbeiten nur auf der Kopie, nie im echten Repo
  const run = (...args) => {
    const result = spawnSync(process.execPath, [SCRIPT, ...args, '--root', repo.root], { encoding: 'utf8', windowsHide: true });
    return { code: result.status, out: result.stdout, err: result.stderr };
  };

  const git = (...args) => execFileSync('git', args, {
    cwd: repo.root, encoding: 'utf8', windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'],
  });

  // Macht aus der Kopie ein Git-Repo mit Tags
  const withTags = (...tags) => {
    git('init', '-q', '-b', 'main');
    git('add', '-A');
    git('-c', 'user.name=Test', '-c', 'user.email=test@example.invalid', 'commit', '-q', '-m', 'erster');
    for (const tag of tags) git('tag', tag);
  };

  describe('parseArgs', () => {
    it('liest Befehl, Wert und Optionen', () => {
      const options = parseArgs(['set', '0.2.0', '--changelog', '--date', '2026-11-01', '--root', '/tmp/x']);
      expect(options).toMatchObject({ command: 'set', value: '0.2.0', changelog: true, date: '2026-11-01', root: '/tmp/x' });
    });

    it('liest `--check` wie `check`', () => {
      expect(parseArgs(['--check']).command).toBe('check');
      expect(parseArgs(['check']).command).toBe('check');
    });

    it('wirft bei unbekanntem Befehl, unbekanntem Argument und fehlendem Wert', () => {
      expect(() => parseArgs(['unbekannt'])).toThrow(/Unbekannter Befehl/);
      expect(() => parseArgs(['check', '--quatsch'])).toThrow(/Unbekanntes Argument/);
      expect(() => parseArgs(['--root'])).toThrow(/braucht einen Wert/);
    });

    it('nimmt ohne --root das Repo dieses Skripts', () => {
      expect(parseArgs(['check']).root).toMatch(/Nexus/);
    });
  });

  it('schreibt das heutige Datum als YYYY-MM-DD', () => {
    expect(today(new Date(2026, 10, 1))).toBe('2026-11-01');
  });

  it('gibt bei `current` die Version der Quelle aus', () => {
    const { code, out } = run('current');
    expect(code).toBe(0);
    expect(out).toBe('0.1.0\n');
  });

  it('rechnet `next` ohne Tag ab der Quelle', () => {
    expect(run('next', 'patch').out).toBe('0.1.1\n');
    expect(run('next', 'patch').err).toContain('Kein Git-Repo');
  });

  it('rechnet `next` ab dem höchsten Tag und übergeht Vorabversionen', () => {
    withTags('v0.9.0', 'v0.10.0', 'v0.10.0-rc.1');
    expect(run('next', 'patch').out).toBe('0.10.1\n');
    expect(run('next', 'minor').out).toBe('0.11.0\n');
    expect(run('next', 'major').out).toBe('1.0.0\n');
  });

  it('warnt bei einem Tag, das keine Version ist', () => {
    withTags('v0.9.0', 'vFoo');
    const { out, err } = run('next', 'patch');
    expect(out).toBe('0.9.1\n');
    expect(err).toContain('vFoo');
  });

  it('endet bei `check` mit 1 und listet die Abweichung', () => {
    const { code, out } = run('check');
    expect(code).toBe(1);
    expect(out).toContain('marketplace.json');
    expect(out).toContain('fehlt');
  });

  it('endet bei `check` mit 0, wenn alle Stellen gleich sind', () => {
    expect(run('set', '0.2.0').code).toBe(0);
    const { code, out } = run('check');
    expect(code).toBe(0);
    expect(out).toContain('16 Stellen geprüft, alle auf 0.2.0.');
  });

  it('endet bei `--check` wie bei `check`', () => {
    expect(run('--check').code).toBe(1);
    run('set', '0.2.0');
    expect(run('--check').code).toBe(0);
  });

  it('setzt mit `set` alle Stellen und prüft danach selbst', () => {
    const { code, out } = run('set', '0.2.0');
    expect(code).toBe(0);
    expect(out).toContain('auf 0.2.0 gesetzt');
    expect(out).toContain('16 Stellen geprüft, alle auf 0.2.0.');
    expect(repo.read('apps/desktop/src-tauri/Cargo.toml')).toContain('version = "0.2.0"');
  });

  it('lässt das Fixture selbst unverändert – gearbeitet wird nur auf der Kopie', () => {
    run('set', '0.2.0');
    expect(repo.original('Cargo.toml')).toContain('version = "0.1.0"');
    expect(repo.read('Cargo.toml')).toContain('version = "0.2.0"');
  });

  it('endet bei einer Version, die keine ist, mit 1 und ändert nichts', () => {
    const { code, err } = run('set', '0.2');
    expect(code).toBe(1);
    expect(err).toContain('keine Version');
    expect(repo.read('Cargo.toml')).toBe(repo.original('Cargo.toml'));
  });

  it('benennt mit --changelog den Abschnitt um und setzt einen leeren darüber', () => {
    const { code, out } = run('set', '0.2.0', '--changelog', '--date', '2026-11-01');
    expect(code).toBe(0);
    expect(out).toContain('CHANGELOG.md');
    const changelog = repo.read('CHANGELOG.md');
    expect(changelog).toContain('## 0.2.0 – 2026-11-01');
    expect(changelog).toContain('## Unveröffentlicht\n\n### Vertrag\n\n- Keine Änderungen.');
    expect(changelog.indexOf('## Unveröffentlicht')).toBeLessThan(changelog.indexOf('## 0.2.0'));
    // Der Inhalt von „Unveröffentlicht“ steht jetzt im Versionsabschnitt
    expect(changelog.indexOf('- Neue Route `GET /api/fixture`.')).toBeGreaterThan(changelog.indexOf('## 0.2.0'));
  });

  it('schreibt ohne „Unveröffentlicht“ gar nichts – auch keine Version', () => {
    repo.write('CHANGELOG.md', '# Changelog\n\n## 0.0.9 – 2026-10-01\n\n- Der erste Stand.\n');
    const { code, err } = run('set', '0.2.0', '--changelog');
    expect(code).toBe(1);
    expect(err).toContain('fehlt die Überschrift');
    expect(repo.read('Cargo.toml')).toBe(repo.original('Cargo.toml'));
  });

  it('warnt, wenn --date ohne --changelog kommt', () => {
    expect(run('set', '0.2.0', '--date', '2026-11-01').err).toContain('--date wirkt nur');
  });

  it('gibt mit `notes` den Abschnitt einer Version aus', () => {
    run('set', '0.2.0', '--changelog', '--date', '2026-11-01');
    const { code, out } = run('notes', '0.2.0');
    expect(code).toBe(0);
    expect(out).toContain('- Neue Route `GET /api/fixture`.');
    expect(out).not.toContain('## 0.2.0');
    expect(out).not.toContain('Der erste Stand');
    expect(run('notes', '0.0.9').out.trim()).toBe('### Sonstiges\n\n- Der erste Stand.');
  });

  it('endet bei `notes` ohne Abschnitt mit 1', () => {
    const { code, err } = run('notes', '5.0.0');
    expect(code).toBe(1);
    expect(err).toContain('keinen Abschnitt für 5.0.0');
  });

  it('endet ohne Befehl und bei einem unbekannten Befehl mit 1', () => {
    expect(run().code).toBe(1);
    expect(run().out).toContain('node scripts/release/bump.mjs next');
    expect(run('quatsch').code).toBe(1);
    expect(run('quatsch').err).toContain('Unbekannter Befehl');
  });

  it('zeigt mit --help die Hilfe und endet mit 0', () => {
    const { code, out } = run('--help');
    expect(code).toBe(0);
    expect(out).toContain('node scripts/release/bump.mjs set <X.Y.Z>');
  });
});
