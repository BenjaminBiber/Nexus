import { describe, expect, it } from 'vitest';
import { releaseFiles } from '../lib/BundleAssets.mjs';
import { assetNames } from '../lib/ExpectedAssets.mjs';

const TARGETS = ['aarch64-apple-darwin', 'x86_64-pc-windows-msvc', 'x86_64-unknown-linux-gnu'];

describe('BundleAssets', () => {
  it('liefert über alle Ziele genau die erwarteten Dateien des Releases (ohne latest.json)', () => {
    const names = TARGETS.flatMap((target) => releaseFiles('1.2.3', target, { updater: true }).map((file) => file.name));
    const expected = assetNames('1.2.3', { updater: true }).filter((name) => name !== 'latest.json');
    expect([...names].sort()).toEqual([...expected].sort());
  });

  it('lässt ohne Updater die Signaturen und das App-Archiv weg', () => {
    const names = TARGETS.flatMap((target) => releaseFiles('1.2.3', target).map((file) => file.name));
    expect([...names].sort()).toEqual([...assetNames('1.2.3')].sort());
  });

  it('sucht die Installer im Bundle-Ordner des Ziels', () => {
    expect(releaseFiles('1.2.3', 'aarch64-apple-darwin', { updater: true })).toContainEqual({
      path: 'target/aarch64-apple-darwin/release/bundle/macos/Nexus.app.tar.gz',
      name: 'Nexus_aarch64.app.tar.gz',
    });
  });

  it('nimmt das CLI-Archiv aus dist/release', () => {
    expect(releaseFiles('1.2.3', 'x86_64-pc-windows-msvc')).toContainEqual({
      path: 'dist/release/nexus-cli-1.2.3-x86_64-pc-windows-msvc.zip',
      name: 'nexus-cli-1.2.3-x86_64-pc-windows-msvc.zip',
    });
  });

  it('bricht bei einem unbekannten Ziel ab', () => {
    expect(() => releaseFiles('1.2.3', 'x86_64-apple-darwin')).toThrow(/keinen Release-Build/);
  });
});
