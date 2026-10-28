import { describe, expect, it } from 'vitest';
import { Normalizer } from '../lib/Normalizer.mjs';

const TODAY = '2026-10-27';
const STABLE_UUID = '11111111-1111-1111-1111-111111111111';

// Normalisierer mit festem heutigen Datum, damit die Fälle unabhängig vom Tag gelten
const make = (context = {}) => new Normalizer({ today: TODAY, ...context });

describe('Normalizer', () => {
  describe('1. gespeicherte Werte', () => {
    it('ersetzt einen gespeicherten Wert durch <var:name>', () => {
      const normalizer = make({ saved: { pid: 'a1b2c3d4' } });
      expect(normalizer.savedValues('Projekt a1b2c3d4 gelöscht')).toBe('Projekt <var:pid> gelöscht');
    });

    it('lässt kurze Werte in Ruhe, damit nicht überall ersetzt wird', () => {
      const normalizer = make({ saved: { n: '7' } });
      expect(normalizer.savedValues('7 von 17')).toBe('7 von 17');
    });

    it('ersetzt mehrere Vorkommen und mehrere Werte', () => {
      const normalizer = make({ saved: { pid: 'a1b2c3d4', ticket: 'BEN-0001' } });
      expect(normalizer.savedValues('a1b2c3d4/BEN-0001/a1b2c3d4')).toBe('<var:pid>/<var:ticket>/<var:pid>');
    });
  });

  describe('2. Pfade', () => {
    it('ersetzt Repo-Pfad, Home und Temp-Ordner', () => {
      const normalizer = make({ paths: { REPO1: '/tmp/repos/r1', HOME: '/tmp/home-1', TMP: '/tmp' } });
      expect(normalizer.paths('/tmp/repos/r1/docs/a.md')).toBe('<REPO1>/docs/a.md');
      expect(normalizer.paths('/tmp/home-1/board.yaml')).toBe('<HOME>/board.yaml');
      expect(normalizer.paths('/tmp/sonst')).toBe('<TMP>/sonst');
    });

    it('nimmt den längsten Pfad zuerst, auch wenn Home im Temp-Ordner liegt', () => {
      const normalizer = make({ paths: { TMP: '/tmp', HOME: '/tmp/home-1' } });
      expect(normalizer.paths('/tmp/home-1/server.json')).toBe('<HOME>/server.json');
    });

    it('erkennt Windows-Pfade in allen drei Schreibweisen', () => {
      const normalizer = make({ paths: { HOME: 'C:\\Users\\t\\.harness' } });
      expect(normalizer.paths('C:\\Users\\t\\.harness\\board.yaml')).toBe('<HOME>/board.yaml');
      expect(normalizer.paths('C:/Users/t/.harness/board.yaml')).toBe('<HOME>/board.yaml');
      expect(normalizer.paths('C:\\\\Users\\\\t\\\\.harness\\\\board.yaml')).toBe('<HOME>/board.yaml');
    });

    it('vereinheitlicht gemischte Trenner hinter dem Platzhalter', () => {
      const normalizer = make({ paths: { REPO1: 'C:\\Repos\\a' } });
      expect(normalizer.paths('C:\\Repos\\a/.git\\harness')).toBe('<REPO1>/.git/harness');
    });

    it('lässt einen Satz um den Pfad herum stehen', () => {
      const normalizer = make({ paths: { HOME: '/tmp/home-1' } });
      expect(normalizer.paths('Datei geöffnet (/tmp/home-1/board.db).')).toBe('Datei geöffnet (<HOME>/board.db).');
    });
  });

  describe('3. Ports', () => {
    it('ersetzt den Port des Servers hinter 127.0.0.1 und localhost', () => {
      const normalizer = make({ port: 52744 });
      expect(normalizer.ports('http://127.0.0.1:52744/api/health')).toBe('http://127.0.0.1:<PORT>/api/health');
      expect(normalizer.ports('localhost:52744')).toBe('localhost:<PORT>');
    });

    it('lässt andere Ports in Ruhe', () => {
      expect(make({ port: 52744 }).ports('127.0.0.1:4710')).toBe('127.0.0.1:4710');
    });
  });

  describe('4. Zeit und Datum', () => {
    it('ersetzt Zeitstempel mit Z und mit Zonenangabe', () => {
      expect(make().times('2026-10-27T18:05:00.123Z')).toBe('<TIME>');
      expect(make().times('2026-10-27T18:05:00+02:00')).toBe('<TIME>');
      expect(make().times('von 2026-01-02T03:04:05Z bis 2026-01-02T03:04:06Z')).toBe('von <TIME> bis <TIME>');
    });

    it('ersetzt nur das heutige Datum', () => {
      expect(make().times(TODAY)).toBe('<DATE>');
      expect(make().times('2020-01-01')).toBe('2020-01-01');
    });

    it('ersetzt Datum mit Uhrzeit in deutscher Schreibweise – so gibt die CLI Kommentare aus', () => {
      expect(make().times('- 27.10.2026, 18:05, Vertrag: Hallo')).toBe('- <TIME>, Vertrag: Hallo');
      expect(make().times('27.10.2026, 18:05:09')).toBe('<TIME>');
    });

    it('lässt ein deutsches Datum ohne Uhrzeit stehen', () => {
      expect(make().times('27.10.2026')).toBe('27.10.2026');
    });
  });

  describe('5. Datum relativ zu heute', () => {
    it('ersetzt einen Tag im letzten Jahr durch seinen Abstand zu heute', () => {
      expect(make().relativeDates('2026-09-02')).toBe('<DATE-55>');
      expect(make().relativeDates('2026-10-26')).toBe('<DATE-1>');
      expect(make().relativeDates('von 2026-09-02 bis 2026-09-09')).toBe('von <DATE-55> bis <DATE-48>');
    });

    it('lässt heute, die Zukunft und weiter zurückliegende Tage in Ruhe', () => {
      // heute wird schon in Regel 4 zu <DATE>
      expect(make().relativeDates(TODAY)).toBe(TODAY);
      expect(make().relativeDates('2026-12-24')).toBe('2026-12-24');
      expect(make().relativeDates('2020-03-02')).toBe('2020-03-02');
    });

    it('lässt feste Daten der Szenarien ab einem Jahr Abstand stehen', () => {
      expect(make().relativeDates('2025-10-27')).toBe('<DATE-365>');
      expect(make().relativeDates('2025-10-26')).toBe('<DATE-366>');
      expect(make().relativeDates('2025-10-25')).toBe('2025-10-25');
    });

    it('greift nicht in längere Zahlenfolgen', () => {
      expect(make().relativeDates('12026-09-02')).toBe('12026-09-02');
      expect(make().relativeDates('2026-09-021')).toBe('2026-09-021');
    });
  });

  describe('6. UUIDs', () => {
    it('ersetzt UUIDs', () => {
      expect(make().uuids('a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a11')).toBe('<UUID>');
    });

    it('lässt die feste GUID der Szenarien stehen', () => {
      expect(make().uuids(STABLE_UUID)).toBe(STABLE_UUID);
    });
  });

  describe('7. Git-Commits', () => {
    it('ersetzt lange und kurze Commits in sha, commit und head', () => {
      expect(make().shas('3f2a9b1c4d5e6f708192a3b4c5d6e7f809a1b2c3', 'sha')).toBe('<SHA>');
      expect(make().shas('3f2a9b1', 'commit')).toBe('<SHA>');
      expect(make().shas('3f2a9b1c4d5e', 'head')).toBe('<SHA>');
    });

    it('lässt andere Felder in Ruhe', () => {
      expect(make().shas('3f2a9b1', 'name')).toBe('3f2a9b1');
    });
  });

  describe('8. Zahlenfelder', () => {
    it('setzt Felder auf Ms und At sowie pid, port und durationMs auf 0', () => {
      const normalizer = make();
      expect(normalizer.number(15000, 'heartbeatMs')).toBe(0);
      expect(normalizer.number(1793000000000, 'startedAt')).toBe(0);
      expect(normalizer.number(42, 'durationMs')).toBe(0);
      expect(normalizer.number(4711, 'pid')).toBe(0);
      expect(normalizer.number(1433, 'port')).toBe(0);
    });

    it('lässt andere Zahlen in Ruhe', () => {
      expect(make().number(7, 'count')).toBe(7);
    });
  });

  describe('value über ganze Antworten', () => {
    it('läuft durch Objekte, Listen und Schlüssel', () => {
      const normalizer = make({ paths: { REPO1: '/tmp/repos/r1' }, port: 52744, saved: { pid: 'a1b2c3d4' } });
      const value = normalizer.value({
        '/tmp/repos/r1/docs': 'da',
        items: [{ id: 'a1b2c3d4', path: '/tmp/repos/r1', sha: '3f2a9b1c4d5e6f708192a3b4c5d6e7f809a1b2c3' }],
        startedAt: 1793000000000,
        url: 'http://127.0.0.1:52744/api/health',
        when: '2026-10-27T18:05:00Z',
        count: 1,
      });
      expect(value).toEqual({
        '<REPO1>/docs': 'da',
        items: [{ id: '<var:pid>', path: '<REPO1>', sha: '<SHA>' }],
        startedAt: 0,
        url: 'http://127.0.0.1:<PORT>/api/health',
        when: '<TIME>',
        count: 1,
      });
    });

    it('lässt null, true und Zahlen ohne besonderen Namen unverändert', () => {
      expect(make().value({ a: null, b: true, c: 3 })).toEqual({ a: null, b: true, c: 3 });
    });

    it('hält die Reihenfolge der Regeln ein: ein gespeicherter Pfad wird <var:…>', () => {
      const normalizer = make({ paths: { REPO1: '/tmp/repos/r1' }, saved: { ziel: '/tmp/repos/r1' } });
      expect(normalizer.text('/tmp/repos/r1')).toBe('<var:ziel>');
    });
  });
});
