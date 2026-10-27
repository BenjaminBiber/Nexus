import { describe, expect, it } from 'vitest';
import { HarnessError } from '@harness/core';
import { ErrorMapper } from '../../src/errors/ErrorMapper.js';

describe('ErrorMapper', () => {
  const mapper = new ErrorMapper();
  const status = (code: string) => mapper.map(new HarnessError(code, 'Meldung')).status;

  it('meldet eine nicht verfügbare Board-Datenbank mit 503 samt Code und Meldung', () => {
    for (const code of ['board-config-missing', 'board-config-broken', 'board-db-unreachable', 'board-db-overloaded', 'board-db-storage', 'board-schema-missing', 'board-schema-outdated', 'board-schema-newer']) {
      expect(status(code), code).toBe(503);
    }
    expect(mapper.map(new HarnessError('board-db-unreachable', 'nicht erreichbar')).body).toEqual({ code: 'board-db-unreachable', message: 'nicht erreichbar' });
  });

  it('ordnet die Docs-Fehler zu', () => {
    expect(status('doc-changed')).toBe(409);
    expect(status('doc-range-invalid')).toBe(400);
    expect(status('doc-not-writable')).toBe(422);
    expect(status('doc-not-found')).toBe(404);
    expect(mapper.map(new HarnessError('doc-changed', 'geändert', { hash: 'abc' })).body).toEqual({ code: 'doc-changed', message: 'geändert', details: { hash: 'abc' } });
  });

  it('bleibt bei 404, 409 und 422', () => {
    expect(status('board-not-found')).toBe(404);
    expect(status('conflict')).toBe(409);
    expect(status('board-config-invalid')).toBe(422);
    expect(status('title-empty')).toBe(422);
  });
});
