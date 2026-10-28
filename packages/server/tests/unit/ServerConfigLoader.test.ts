import { afterEach, describe, expect, it } from 'vitest';
import { dirname, join } from 'node:path';
import { DEFAULT_PORT, ServerConfigLoader } from '../../src/config/ServerConfigLoader.js';

// Einstellungen des Servers aus Argumenten und Umgebung, mit und ohne gebündeltes Sidecar (SEA).
describe('ServerConfigLoader', () => {
  const loader = new ServerConfigLoader(false);
  const sidecar = new ServerConfigLoader(true);
  const beside = (name: string) => join(dirname(process.execPath), name);

  afterEach(() => {
    delete process.env.HARNESS_BUNDLED_VERSION;
  });

  it('nimmt den Port aus --port, sonst aus HARNESS_PORT, sonst den Standard', () => {
    expect(loader.load(['--port', '4800'], { HARNESS_PORT: '4900' }).port).toBe(4800);
    expect(loader.load([], { HARNESS_PORT: '4900' }).port).toBe(4900);
    expect(loader.load([], {}).port).toBe(DEFAULT_PORT);
  });

  it.each(['abc', '-1', '65536', '1.5'])('weist den Port %j ab', (raw) => {
    expect(() => loader.load(['--port', raw], {})).toThrowError(expect.objectContaining({ code: 'port-invalid' }));
  });

  it('nimmt den Ordner der Oberfläche aus HARNESS_WEB_DIR', () => {
    expect(loader.load([], { HARNESS_WEB_DIR: '/wo/web' }).webDir).toBe('/wo/web');
  });

  it('nimmt den Ordner der Benachrichtigungsbilder aus HARNESS_TOAST_DIR', () => {
    expect(loader.load([], { HARNESS_TOAST_DIR: '/wo/toast' }).toastDir).toBe('/wo/toast');
    expect(sidecar.load([], { HARNESS_TOAST_DIR: '/wo/toast' }).toastDir).toBe('/wo/toast');
  });

  it('sucht Oberfläche und Bilder im Sidecar neben dem Programm', () => {
    const config = sidecar.load([], {});
    expect(config.webDir).toBe(beside('web'));
    expect(config.toastDir).toBe(beside('toast'));
  });

  it('sucht Oberfläche und Bilder ohne Sidecar in den Paketordnern', () => {
    const config = loader.load([], {});
    expect(config.webDir).not.toBe(beside('web'));
    expect(config.webDir).toContain('web');
    expect(config.toastDir).toContain('toast');
  });

  it('nimmt die beim Bündeln gesetzte Version, wenn es eine gibt', () => {
    process.env.HARNESS_BUNDLED_VERSION = '9.8.7';
    expect(loader.load([], {}).version).toBe('9.8.7');
    expect(sidecar.load([], {}).version).toBe('9.8.7');
  });

  it('nimmt sonst die Version des Serverpakets, nie 0.0.0', () => {
    expect(loader.load([], {}).version).toMatch(/^\d+\.\d+\.\d+/);
    expect(loader.load([], {}).version).not.toBe('0.0.0');
  });

  it('nimmt HARNESS_HOME für den Benutzerordner', () => {
    expect(loader.load([], { HARNESS_HOME: '/wo/home' }).home.dir).toBe('/wo/home');
  });

  it('läuft nur mit HARNESS_SIDECAR=1 als Sidecar', () => {
    expect(loader.load([], {}).sidecar).toBe(false);
    expect(loader.load([], { HARNESS_SIDECAR: '1' }).sidecar).toBe(true);
    expect(loader.load([], { HARNESS_SIDECAR: '0' }).sidecar).toBe(false);
    expect(loader.load([], { HARNESS_SIDECAR: '' }).sidecar).toBe(false);
  });

  it('nimmt den öffentlichen Port aus HARNESS_PUBLIC_PORT', () => {
    expect(loader.load([], {}).publicPort).toBeUndefined();
    expect(loader.load([], { HARNESS_PUBLIC_PORT: '' }).publicPort).toBeUndefined();
    expect(loader.load(['--port', '0'], { HARNESS_PUBLIC_PORT: '4710' }).publicPort).toBe(4710);
  });

  it.each(['abc', '-1', '65536', '1.5'])('weist den öffentlichen Port %j ab', (raw) => {
    expect(() => loader.load([], { HARNESS_PUBLIC_PORT: raw })).toThrowError(
      expect.objectContaining({ code: 'port-invalid' }),
    );
  });

  it('schickt Links in Meldungen auf den öffentlichen Port', () => {
    const hinter = loader.load(['--port', '0'], { HARNESS_SIDECAR: '1', HARNESS_PUBLIC_PORT: '4710' });
    expect(ServerConfigLoader.noticeBaseUrl({ ...hinter, port: 51234 })).toBe('http://127.0.0.1:4710');
    expect(ServerConfigLoader.noticeBaseUrl(loader.load(['--port', '4800'], {}))).toBe('http://127.0.0.1:4800');
  });
});
