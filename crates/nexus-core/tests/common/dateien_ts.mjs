// Gegenprobe zu den gemeinsamen Dateien: schreibt und liest board.yaml, board-repos.yaml
// und settings.json mit den Speichern des gebauten TypeScript-Harness.
// Aufruf: node dateien_ts.mjs <modus> <index.js> <harness-home>
//   schreiben – legt die drei Dateien mit festen Werten an
//   lesen     – gibt den Inhalt der drei Dateien als JSON auf der Standardausgabe aus
import { pathToFileURL } from 'node:url';

const [mode, modulePath, home] = process.argv.slice(2);
const { BoardConfigStore, BoardRepoFile, HarnessHome, UserSettingsStore } = await import(pathToFileURL(modulePath).href);

const harnessHome = new HarnessHome({ HARNESS_HOME: home });
const config = new BoardConfigStore(harnessHome);
const repos = new BoardRepoFile(harnessHome);
const settings = new UserSettingsStore(harnessHome);

if (mode === 'schreiben') {
  await config.write({
    provider: 'mssql',
    file: '',
    server: 'dev-sql-5',
    port: 1433,
    database: 'AiHarness',
    user: 'harness',
    password: 'geheim 1234',
    encrypt: true,
    trustServerCertificate: true,
    userId: '11111111-1111-1111-1111-111111111111',
    userName: 'Florian Müller',
    activeBoard: 7,
  });
  await repos.write(new Map([[7, 'C:\\Projects\\Arches ä'], [8, null], [12, '/home/x/mein repo']]));
  await settings.write({
    notifications: 'windows',
    heroImages: true,
    theme: 'dark',
    claudeUsage: false,
    compactCards: true,
    hideNotNeeded: false,
    hideInPlanning: true,
    ntfy: true,
    ntfyServer: 'https://push.example',
    ntfyTopic: 'harness-x',
    ntfyToken: 'tk_1',
  });
} else {
  const entries = await repos.read();
  console.log(JSON.stringify({
    config: await config.require(),
    repos: Object.fromEntries([...entries].map(([id, path]) => [String(id), path])),
    settings: await settings.read(),
  }));
}
