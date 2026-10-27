import { describe, expect, it } from 'vitest';
import { EXPECTED_SCHEMA_VERSION, REQUIRED_SCHEMA_FEATURES, SchemaVersionGuard } from '../src/SchemaVersionGuard.js';

describe('SchemaVersionGuard', () => {
  const guard = new SchemaVersionGuard();
  const features = Object.keys(REQUIRED_SCHEMA_FEATURES);

  it('akzeptiert die erwartete Version mit allen Erweiterungen', () => {
    expect(guard.check(EXPECTED_SCHEMA_VERSION, 'AiHarness', features)).toEqual([]);
  });

  it('nennt das Startskript, wenn das Schema fehlt', () => {
    expect(() => guard.check(null, 'AiHarness', features)).toThrow(expect.objectContaining({ code: 'board-schema-missing', message: expect.stringContaining('001_initial.sql') }));
  });

  it('bleibt bei Schema-Version 3, damit ältere Stände mit der erweiterten DB weiterlaufen', () => {
    expect(EXPECTED_SCHEMA_VERSION).toBe(3);
    expect(REQUIRED_SCHEMA_FEATURES).toEqual({ 'dispatch-runs': '004_dispatch_runs.sql', 'note-structure': '005_note_structure.sql', 'dispatch-run-details': '006_dispatch_run_details.sql', 'target-branch': '007_target_branch.sql' });
  });

  it('nennt das nächste Skript bei veralteter DB', () => {
    expect(() => guard.check(2, 'AiHarness', features)).toThrow(expect.objectContaining({ code: 'board-schema-outdated', message: expect.stringContaining('003_') }));
    expect(() => guard.check(1, 'AiHarness', features)).toThrow(expect.objectContaining({ code: 'board-schema-outdated', message: expect.stringContaining('002_') }));
  });

  it('meldet fehlende Erweiterungen, statt den ganzen Zugriff zu sperren', () => {
    expect(guard.check(3, 'AiHarness', [])).toEqual(['dispatch-runs', 'note-structure', 'dispatch-run-details', 'target-branch']);
    expect(guard.check(3, 'AiHarness', ['dispatch-runs'])).toEqual(['note-structure', 'dispatch-run-details', 'target-branch']);
    expect(guard.check(3, 'AiHarness', ['dispatch-runs', 'note-structure'])).toEqual(['dispatch-run-details', 'target-branch']);
    expect(guard.check(3, 'AiHarness', [...features, 'unbekannt'])).toEqual([]);
  });

  it('nennt im Meldungstext eines gesperrten Zugriffs Datenbank und Skript', () => {
    expect(SchemaVersionGuard.featureMessage('note-structure', 'AiHarness')).toContain('packages/db/sql/005_note_structure.sql');
    expect(SchemaVersionGuard.featureMessage('note-structure', 'AiHarness')).toContain('AiHarness');
    expect(SchemaVersionGuard.featureMessage('dispatch-runs', 'AiHarness')).toContain('004_dispatch_runs.sql');
    expect(SchemaVersionGuard.featureMessage('dispatch-run-details', 'AiHarness')).toContain('packages/db/sql/006_dispatch_run_details.sql');
    expect(SchemaVersionGuard.script('note-structure')).toBe('005_note_structure.sql');
    expect(SchemaVersionGuard.featureMessage('target-branch', 'AiHarness')).toContain('packages/db/sql/007_target_branch.sql');
  });

  it('verlangt ein Update des Harness bei neuerer DB', () => {
    expect(() => guard.check(EXPECTED_SCHEMA_VERSION + 1, 'AiHarness', features)).toThrow(expect.objectContaining({ code: 'board-schema-newer', message: expect.stringContaining('npm run setup') }));
  });

  // Liefert den Meldungstext des geworfenen Fehlers, um Textbestandteile ausschließen zu können
  const message = (run: () => void): string => {
    try {
      run();
    } catch (error) {
      return (error as Error).message;
    }
    throw new Error('erwarteter Fehler blieb aus');
  };

  describe('bei SQLite ohne Hinweis auf die SQL-Skripte', () => {
    const file = 'C:/x/board.db';

    it('akzeptiert die erwartete Version mit allen Erweiterungen', () => {
      expect(guard.check(EXPECTED_SCHEMA_VERSION, file, features, 'sqlite')).toEqual([]);
    });

    it('nennt die Datei, wenn das Schema fehlt', () => {
      expect(() => guard.check(null, file, features, 'sqlite')).toThrow(expect.objectContaining({
        code: 'board-schema-missing', message: expect.stringContaining(file),
      }));
      expect(message(() => guard.check(null, file, features, 'sqlite'))).not.toContain('packages/db/sql');
    });

    it('verweist bei veralteter Datei auf den Neustart', () => {
      expect(() => guard.check(2, file, features, 'sqlite')).toThrow(expect.objectContaining({
        code: 'board-schema-outdated', message: expect.stringContaining('neu starten'),
      }));
      expect(message(() => guard.check(2, file, features, 'sqlite'))).not.toContain('packages/db/sql');
    });

    it('nennt die fehlende Erweiterung ohne Skriptnamen', () => {
      expect(guard.check(EXPECTED_SCHEMA_VERSION, file, [], 'sqlite')).toEqual(['dispatch-runs', 'note-structure', 'dispatch-run-details', 'target-branch']);
      expect(SchemaVersionGuard.featureMessage('dispatch-runs', file, 'sqlite')).toContain('dispatch-runs');
      expect(SchemaVersionGuard.featureMessage('dispatch-runs', file, 'sqlite')).not.toContain('004_');
    });

    it('verlangt ein Update des Harness bei neuerer Datei', () => {
      expect(() => guard.check(EXPECTED_SCHEMA_VERSION + 1, file, features, 'sqlite')).toThrow(expect.objectContaining({
        code: 'board-schema-newer', message: expect.stringContaining('npm run setup'),
      }));
    });
  });
});
