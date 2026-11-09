// Legt das NSIS-Werkzeug für den Windows-Installer vor dem Tauri-Build dort ab, wo Tauri es sucht
// (%LOCALAPPDATA%\tauri\NSIS). Tauri lädt es sonst selbst, aber erst beim Bündeln nach dem ganzen
// Build, anonym, und bricht bei einem Fehler von GitHub (etwa HTTP 504) ohne zweiten Versuch ab.
// Hier wird angemeldet und mit Wiederholungen geladen und die Prüfsumme wie bei Tauri verglichen.
// Passt der Stand später doch nicht, prüft Tauri das selbst und lädt nach: das Skript kann den
// Build also nicht verschlechtern.
//
// Aufruf (im Workflow, nur Windows): node scripts/ci/prefetch-nsis.mjs
//   LOCALAPPDATA  Basis des Werkzeug-Ordners (setzt Windows)
//   GITHUB_TOKEN  optional; damit wird über die Release-API geladen statt anonym über github.com
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, renameSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { pathToFileURL } from 'node:url';

// Adressen und Prüfsummen aus tauri-bundler (crates/tauri-bundler/src/bundle/windows/nsis/mod.rs,
// Stand tauri-cli-v2.12.4). Beim Anheben von @tauri-apps/cli dort nachsehen und hier angleichen;
// der Cache-Schlüssel im Release-Workflow hängt am Inhalt dieser Datei
export const NSIS = {
  url: 'https://github.com/tauri-apps/binary-releases/releases/download/nsis-3.13/nsis-3.13.zip',
  sha1: 'DB14F8AF2EA346B786C6AE19343F61AAF5E09B8E',
  // Ordner im Archiv; Tauri benennt ihn in NSIS um
  folder: 'nsis-3.13',
  exeSha1: '6D45D4E8CCF18FE1FDAC9164058F356102A2ECF2',
};
export const TAURI_UTILS = {
  url: 'https://github.com/tauri-apps/nsis-tauri-utils/releases/download/nsis_tauri_utils-v0.5.3/nsis_tauri_utils.dll',
  sha1: '75197FEE3C6A814FE035788D1C34EAD39349B860',
  path: 'Plugins/x86-unicode/additional/nsis_tauri_utils.dll',
};

export const sha1 = (data) => createHash('sha1').update(data).digest('hex').toUpperCase();

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

// https://github.com/<owner>/<repo>/releases/download/<tag>/<datei>
const RELEASE_DOWNLOAD = /^https:\/\/github\.com\/([^/]+)\/([^/]+)\/releases\/download\/([^/]+)\/([^/]+)$/;

// Holt eine Release-Datei. Mit Token über api.github.com: anonyme Zugriffe auf github.com drosselt
// GitHub unter Last mit HTTP 504, angemeldete nicht. Der Weiterleitung zum Speicher folgt fetch ohne
// den Authorization-Header, weil sie auf eine andere Herkunft zeigt
async function fetchFile(url, token, fetchImpl) {
  const parts = token ? RELEASE_DOWNLOAD.exec(url) : null;
  if (!parts) return fetchImpl(url);
  const [, owner, repo, tag, name] = parts;
  const headers = { authorization: `Bearer ${token}`, 'x-github-api-version': '2022-11-28' };
  const release = await fetchImpl(`https://api.github.com/repos/${owner}/${repo}/releases/tags/${tag}`, {
    headers: { ...headers, accept: 'application/vnd.github+json' },
  });
  if (!release.ok) throw new Error(`Release ${tag}: HTTP ${release.status}`);
  const asset = (await release.json()).assets.find((entry) => entry.name === name);
  if (!asset) throw new Error(`Release ${tag} hat keine Datei ${name}`);
  return fetchImpl(asset.url, { headers: { ...headers, accept: 'application/octet-stream' } });
}

// Lädt eine Datei und prüft ihre SHA1. Jeder Fehler (Netz, HTTP-Status, falsche Prüfsumme durch einen
// abgebrochenen Download) führt zu einem neuen Versuch; die Pause wächst mit jedem Versuch
export async function download(
  url,
  expectedSha1,
  { token, attempts = 6, delayMs = 10000, fetchImpl = fetch, wait = sleep, log = console.log } = {},
) {
  for (let attempt = 1; ; attempt += 1) {
    try {
      const response = await fetchFile(url, token, fetchImpl);
      if (!response.ok) throw new Error(`HTTP ${response.status}`);
      const data = Buffer.from(await response.arrayBuffer());
      const actual = sha1(data);
      if (actual !== expectedSha1.toUpperCase()) throw new Error(`SHA1 ${actual} statt ${expectedSha1}`);
      return data;
    } catch (error) {
      if (attempt >= attempts) throw new Error(`${url}: ${error.message} (nach ${attempts} Versuchen)`);
      const pause = delayMs * attempt;
      log(`Versuch ${attempt}/${attempts} für ${url} gescheitert (${error.message}); neuer Versuch in ${pause / 1000} s`);
      await wait(pause);
    }
  }
}

// SHA1 einer Datei oder null, wenn es sie nicht gibt
function fileSha1(path) {
  return existsSync(path) ? sha1(readFileSync(path)) : null;
}

// Das tar von Windows (bsdtar) entpackt auch zip; ausdrücklich aus System32, weil auf den Runnern
// das GNU-tar von Git vorne im PATH stehen kann, und das kennt kein zip
function extractZip(zipPath, target) {
  const tar = join(process.env.SystemRoot ?? 'C:\\Windows', 'System32', 'tar.exe');
  const result = spawnSync(tar, ['-xf', zipPath, '-C', target], { stdio: 'inherit' });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`tar endete mit ${result.status}`);
}

// Stellt toolsDir/NSIS auf den Stand, den Tauri erwartet. Ergebnis: „vorhanden“ oder „geladen“
export async function prefetch({
  toolsDir,
  token,
  nsis = NSIS,
  utils = TAURI_UTILS,
  download: load = download,
  extract = extractZip,
  log = console.log,
}) {
  const nsisDir = join(toolsDir, 'NSIS');
  if (fileSha1(join(nsisDir, 'NSIS.exe')) === nsis.exeSha1 && fileSha1(join(nsisDir, utils.path)) === utils.sha1) {
    log(`NSIS liegt passend in ${nsisDir}`);
    return 'vorhanden';
  }

  rmSync(nsisDir, { recursive: true, force: true });
  rmSync(join(toolsDir, nsis.folder), { recursive: true, force: true });
  mkdirSync(toolsDir, { recursive: true });

  log(`Lade ${nsis.url}`);
  const zipPath = join(toolsDir, `${nsis.folder}.zip`);
  writeFileSync(zipPath, await load(nsis.url, nsis.sha1, { token }));
  try {
    extract(zipPath, toolsDir);
  } finally {
    rmSync(zipPath, { force: true });
  }
  renameSync(join(toolsDir, nsis.folder), nsisDir);

  log(`Lade ${utils.url}`);
  const dllPath = join(nsisDir, utils.path);
  const dll = await load(utils.url, utils.sha1, { token });
  mkdirSync(dirname(dllPath), { recursive: true });
  writeFileSync(dllPath, dll);

  log(`NSIS liegt jetzt in ${nsisDir}`);
  return 'geladen';
}

async function main() {
  const base = process.env.LOCALAPPDATA;
  if (!base) {
    console.error('LOCALAPPDATA nicht gesetzt: das Skript ist nur für Windows gedacht.');
    process.exit(2);
  }
  try {
    const token = process.env.GITHUB_TOKEN || undefined;
    if (!token) console.log('Kein GITHUB_TOKEN: es wird anonym über github.com geladen.');
    await prefetch({ toolsDir: join(base, 'tauri'), token });
  } catch (error) {
    console.error(`::error::NSIS nicht ladbar: ${error.message}`);
    process.exit(1);
  }
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? '').href) await main();
