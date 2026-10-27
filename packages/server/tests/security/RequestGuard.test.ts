import { describe, it, expect } from 'vitest';
import { RequestGuard } from '../../src/security/RequestGuard.js';

describe('RequestGuard', () => {
  const guard = new RequestGuard(4710);
  const ok = { 'content-type': 'application/json', 'x-harness': '1' };

  it('lässt Lesezugriffe mit erlaubtem Host zu', () => {
    expect(guard.check('GET', { host: '127.0.0.1:4710' })).toBeNull();
    expect(guard.check('GET', { host: 'localhost:4710' })).toBeNull();
    expect(guard.check('HEAD', { host: 'localhost:4710' })).toBeNull();
  });

  it('lehnt fremde Hostnamen ab (Schutz vor DNS-Rebinding)', () => {
    expect(guard.check('GET', { host: 'evil.example' })?.code).toBe('host-denied');
    expect(guard.check('GET', { host: 'evil.example:4710' })?.code).toBe('host-denied');
    expect(guard.check('GET', { host: '127.0.0.1:9999' })?.code).toBe('host-denied');
    expect(guard.check('GET', {})?.code).toBe('host-denied');
  });

  it('lehnt eine fremde Herkunft ab, auch bei Lesezugriffen', () => {
    const base = { host: '127.0.0.1:4710' };
    expect(guard.check('GET', { ...base, origin: 'http://evil.example' })?.code).toBe('origin-denied');
    expect(guard.check('GET', { ...base, origin: 'null' })?.code).toBe('origin-denied');
    expect(guard.check('GET', { ...base, origin: 'http://127.0.0.1:4711' })?.code).toBe('origin-denied');
  });

  it('akzeptiert die eigene Herkunft', () => {
    const base = { host: 'localhost:4710', ...ok };
    expect(guard.check('POST', { ...base, origin: 'http://localhost:4710' })).toBeNull();
    expect(guard.check('PATCH', { ...base, origin: 'http://127.0.0.1:4710' })).toBeNull();
  });

  it('verlangt bei Schreibzugriffen den Pflichtkopf und JSON', () => {
    const host = '127.0.0.1:4710';
    expect(guard.check('POST', { host })?.code).toBe('header-missing');
    expect(guard.check('POST', { host, 'content-type': 'application/json' })?.code).toBe('header-missing');
    expect(guard.check('POST', { host, 'x-harness': '1' })?.code).toBe('header-missing');
    expect(guard.check('DELETE', { host, 'x-harness': '1', 'content-type': 'text/plain' })?.code).toBe('header-missing');
    expect(guard.check('POST', { host, ...ok })).toBeNull();
    expect(guard.check('POST', { host, 'x-harness': '1', 'content-type': 'application/json; charset=utf-8' })).toBeNull();
  });

  it('übernimmt einen später bekannten Port', () => {
    const dynamic = new RequestGuard(0);
    dynamic.allowPort(5555);
    expect(dynamic.check('GET', { host: '127.0.0.1:5555' })).toBeNull();
  });
});
