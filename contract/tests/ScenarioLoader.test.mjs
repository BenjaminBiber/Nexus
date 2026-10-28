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
      expect(scenario.fixtures).toEqual({ repos: [], boardConfig: null, homeFiles: {} });
      expect(ScenarioLoader.runsHere(scenario, 'win32')).toBe(true);
    });

    it('übernimmt homeFiles als Text und als JSON-Wert', () => {
      const homeFiles = { 'dispatch-logs/a.log': 'eine Zeile\n', 'dispatch/runs/r1.json': { id: 'r1', pid: null }, 'dispatch/liste.json': [1, 2] };
      const scenario = loader.parse(write('home', { title: 'Home', fixtures: { homeFiles }, steps: GOOD.steps }));
      expect(scenario.fixtures.homeFiles).toEqual(homeFiles);
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

    it('liest die Schritte eines Ereignisstroms mit den Feldern ihrer Art', () => {
      const steps = [
        { name: 'öffnen', kind: 'sseOpen', as: 's1', path: '/api/projects/x/events?notify=1' },
        { name: 'hello', kind: 'sseExpect', stream: 's1', until: { type: 'hello' } },
        { name: 'schließen', kind: 'sseClose', stream: 's1' },
      ];
      const scenario = loader.parse(write('strom', { title: 'Strom', steps }));
      expect(scenario.steps[0]).toMatchObject({ kind: 'sseOpen', as: 's1', path: '/api/projects/x/events?notify=1' });
      expect(scenario.steps[1]).toMatchObject({ kind: 'sseExpect', stream: 's1', until: { type: 'hello' }, timeoutMs: 3000 });
      expect(scenario.steps[2]).toMatchObject({ kind: 'sseClose', stream: 's1' });
    });

    it('nimmt die CONTRACT-Variablen des Szenarios', () => {
      const scenario = loader.parse(write('takt', { title: 'Takt', env: { CONTRACT_HEARTBEAT_MS: '300' }, steps: GOOD.steps }));
      expect(scenario.env).toEqual({ CONTRACT_HEARTBEAT_MS: '300' });
    });

    it('ohne env bleibt die Umgebung leer', () => {
      expect(loader.parse(write('projects', GOOD)).env).toEqual({});
    });

    it('nimmt die eigenen Felder einer anderen Schritt-Art als raw mit', () => {
      const steps = [{ name: 'Attrappe', kind: 'httpStub', action: 'start', stub: 'NTFY', record: false }];
      const scenario = loader.parse(write('stub', { title: 'Attrappe', steps }));
      expect(scenario.steps[0]).toMatchObject({ kind: 'httpStub', record: false, raw: steps[0] });
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
      const alle = /steps\[0\]\.kind muss eine bekannte Art sein \(http, httpStub, sseClose, sseExpect, sseOpen\)/;
      failsWith({ title: 'T', steps: [{ name: 'a', kind: 'cli' }] }, alle);
      failsWith({ title: 'T', steps: [{ name: 'a', kind: 'sseWarten', stream: 's1' }] }, alle);
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

    it('meldet einen absoluten Pfad in homeFiles', () => {
      failsWith({ title: 'T', fixtures: { homeFiles: { '/etc/passwd': 'x' } }, steps: GOOD.steps }, /homeFiles\["\/etc\/passwd"\]: der Pfad muss relativ/);
    });

    it('meldet einen Pfad in homeFiles, der aus dem Home hinausführt', () => {
      failsWith({ title: 'T', fixtures: { homeFiles: { 'dispatch/../../x': 'x' } }, steps: GOOD.steps }, /ohne „\.\.“/);
    });

    it('meldet einen Pfad in homeFiles mit umgekehrtem Schrägstrich', () => {
      failsWith({ title: 'T', fixtures: { homeFiles: { 'dispatch\\state.json': 'x' } }, steps: GOOD.steps }, /mit \/ getrennt/);
    });

    it('meldet einen Inhalt in homeFiles, der kein Text und kein JSON-Wert ist', () => {
      failsWith({ title: 'T', fixtures: { homeFiles: { 'dispatch/state.json': 7 } }, steps: GOOD.steps }, /muss ein Text oder ein JSON-Wert/);
    });

    it('meldet ein unbekanntes Betriebssystem', () => {
      failsWith({ title: 'T', platforms: ['macos'], steps: GOOD.steps }, /platforms muss eine Liste/);
    });

    it('meldet ein Feld, das es bei dieser Art nicht gibt', () => {
      failsWith(
        { title: 'T', steps: [{ name: 'a', kind: 'sseOpen', as: 's1', path: '/api/events', method: 'GET' }] },
        /unbekannte Felder in steps\[0\]: method/,
      );
    });

    it('meldet ein Feld eines Ereignisstroms an einem HTTP-Schritt', () => {
      failsWith({ title: 'T', steps: [{ name: 'a', method: 'GET', path: '/api/health', stream: 's1' }] }, /unbekannte Felder in steps\[0\]: stream/);
    });

    it('meldet eine Umgebungsvariable außerhalb von CONTRACT_', () => {
      failsWith({ title: 'T', env: { PATH: '/bin' }, steps: GOOD.steps }, /env\["PATH"\]: ein Szenario darf nur CONTRACT_\*-Variablen setzen/);
    });

    it('meldet eine Umgebungsvariable, die kein Text ist', () => {
      failsWith({ title: 'T', env: { CONTRACT_HEARTBEAT_MS: 300 }, steps: GOOD.steps }, /env\["CONTRACT_HEARTBEAT_MS"\] muss ein Text sein/);
    });

    it('meldet einen Kopf, der kein Text und nicht null ist', () => {
      failsWith({ title: 'T', steps: [{ name: 'a', method: 'GET', path: '/api/health', headers: { 'x-harness': 1 } }] }, /headers\["x-harness"\] muss ein Text oder null sein/);
    });
  });
});
