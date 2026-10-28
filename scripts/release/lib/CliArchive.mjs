// Namen rund um das CLI-Archiv eines Releases: Archiv, Ordner im Archiv und die Datei des Programms.
// Reine Funktionen, damit sich die Namensbildung ohne Build prüfen lässt; das Packen steckt in cli-archive.mjs.

// true für ein Windows-Ziel (`x86_64-pc-windows-msvc`): dort heißt das Programm `.exe` und das Archiv `.zip`
export function isWindowsTarget(target) {
  return target.includes('windows');
}

// Ordner im Archiv, zugleich der Name ohne Endung: `nexus-cli-0.2.0-aarch64-apple-darwin`
export function archiveStem(version, target) {
  return `nexus-cli-${version}-${target}`;
}

// Dateiendung des Archivs: Windows bekommt `.zip`, alle anderen `.tar.gz`
export function archiveExtension(target) {
  return isWindowsTarget(target) ? '.zip' : '.tar.gz';
}

// Dateiname des Archivs: `nexus-cli-<version>-<ziel>.tar.gz` bzw. `.zip`
export function archiveName(version, target) {
  return archiveStem(version, target) + archiveExtension(target);
}

// Name des Programms im Archiv; `harness` ist der Befehl, den der Benutzer danach aufruft
export function binaryName(target) {
  return isWindowsTarget(target) ? 'harness.exe' : 'harness';
}
