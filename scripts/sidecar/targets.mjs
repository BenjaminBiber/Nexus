// Rust-Ziele nach Tauri-Konvention: so heißt das Sidecar, damit `bundle.externalBin` es findet.
// Gemeinsam genutzt von build-sidecar.mjs und smoke-sidecar.mjs.

// Rust-Ziel je Rechnerart (process.platform-process.arch)
export const TARGETS = {
  'darwin-arm64': 'aarch64-apple-darwin',
  'darwin-x64': 'x86_64-apple-darwin',
  'win32-x64': 'x86_64-pc-windows-msvc',
  'win32-arm64': 'aarch64-pc-windows-msvc',
  'linux-x64': 'x86_64-unknown-linux-gnu',
  'linux-arm64': 'aarch64-unknown-linux-gnu',
};

// Rust-Ziel des bauenden Rechners; undefined, wenn es dafür keines gibt
export function hostTarget() {
  return TARGETS[`${process.platform}-${process.arch}`];
}

// Dateiname des Sidecars für ein Ziel (Windows mit .exe)
export function binaryName(target) {
  return `harness-sidecar-${target}${target.includes('windows') ? '.exe' : ''}`;
}
