import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { VersionFiles } from '../lib/VersionFiles.mjs';
import { RepoCopy } from './RepoCopy.mjs';

// Alle Stellen des Fixture-Repos. Die Liste hält fest, was das Skript findet – kommt eine Datei dazu, fällt es hier auf.
const ALL_PLACES = [
  '.claude-plugin/marketplace.json plugins[fix-harness] → version',
  'Cargo.toml [workspace.package] version',
  'apps/desktop/package.json version',
  'apps/desktop/src-tauri/Cargo.lock [[package]] fix-core → version',
  'apps/desktop/src-tauri/Cargo.lock [[package]] fix-desktop → version',
  'apps/desktop/src-tauri/Cargo.toml [package] version',
  'package-lock.json packages["apps/desktop"] → version',
  'package-lock.json packages["packages/cli"] → dependencies["@fix/core"]',
  'package-lock.json packages["packages/cli"] → version',
  'package-lock.json packages["packages/core"] → version',
  'package-lock.json packages["packages/plugin"] → version',
  'packages/cli/package.json dependencies["@fix/core"]',
  'packages/cli/package.json version',
  'packages/core/package.json version',
  'packages/plugin/manifest/.claude-plugin/plugin.json version',
  'packages/plugin/package.json version',
];

describe('VersionFiles', () => {
  let repo;

  beforeEach(() => {
    repo = new RepoCopy();
  });

  afterEach(() => {
    repo.dispose();
  });

  const files = () => new VersionFiles(repo.root);
  const named = (places) => places.map((place) => `${place.file} ${place.where}`).sort();

  it('nimmt die Workspace-Cargo.toml als Quelle', () => {
    const { source } = files().collect();
    expect(source.file).toBe('Cargo.toml');
    expect(source.where).toBe('[workspace.package] version');
    expect(files().current()).toBe('0.1.0');
  });

  it('nimmt ohne Workspace-Cargo.toml die Version des einzigen Crates als Quelle', () => {
    repo.remove('Cargo.toml');
    const { source } = files().collect();
    expect(source.file).toBe('apps/desktop/src-tauri/Cargo.toml');
    expect(source.where).toBe('[package] version');
  });

  it('findet alle Versionsstellen', () => {
    expect(named(files().collect().places)).toEqual(ALL_PLACES);
  });

  it('meldet die fehlende Version im Marktplatz-Eintrag als Abweichung', () => {
    const { deviations } = files().check();
    expect(named(deviations)).toEqual(['.claude-plugin/marketplace.json plugins[fix-harness] → version']);
    expect(deviations[0].version).toBeNull();
  });

  it('setzt alle Stellen auf die neue Version', () => {
    files().set('0.2.0');
    const after = files().check();
    expect(after.version).toBe('0.2.0');
    expect(after.deviations).toEqual([]);
    expect(named(after.places)).toEqual(ALL_PLACES);
    expect(after.places.every((place) => place.version === '0.2.0')).toBe(true);
  });

  it('lässt eine Datei mit `version.workspace = true` unverändert', () => {
    files().set('0.2.0');
    expect(repo.read('crates/core/Cargo.toml')).toBe(repo.original('crates/core/Cargo.toml'));
  });

  it('lässt fremde Versionen in Cargo.lock und package-lock.json stehen', () => {
    files().set('0.2.0');
    const lock = repo.read('apps/desktop/src-tauri/Cargo.lock');
    // Version des Lock-Formats und das fremde Paket
    expect(lock).toContain('\nversion = 4\n');
    expect(lock).toContain('name = "serde"\nversion = "1.0.210"');
    const npmLock = repo.read('package-lock.json');
    expect(npmLock).toContain('"node_modules/yaml"');
    expect(JSON.parse(npmLock).packages['node_modules/yaml'].version).toBe('2.5.0');
    expect(JSON.parse(npmLock).packages['packages/web']).toEqual({ name: '@fix/web' });
  });

  it('zieht exakt gepinnte Geschwister mit und lässt `^`-Angaben in Ruhe', () => {
    files().set('0.2.0');
    const cli = JSON.parse(repo.read('packages/cli/package.json'));
    expect(cli.dependencies['@fix/core']).toBe('0.2.0');
    expect(cli.dependencies['@fix/web']).toBe('^0.1.0');
    expect(cli.dependencies.commander).toBe('^12.1.0');
    expect(JSON.parse(repo.read('package-lock.json')).packages['packages/cli'].dependencies['@fix/core']).toBe('0.2.0');
  });

  it('entfernt eine Version aus tauri.conf.json und warnt davor', () => {
    const { warnings } = files().set('0.2.0');
    expect(warnings.join('\n')).toContain('tauri.conf.json');
    const conf = JSON.parse(repo.read('apps/desktop/src-tauri/tauri.conf.json'));
    expect(conf.version).toBeUndefined();
    expect(conf.productName).toBe('Fixture');
    expect(conf.identifier).toBe('cc.example.fixture');
    expect(conf.app.windows[0].label).toBe('main');
  });

  it('legt die Version im Marktplatz-Eintrag an und lässt fremde Einträge stehen', () => {
    files().set('0.2.0');
    const market = JSON.parse(repo.read('.claude-plugin/marketplace.json'));
    expect(market.plugins[1]).toEqual({
      name: 'fix-harness',
      version: '0.2.0',
      source: './packages/plugin/dist/fix-harness',
      defaultEnabled: false,
    });
    expect(market.plugins[0]).toEqual({ name: 'fremdes-plugin', version: '9.9.9', source: './fremd' });
  });

  it('ändert in einer Datei nur die Versionszeile', () => {
    files().set('0.2.0');
    expect(repo.read('packages/plugin/manifest/.claude-plugin/plugin.json'))
      .toBe(repo.original('packages/plugin/manifest/.claude-plugin/plugin.json').replace('"0.1.0"', '"0.2.0"'));
    expect(repo.read('apps/desktop/src-tauri/Cargo.toml'))
      .toBe(repo.original('apps/desktop/src-tauri/Cargo.toml').replace('"0.1.0"', '"0.2.0"'));
  });

  it('nennt die geänderten Dateien und fasst sie je Datei zusammen', () => {
    const { changed } = files().set('0.2.0');
    expect([...changed].sort()).toEqual([
      '.claude-plugin/marketplace.json',
      'Cargo.toml',
      'apps/desktop/package.json',
      'apps/desktop/src-tauri/Cargo.lock',
      'apps/desktop/src-tauri/Cargo.toml',
      'apps/desktop/src-tauri/tauri.conf.json',
      'package-lock.json',
      'packages/cli/package.json',
      'packages/core/package.json',
      'packages/plugin/manifest/.claude-plugin/plugin.json',
      'packages/plugin/package.json',
    ]);
    expect(new Set(changed).size).toBe(changed.length);
  });

  it('ändert beim zweiten Setzen derselben Version keine Datei mehr', () => {
    files().set('0.2.0');
    const before = ALL_PLACES.map((place) => place.split(' ')[0]).map((file) => repo.read(file));
    expect(files().set('0.2.0').changed).toEqual([]);
    expect(ALL_PLACES.map((place) => place.split(' ')[0]).map((file) => repo.read(file))).toEqual(before);
  });

  it('meldet eine absichtlich falsch gesetzte Stelle', () => {
    files().set('0.2.0');
    repo.write('packages/core/package.json', repo.read('packages/core/package.json').replace('"0.2.0"', '"0.9.9"'));
    const { deviations, version } = files().check();
    expect(version).toBe('0.2.0');
    expect(named(deviations)).toEqual(['packages/core/package.json version']);
    expect(deviations[0].version).toBe('0.9.9');
  });

  it('wirft bei einer Version, die keine ist', () => {
    expect(() => files().set('0.2')).toThrow(/keine Version/);
  });

  it('wirft, wenn es keine Quelle gibt', () => {
    repo.remove('Cargo.toml');
    repo.remove('apps/desktop/src-tauri/Cargo.toml');
    expect(() => files().collect()).toThrow(/Keine Cargo.toml mit einer Version/);
  });
});
