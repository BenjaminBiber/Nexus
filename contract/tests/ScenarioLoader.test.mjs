import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { ScenarioLoader } from '../lib/ScenarioLoader.mjs';

const GOOD = {
  title: 'Projekte anmelden',
  fixtures: { repos: [{ name: 'REPO1', harnessInit: true, files: { 'docs/a.md': '# A\n' } }] },
  steps: [
    { name: 'anmelden', method: 'POST', path: '/api/projects', body: { path: '{{REPO1}}' }, save: { pid: 'json.id' } },
    { name: 'liste', method: 'GET', path: '/api/projects' },
  ],
};

describe('ScenarioLoader', () => {
  let root;
  let loader;

  beforeEach(() => {
    root = mkdtempSync(join(tmpdir(), 'harness-contract-loader-'));
    mkdirSync(join(root, 'api', 'basis'), { recursive: true });
    loader = new ScenarioLoader(root);
  });

  afterEach(() => {
    rmSync(root, { recursive: true, force: true });
  });

  // Schreibt ein Szenario (Objekt oder Text) und liefert seinen Pfad
  const write = (name, value) => {
    const file = join(root, 'api', 'basis', `${name}.scenario.json`);
    writeFileSync(file, typeof value === 'string' ? value : JSON.stringify(value, null, 2));
    return file;
  };

  // Erwartet eine Meldung beim Lesen eines fehlerhaften Szenarios
  const failsWith = (value, pattern) => {
    const file = write('kaputt', value);
    expect(() => loader.parse(file)).toThrow(pattern);
  };

  describe('gültige Szenarien', () => {
    it('liest Titel, Fixtures und Schritte', () => {
      const scenario = loader.parse(write('projects', GOOD));
      expect(scenario.title).toBe('Projekte anmelden');
      expect(scenario.steps).toHaveLength(2);
      expect(scenario.fixtures.repos[0]).toEqual({ name: 'REPO1', git: true, harnessInit: true, files: { 'docs/a.md': '# A\n' } });
    });

    it('setzt die Standardwerte je Schritt', () => {
      const scenario = loader.parse(write('projects', GOOD));
      expect(scenario.steps[1]).toMatchObject({ kind: 'http', compare: 'full', record: true, ignore: [], save: {}, headers: {} });
    });

    it('nennt das Szenario nach seinem Pfad und findet die Golden-Datei daneben', () => {
      const scenario = loader.parse(write('projects', GOOD));
      expect(scenario.name).toBe('api/basis/projects');
      expect(scenario.golden).toBe(join(root, 'api', 'basis', 'projects.golden.json'));
    });

    it('ohne platforms und fixtures gilt: überall, ohne Vorbereitung', () => {
      const scenario = loader.parse(write('health', { title: 'Health', steps: [{ name: 'health', method: 'GET', path: '/api/health' }] }));
      expect(scenario.platforms).toEqual([]);
      expect(scenario.fixtures).toEqual({ repos: [], boardConfig: null, claudeEnvFile: false });
      expect(ScenarioLoader.runsHere(scenario, 'win32')).toBe(true);
    });

    it('platforms grenzt das Betriebssystem ein', () => {
      const scenario = loader.parse(write('nurwin', { title: 'Nur Windows', platforms: ['win32'], steps: GOOD.steps }));
      expect(ScenarioLoader.runsHere(scenario, 'win32')).toBe(true);
      expect(ScenarioLoader.runsHere(scenario, 'linux')).toBe(false);
    });

    it('findet alle Szenarien eines Baums, nach Namen sortiert', () => {
      write('projects', GOOD);
      write('health', { title: 'Health', steps: [{ name: 'health', method: 'GET', path: '/api/health' }] });
      writeFileSync(join(root, 'api', 'basis', 'projects.golden.json'), '[]');
      const scenarios = loader.load(join(root, 'api'));
      expect(scenarios.map((scenario) => scenario.name)).toEqual(['api/basis/health', 'api/basis/projects']);
    });

    it('liefert für einen fehlenden Ordner eine leere Liste', () => {
      expect(loader.load(join(root, 'gibtsnicht'))).toEqual([]);
    });

    it('nimmt die eigenen Felder einer anderen Schritt-Art als raw mit', () => {
      const steps = [{ name: 'Attrappe', kind: 'httpStub', action: 'start', stub: 'NTFY', record: false }];
      const scenario = loader.parse(write('stub', { title: 'Attrappe', steps }));
      expect(scenario.steps[0]).toMatchObject({ kind: 'httpStub', record: false, raw: steps[0] });
    });
  });

  describe('Schritte der Art cli', () => {
    // Szenario mit einem CLI-Schritt
    const cli = (step) => ({ title: 'Hook', steps: [{ name: 'hook', kind: 'cli', ...step }] });

    it('liest Argumente, Arbeitsordner, Eingabe und Umgebung', () => {
      const scenario = loader.parse(write('hook', cli({
        args: ['hook', 'session-start'], cwd: '{{REPO1}}', stdin: { session_id: 's1' }, env: { HARNESS_TICKET: 'ARC-0001' },
      })));
      expect(scenario.steps[0]).toMatchObject({
        kind: 'cli', args: ['hook', 'session-start'], cwd: '{{REPO1}}', stdin: { session_id: 's1' }, env: { HARNESS_TICKET: 'ARC-0001' },
      });
    });

    it('nimmt eine Eingabe als Text, auch kaputtes JSON', () => {
      const scenario = loader.parse(write('hook', cli({ args: ['hook', 'session-start'], stdin: '{kaputt' })));
      expect(scenario.steps[0].stdin).toBe('{kaputt');
    });

    it('lässt cwd, stdin und env weg; dann gelten Home und keine Eingabe', () => {
      const scenario = loader.parse(write('hook', cli({ args: ['--help'] })));
      expect(scenario.steps[0]).toMatchObject({ cwd: undefined, stdin: undefined, env: {} });
    });

    it('meldet fehlende oder leere Argumente', () => {
      failsWith(cli({}), /steps\[0\]\.args muss eine nicht leere Liste aus Texten sein/);
      failsWith(cli({ args: [] }), /steps\[0\]\.args muss eine nicht leere Liste aus Texten sein/);
      failsWith(cli({ args: ['hook', 7] }), /steps\[0\]\.args muss eine nicht leere Liste aus Texten sein/);
    });

    it('meldet eine Eingabe, die weder Objekt noch Text ist', () => {
      failsWith(cli({ args: ['hook'], stdin: 7 }), /steps\[0\]\.stdin muss ein Objekt \(als JSON\) oder ein Text \(roh\) sein/);
      failsWith(cli({ args: ['hook'], stdin: [] }), /steps\[0\]\.stdin muss ein Objekt \(als JSON\) oder ein Text \(roh\) sein/);
    });

    it('meldet eine Umgebungsvariable, die kein Text ist', () => {
      failsWith(cli({ args: ['hook'], env: { HARNESS_TICKET: 1 } }), /steps\[0\]\.env\["HARNESS_TICKET"\] muss ein Text sein/);
    });

    it('meldet Felder der Art http an einem CLI-Schritt', () => {
      failsWith(cli({ args: ['hook'], method: 'GET' }), /unbekannte Felder in steps\[0\]: method/);
    });

    it('nimmt die Fixture claudeEnvFile', () => {
      const scenario = loader.parse(write('hook', { ...cli({ args: ['hook'] }), fixtures: { claudeEnvFile: true } }));
      expect(scenario.fixtures.claudeEnvFile).toBe(true);
    });

    it('meldet ein claudeEnvFile, das nicht true oder false ist', () => {
      failsWith({ ...cli({ args: ['hook'] }), fixtures: { claudeEnvFile: 'ja' } }, /fixtures\.claudeEnvFile muss true oder false sein/);
    });
  });

  describe('fehlerhafte Szenarien', () => {
    it('meldet ungültiges JSON mit dem Namen der Datei', () => {
      failsWith('{ kein json', /api\/basis\/kaputt\.scenario\.json: kein gültiges JSON/);
    });

    it('meldet einen fehlenden Titel', () => {
      failsWith({ steps: GOOD.steps }, /title fehlt/);
    });

    it('meldet unbekannte Felder im Szenario', () => {
      failsWith({ ...GOOD, titel: 'Tippfehler' }, /unbekannte Felder: titel/);
    });

    it('meldet unbekannte Felder in einem Schritt', () => {
      failsWith({ title: 'T', steps: [{ name: 'a', method: 'GET', path: '/api/health', pfad: '/x' }] }, /unbekannte Felder in steps\[0\]: pfad/);
    });

    it('meldet unbekannte Felder in einem Schritt einer anderen Art', () => {
      failsWith({ title: 'T', steps: [{ name: 'a', kind: 'httpStub', stub: 'NTFY', aktion: 'start' }] }, /unbekannte Felder in steps\[0\]: aktion/);
    });

    it('meldet eine unbekannte Schritt-Art', () => {
      failsWith({ title: 'T', steps: [{ name: 'a', kind: 'sse' }] }, /steps\[0\]\.kind muss eine bekannte Art sein \(cli, http, httpStub\)/);
    });

    it('meldet leere Schritte', () => {
      failsWith({ title: 'T', steps: [] }, /steps muss eine nicht leere Liste sein/);
    });

    it('meldet eine unbekannte Methode', () => {
      failsWith({ title: 'T', steps: [{ name: 'a', method: 'FETCH', path: '/api/health' }] }, /steps\[0\]\.method/);
    });

    it('meldet einen Pfad ohne führenden Schrägstrich', () => {
      failsWith({ title: 'T', steps: [{ name: 'a', method: 'GET', path: 'api/health' }] }, /steps\[0\]\.path muss mit \/ beginnen/);
    });

    it('meldet doppelte Schrittnamen', () => {
      const steps = [{ name: 'a', method: 'GET', path: '/api/health' }, { name: 'a', method: 'GET', path: '/api/projects' }];
      failsWith({ title: 'T', steps }, /steps enthält den Namen „a“ zweimal/);
    });

    it('meldet ein unbekanntes compare', () => {
      failsWith({ title: 'T', steps: [{ name: 'a', method: 'GET', path: '/api/health', compare: 'fast' }] }, /steps\[0\]\.compare/);
    });

    it('meldet einen save-Pfad ohne json. oder headers.', () => {
      failsWith({ title: 'T', steps: [{ name: 'a', method: 'GET', path: '/api/health', save: { id: 'body.id' } }] }, /save\["id"\] muss mit json\. oder headers\. beginnen/);
    });

    it('meldet einen Repo-Namen in Kleinbuchstaben', () => {
      failsWith({ title: 'T', fixtures: { repos: [{ name: 'repo1' }] }, steps: GOOD.steps }, /fixtures\.repos\[0\]\.name/);
    });

    it('meldet doppelte Repo-Namen', () => {
      failsWith({ title: 'T', fixtures: { repos: [{ name: 'REPO1' }, { name: 'REPO1' }] }, steps: GOOD.steps }, /den Namen REPO1 zweimal/);
    });

    it('meldet harnessInit ohne git', () => {
      failsWith({ title: 'T', fixtures: { repos: [{ name: 'REPO1', git: false, harnessInit: true }] }, steps: GOOD.steps }, /harnessInit braucht git: true/);
    });

    it('meldet ein unbekanntes boardConfig', () => {
      failsWith({ title: 'T', fixtures: { boardConfig: 'mssql' }, steps: GOOD.steps }, /fixtures\.boardConfig erlaubt nur sqlite/);
    });

    it('meldet ein unbekanntes Betriebssystem', () => {
      failsWith({ title: 'T', platforms: ['macos'], steps: GOOD.steps }, /platforms muss eine Liste/);
    });

    it('meldet einen Kopf, der kein Text und nicht null ist', () => {
      failsWith({ title: 'T', steps: [{ name: 'a', method: 'GET', path: '/api/health', headers: { 'x-harness': 1 } }] }, /headers\["x-harness"\] muss ein Text oder null sein/);
    });
  });
});
