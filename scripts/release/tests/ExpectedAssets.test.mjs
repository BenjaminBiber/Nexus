import { describe, expect, it } from 'vitest';
import { assetNames, baseAssetNames, missingAssets, updaterAssetNames, updaterEnabled } from '../lib/ExpectedAssets.mjs';

describe('ExpectedAssets', () => {
  it('listet die Installer und CLI-Archive je Plattform', () => {
    expect(baseAssetNames('1.2.3')).toEqual([
      'Nexus_1.2.3_aarch64.dmg',
      'Nexus_1.2.3_x64.dmg',
      'Nexus_1.2.3_x64-setup.exe',
      'Nexus_1.2.3_amd64.AppImage',
      'Nexus_1.2.3_amd64.deb',
      'Nexus-1.2.3-1.x86_64.rpm',
      'nexus-cli-1.2.3-aarch64-apple-darwin.tar.gz',
      'nexus-cli-1.2.3-x86_64-apple-darwin.tar.gz',
      'nexus-cli-1.2.3-x86_64-pc-windows-msvc.zip',
      'nexus-cli-1.2.3-x86_64-unknown-linux-gnu.tar.gz',
    ]);
  });

  it('listet die zusätzlichen Updater-Dateien mit latest.json und Signaturen', () => {
    expect(updaterAssetNames('1.2.3')).toEqual([
      'latest.json',
      'Nexus_aarch64.app.tar.gz',
      'Nexus_aarch64.app.tar.gz.sig',
      'Nexus_x64.app.tar.gz',
      'Nexus_x64.app.tar.gz.sig',
      'Nexus_1.2.3_x64-setup.exe.sig',
      'Nexus_1.2.3_amd64.AppImage.sig',
    ]);
  });

  it('erkennt den aktivierten Updater an bundle.createUpdaterArtifacts', () => {
    expect(updaterEnabled({ bundle: { createUpdaterArtifacts: true } })).toBe(true);
    expect(updaterEnabled({ bundle: { createUpdaterArtifacts: false } })).toBe(false);
    expect(updaterEnabled({ bundle: {} })).toBe(false);
    expect(updaterEnabled({})).toBe(false);
  });

  it('liefert ohne Updater nur die Grundliste', () => {
    expect(assetNames('1.2.3')).toEqual(baseAssetNames('1.2.3'));
    expect(assetNames('1.2.3', { updater: false })).toEqual(baseAssetNames('1.2.3'));
  });

  it('liefert mit Updater Grundliste und Updater-Dateien zusammen', () => {
    expect(assetNames('1.2.3', { updater: true })).toEqual([...baseAssetNames('1.2.3'), ...updaterAssetNames('1.2.3')]);
  });

  it('meldet Dateien aus expected, die in actual fehlen', () => {
    const expected = ['a', 'b', 'c'];
    expect(missingAssets(expected, ['a', 'c'])).toEqual(['b']);
    expect(missingAssets(expected, ['a', 'b', 'c'])).toEqual([]);
    expect(missingAssets(expected, [])).toEqual(expected);
  });
});
