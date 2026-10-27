import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { MssqlSchemaRunner, type SchemaBatchRunner } from '../src/MssqlSchemaRunner.js';

// Fehler, wie mssql sie liefert: Meldung plus Nummer des SQL-Servers
const sqlError = (message: string, number?: number): Error => Object.assign(new Error(message), { number });

// Pool-Ersatz: merkt sich die ausgeführten Batches und wirft, was der Test vorgibt
const pool = (fail: (text: string) => Error | null = () => null) => {
  const batches: string[] = [];
  const warnings: string[] = [];
  const runner: SchemaBatchRunner = {
    request: () => ({
      batch: async (text: string) => {
        batches.push(text);
        const error = fail(text);
        if (error) throw error;
      },
    }),
  };
  return { runner, batches, warnings, warn: (m: string) => warnings.push(m) };
};

describe('MssqlSchemaRunner', () => {
  it('spielt die fehlenden Erweiterungen in Skript-Reihenfolge ein', async () => {
    const p = pool();
    const applied = await new MssqlSchemaRunner(p.runner, p.warn).apply(['note-structure', 'dispatch-runs']);
    expect(applied).toEqual(['dispatch-runs', 'note-structure']);
    expect(p.batches).toHaveLength(2);
    expect(p.batches[0]).toContain('CREATE TABLE board.DispatchRuns (');
    expect(p.batches[1]).toContain('ALTER TABLE board.Notes ADD Title');
  });

  it('lässt eingespielte Erweiterungen aus und führt ohne fehlende nichts aus', async () => {
    const p = pool();
    expect(await new MssqlSchemaRunner(p.runner, p.warn).apply(['note-structure'])).toEqual(['note-structure']);
    expect(p.batches).toHaveLength(1);
    expect(await new MssqlSchemaRunner(p.runner, p.warn).apply([])).toEqual([]);
    expect(p.batches).toHaveLength(1);
  });

  it('schickt den Skripttext aus packages/db/sql ohne das abschließende GO', async () => {
    const p = pool();
    await new MssqlSchemaRunner(p.runner, p.warn).apply(['note-structure']);
    const original = readFileSync(fileURLToPath(new URL('../sql/005_note_structure.sql', import.meta.url)), 'utf8');
    expect(p.batches[0]).toContain('COMMIT TRANSACTION;');
    expect(p.batches[0]).not.toMatch(/^\s*GO\s*$/m);
    expect(original).toMatch(/^GO\s*$/m);
  });

  it('wertet „schon eingespielt“ (50002) als Erfolg, ohne zu warnen', async () => {
    const p = pool(() => sqlError('Erweiterung note-structure ist bereits eingespielt.', 50002));
    expect(await new MssqlSchemaRunner(p.runner, p.warn).apply(['note-structure'])).toEqual(['note-structure']);
    expect(p.warnings).toEqual([]);
  });

  it('wirft bei fehlendem Recht nicht, sondern warnt mit dem Skriptnamen', async () => {
    for (const error of [sqlError('Keine Berechtigung', 15247), sqlError('ALTER permission was denied on object Notes')]) {
      const p = pool(() => error);
      expect(await new MssqlSchemaRunner(p.runner, p.warn).apply(['note-structure'])).toEqual([]);
      expect(p.warnings).toHaveLength(1);
      expect(p.warnings[0]).toContain('005_note_structure.sql');
      expect(p.warnings[0]).toContain('ALTER');
    }
  });

  it('wirft auch bei fehlender Vorbedingung und unbekannten Fehlern nicht', async () => {
    const p = pool((text) => (text.includes('DispatchRuns') ? sqlError('Verbindung weg') : sqlError('Erweiterung dispatch-runs fehlt', 50001)));
    expect(await new MssqlSchemaRunner(p.runner, p.warn).apply(['dispatch-runs', 'note-structure'])).toEqual([]);
    expect(p.warnings).toHaveLength(2);
    expect(p.warnings[0]).toContain('Verbindung weg');
    expect(p.warnings[1]).toContain('Vorbedingung');
  });

  it('überspringt eine Erweiterung ohne Skriptdatei', async () => {
    const p = pool();
    expect(await new MssqlSchemaRunner(p.runner, p.warn).apply(['unbekannt'])).toEqual([]);
    expect(p.batches).toEqual([]);
  });
});
