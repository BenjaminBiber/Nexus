// Was ein Build je Rust-Ziel an das Release hängt: Tauri legt die Installer unter
// target/<ziel>/release/bundle/<art>/ ab, das Release erwartet sie unter den Namen aus ExpectedAssets.mjs.
// Bisher hat tauri-action das übernommen; ohne GitHub Actions (Gitea Actions) erledigt es upload-bundles.mjs.
import { archiveName } from './CliArchive.mjs';

// Dateien je Ziel: `from` relativ zum Bundle-Ordner, `name` wie am Release, `updater` nur mit Updater-Schlüssel
export function bundleFiles(version, target) {
  switch (target) {
    case 'aarch64-apple-darwin':
      return [
        { from: `dmg/Nexus_${version}_aarch64.dmg`, name: `Nexus_${version}_aarch64.dmg` },
        // Tauri nennt das Updater-Archiv nur nach der App; tauri-action hängt die Architektur an
        { from: 'macos/Nexus.app.tar.gz', name: 'Nexus_aarch64.app.tar.gz', updater: true },
        { from: 'macos/Nexus.app.tar.gz.sig', name: 'Nexus_aarch64.app.tar.gz.sig', updater: true },
      ];
    case 'x86_64-pc-windows-msvc':
      return [
        { from: `nsis/Nexus_${version}_x64-setup.exe`, name: `Nexus_${version}_x64-setup.exe` },
        { from: `nsis/Nexus_${version}_x64-setup.exe.sig`, name: `Nexus_${version}_x64-setup.exe.sig`, updater: true },
      ];
    case 'x86_64-unknown-linux-gnu':
      return [
        { from: `appimage/Nexus_${version}_amd64.AppImage`, name: `Nexus_${version}_amd64.AppImage` },
        { from: `appimage/Nexus_${version}_amd64.AppImage.sig`, name: `Nexus_${version}_amd64.AppImage.sig`, updater: true },
        { from: `deb/Nexus_${version}_amd64.deb`, name: `Nexus_${version}_amd64.deb` },
        { from: `rpm/Nexus-${version}-1.x86_64.rpm`, name: `Nexus-${version}-1.x86_64.rpm` },
      ];
    default:
      throw new Error(`Für das Ziel „${target}“ gibt es keinen Release-Build.`);
  }
}

// Alle Dateien eines Ziels mit Pfad relativ zur Wurzel des Repos: Installer, ggf. Updater-Dateien, CLI-Archiv
export function releaseFiles(version, target, { updater = false } = {}) {
  const bundle = `target/${target}/release/bundle`;
  const files = bundleFiles(version, target)
    .filter((file) => updater || !file.updater)
    .map(({ from, name }) => ({ path: `${bundle}/${from}`, name }));
  const cli = archiveName(version, target);
  return [...files, { path: `dist/release/${cli}`, name: cli }];
}
