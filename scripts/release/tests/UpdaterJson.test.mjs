import { describe, expect, it } from 'vitest';
import { updaterAssetNames } from '../lib/ExpectedAssets.mjs';
import { updaterJson, updaterPlatforms } from '../lib/UpdaterJson.mjs';

const SIGNATURES = {
  'Nexus_aarch64.app.tar.gz': 'c2lnLW1hYw==\n',
  'Nexus_1.2.3_x64-setup.exe': 'c2lnLXdpbg==',
  'Nexus_1.2.3_amd64.AppImage': 'c2lnLWxpbnV4',
};

describe('UpdaterJson', () => {
  it('zeigt nur auf Dateien, deren Signatur am Release hängt', () => {
    const signed = updaterAssetNames('1.2.3').filter((name) => name.endsWith('.sig')).map((name) => name.slice(0, -4));
    expect(new Set(Object.values(updaterPlatforms('1.2.3')))).toEqual(new Set(signed));
  });

  it('baut je Plattform Signatur und Adresse am Release', () => {
    const json = updaterJson({
      version: '1.2.3', tag: 'v1.2.3', repo: 'BenjaminBiber/Nexus', notes: ' Neu \n', pubDate: '2026-11-05T00:00:00.000Z', signatures: SIGNATURES,
    });
    expect(json.version).toBe('1.2.3');
    expect(json.notes).toBe('Neu');
    expect(json.pub_date).toBe('2026-11-05T00:00:00.000Z');
    expect(Object.keys(json.platforms).sort()).toEqual([
      'darwin-aarch64', 'darwin-aarch64-app', 'linux-x86_64', 'linux-x86_64-appimage', 'windows-x86_64', 'windows-x86_64-nsis',
    ]);
    expect(json.platforms['darwin-aarch64']).toEqual({
      signature: 'c2lnLW1hYw==',
      url: 'https://github.com/BenjaminBiber/Nexus/releases/download/v1.2.3/Nexus_aarch64.app.tar.gz',
    });
    expect(json.platforms['windows-x86_64-nsis'].url).toBe('https://github.com/BenjaminBiber/Nexus/releases/download/v1.2.3/Nexus_1.2.3_x64-setup.exe');
  });

  it('bricht ab, wenn eine Signatur fehlt', () => {
    const { 'Nexus_1.2.3_amd64.AppImage': _, ...ohneLinux } = SIGNATURES;
    expect(() => updaterJson({ version: '1.2.3', tag: 'v1.2.3', repo: 'a/b', pubDate: 'x', signatures: ohneLinux })).toThrow(
      /Nexus_1\.2\.3_amd64\.AppImage\.sig/,
    );
  });
});
