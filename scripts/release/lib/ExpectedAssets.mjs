// Erwartete Dateinamen eines Releases: Installer und CLI-Archive je Plattform aus dem Job `build` (P6-4), dazu bei
// aktiviertem Updater (`bundle.createUpdaterArtifacts` in tauri.conf.json, P6-7) die Dateien, die tauri-action
// dafür zusätzlich anhängt. Die Tauri-Konfig kommt als Parameter, damit sich das ohne echte Datei testen lässt.

// Grundliste: dmg (nur Apple Silicon, Intel-Macs werden nicht gebaut), NSIS-Installer, AppImage, deb, rpm, CLI-Archive je Ziel
export function baseAssetNames(version) {
  return [
    `Nexus_${version}_aarch64.dmg`,
    `Nexus_${version}_x64-setup.exe`,
    `Nexus_${version}_amd64.AppImage`,
    `Nexus_${version}_amd64.deb`,
    `Nexus-${version}-1.x86_64.rpm`,
    `nexus-cli-${version}-aarch64-apple-darwin.tar.gz`,
    `nexus-cli-${version}-x86_64-pc-windows-msvc.zip`,
    `nexus-cli-${version}-x86_64-unknown-linux-gnu.tar.gz`,
  ];
}

// Zusätzliche Dateien, die tauri-action bei aktiviertem Updater anhängt: `latest.json`, je Installer eine `.sig`
// und für macOS `app.tar.gz` + `.sig` (diese beiden ohne Versionsnummer im Namen)
export function updaterAssetNames(version) {
  return [
    'latest.json',
    'Nexus_aarch64.app.tar.gz',
    'Nexus_aarch64.app.tar.gz.sig',
    `Nexus_${version}_x64-setup.exe.sig`,
    `Nexus_${version}_amd64.AppImage.sig`,
  ];
}

// true, wenn eine der übergebenen Tauri-Konfigs den Updater aktiviert hat
// (`bundle.createUpdaterArtifacts`). Der Schalter steht in `tauri.release.conf.json` und nicht in
// `tauri.conf.json`, weil sonst jeder lokale Build ohne Signierschlüssel abbrechen würde; der
// Release-Workflow gibt diese Datei über `--config` dazu.
export function updaterEnabled(...tauriConfs) {
  return tauriConfs.some((conf) => conf?.bundle?.createUpdaterArtifacts === true);
}

// Dateien des Theme-Stores (theme-index.mjs): `nexus-themes.json` und je mitgeliefertem Theme `<id>-<version>.nexus`.
// `themes` sind die Manifeste aus themes/*/manifest.json
export function themeAssetNames(themes) {
  return themes.length === 0 ? [] : ['nexus-themes.json', ...themes.map((t) => `${t.id}-${t.version}.nexus`)];
}

// Alle erwarteten Dateinamen: Grundliste, bei aktiviertem Updater zusätzlich dessen Dateien, dazu die Theme-Pakete
export function assetNames(version, { updater = false, themes = [] } = {}) {
  const names = updater ? [...baseAssetNames(version), ...updaterAssetNames(version)] : baseAssetNames(version);
  return [...names, ...themeAssetNames(themes)];
}

// Namen aus `expected`, die nicht in `actual` stehen (Reihenfolge wie `expected`)
export function missingAssets(expected, actual) {
  const have = new Set(actual);
  return expected.filter((name) => !have.has(name));
}
