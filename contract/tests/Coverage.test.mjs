import { describe, expect, it } from 'vitest';
import { Coverage } from '../lib/Coverage.mjs';

const ROUTES = [
  { method: 'GET', pattern: '/api/health', file: 'HealthRoutes.ts' },
  { method: 'GET', pattern: '/api/projects/:pid/sessions', file: 'SessionRoutes.ts' },
  { method: 'POST', pattern: '/api/projects/:pid/sessions/:sid/claim', file: 'SessionRoutes.ts' },
  { method: 'GET', pattern: '/', file: 'StaticRoutes.ts' },
  // Der Rückfall der Oberfläche: beantwortet alle übrigen Adressen
  { method: 'GET', pattern: '/*', file: 'StaticRoutes.ts' },
  { method: 'GET', pattern: '/api/workflow', file: 'WorkflowRoutes.ts' },
];

// Muster der Routen, die noch keinen Schritt haben
const openPatterns = (coverage) => coverage.open.map((route) => `${route.method} ${route.pattern}`);

describe('Coverage', () => {
  it('ordnet einen festen Pfad seiner Route zu', () => {
    const coverage = new Coverage(ROUTES);
    expect(coverage.add('GET', '/api/health')).toBe(true);
    expect(openPatterns(coverage)).not.toContain('GET /api/health');
  });

  it('ordnet Pfadabschnitte einem Muster mit :param zu und schneidet den Abfrageteil ab', () => {
    const coverage = new Coverage(ROUTES);
    expect(coverage.add('GET', '/api/projects/abc/sessions?filter=offen')).toBe(true);
    expect(coverage.add('POST', '/api/projects/abc/sessions/s1/claim')).toBe(true);
    expect(openPatterns(coverage)).toEqual(['GET /api/health', 'GET /', 'GET /*', 'GET /api/workflow']);
  });

  it('nimmt eine genauere Route, nicht den Rückfall mit *', () => {
    const coverage = new Coverage(ROUTES);
    coverage.add('GET', '/api/workflow');
    coverage.add('GET', '/');
    expect(openPatterns(coverage)).toContain('GET /*');
  });

  it('zählt übrige Adressen auf den Rückfall mit *', () => {
    const coverage = new Coverage(ROUTES);
    expect(coverage.add('GET', '/p/abc/board')).toBe(true);
    expect(coverage.add('GET', '/api/gibtsnicht')).toBe(true);
    expect(coverage.unmatched).toEqual([]);
    expect(openPatterns(coverage)).toContain('GET /');
  });

  it('meldet einen Schritt ohne passende Route', () => {
    const coverage = new Coverage(ROUTES);
    expect(coverage.add('DELETE', '/api/projects/abc')).toBe(false);
    expect(coverage.unmatched).toEqual(['DELETE /api/projects/abc']);
    expect(coverage.report()).toContain('? DELETE /api/projects/abc');
  });

  it('zählt je Quelldatei, wie viele Routen abgedeckt sind', () => {
    const coverage = new Coverage(ROUTES);
    coverage.add('GET', '/api/health');
    expect(coverage.report()).toContain('Abdeckung: 1 von 6 Routen');
    expect(coverage.report()).toContain('HealthRoutes.ts  1/1');
    expect(coverage.report()).toContain('StaticRoutes.ts  0/2');
  });
});
