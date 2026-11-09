import { describe, expect, it } from 'vitest';
import { inspectThemePackage, ThemePackageReadError } from '../src/themes/inspectPackage.js';
import { buildZip, testManifest } from './helpers/zip.js';

const blob = (bytes: Uint8Array<ArrayBuffer>) => new Blob([bytes]);

describe('inspectThemePackage', () => {
  it('liest das Manifest und erkennt main.js (gespeichert)', async () => {
    const m = testManifest({ permissions: { network: ['fonts.example.com'] } });
    const result = await inspectThemePackage(blob(await buildZip({ 'manifest.json': JSON.stringify(m), 'theme.css': '', 'main.js': 'x' })));
    expect(result.hasCode).toBe(true);
    expect(result.manifest.name).toBe('Ocean');
    expect(result.manifest.permissions.network).toEqual(['fonts.example.com']);
  });

  it('entpackt deflate und meldet ohne main.js keinen Code', async () => {
    const result = await inspectThemePackage(blob(await buildZip({ 'manifest.json': JSON.stringify(testManifest()), 'assets/main.js': 'x' }, 8)));
    expect(result.hasCode).toBe(false);
    expect(result.manifest.author).toBe('Ada');
  });

  it('ergänzt fehlende Domains als leere Liste', async () => {
    const { permissions: _, ...rest } = testManifest();
    const result = await inspectThemePackage(blob(await buildZip({ 'manifest.json': JSON.stringify(rest) })));
    expect(result.manifest.permissions.network).toEqual([]);
  });

  it('meldet kein ZIP, fehlendes Manifest und kaputtes JSON verständlich', async () => {
    await expect(inspectThemePackage(new Blob(['kein zip']))).rejects.toThrow(new ThemePackageReadError('Die Datei ist kein gültiges Theme-Paket (kein ZIP-Archiv).'));
    await expect(inspectThemePackage(blob(await buildZip({ 'theme.css': '' })))).rejects.toThrow('Die Datei ist kein gültiges Theme-Paket: manifest.json fehlt.');
    await expect(inspectThemePackage(blob(await buildZip({ 'manifest.json': '{kaputt' })))).rejects.toThrow('Die manifest.json des Themes ist kein gültiges JSON.');
  });
});
