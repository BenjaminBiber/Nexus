// Wählt je Plattform den Runner: einen eigenen (self-hosted), wenn einer online ist, sonst den von GitHub.
// GitHub selbst kennt kein „self-hosted, falls vorhanden“: ein Job mit self-hosted-Labels wartet ohne
// passenden Runner bis zum Abbruch. Deshalb fragt der Job „runners“ vor der Matrix die API und gibt
// das Ergebnis als JSON aus, das die Matrix mit fromJSON() in runs-on einsetzt.
//
// Aufruf (im Workflow): node scripts/ci/pick-runners.mjs
//   GITHUB_REPOSITORY  owner/name (setzt Actions)
//   RUNNER_TOKEN       Token mit „Administration: read“ auf das Repo; das GITHUB_TOKEN darf Runner
//                      nicht auflisten. Fehlt es oder scheitert die Abfrage, gelten die gehosteten Runner
//   GITHUB_OUTPUT      Datei für die Ausgaben (setzt Actions); ohne sie gehen sie auf stdout
import { appendFileSync } from 'node:fs';
import { pathToFileURL } from 'node:url';

// Je Plattform: Labels, die ein eigener Runner tragen muss, und der gehostete Ersatz.
// Die Architektur steht mit drin: der Release baut für aarch64-apple-darwin und x86_64-pc-windows-msvc
export const PLATFORMS = {
  windows: { labels: ['self-hosted', 'Windows', 'X64'], hosted: 'windows-latest' },
  macos: { labels: ['self-hosted', 'macOS', 'ARM64'], hosted: 'macos-26' },
};

// Labels vergleicht GitHub ohne Rücksicht auf Groß- und Kleinschreibung
const lower = (value) => value.toLowerCase();

// Ein Runner passt, wenn er online ist und alle Labels der Plattform trägt (busy ist egal: dann wartet der Job)
export function matches(runner, labels) {
  if (runner.status !== 'online') return false;
  const own = new Set((runner.labels ?? []).map((label) => lower(label.name)));
  return labels.every((label) => own.has(lower(label)));
}

// Ergebnis je Plattform: die Labels als runs-on-Liste oder der gehostete Runner als Text, beides als JSON
export function pick(runners, platforms = PLATFORMS) {
  const result = {};
  for (const [key, { labels, hosted }] of Object.entries(platforms)) {
    const found = runners.filter((runner) => matches(runner, labels));
    result[key] = {
      runsOn: JSON.stringify(found.length > 0 ? labels : hosted),
      names: found.map((runner) => runner.name),
    };
  }
  return result;
}

// Alle Runner des Repos, seitenweise
async function listRunners(repo, token) {
  const runners = [];
  for (let page = 1; ; page += 1) {
    const response = await fetch(`https://api.github.com/repos/${repo}/actions/runners?per_page=100&page=${page}`, {
      headers: { authorization: `Bearer ${token}`, accept: 'application/vnd.github+json', 'x-github-api-version': '2022-11-28' },
    });
    if (!response.ok) throw new Error(`GitHub antwortet mit ${response.status} ${response.statusText}`);
    const body = await response.json();
    runners.push(...body.runners);
    if (runners.length >= body.total_count || body.runners.length === 0) return runners;
  }
}

async function main() {
  const repo = process.env.GITHUB_REPOSITORY;
  const token = process.env.RUNNER_TOKEN;
  let runners = [];
  if (!token) {
    console.log('Kein RUNNER_TOKEN: es gelten die Runner von GitHub.');
  } else if (!repo) {
    console.log('Kein GITHUB_REPOSITORY: es gelten die Runner von GitHub.');
  } else {
    try {
      runners = await listRunners(repo, token);
    } catch (error) {
      // Kein Abbruch: die gehosteten Runner gehen immer
      console.log(`::warning::Eigene Runner nicht abfragbar (${error.message}); es gelten die Runner von GitHub.`);
    }
  }
  const lines = [];
  for (const [key, { runsOn, names }] of Object.entries(pick(runners))) {
    console.log(`${key}: ${runsOn}${names.length > 0 ? ` (online: ${names.join(', ')})` : ''}`);
    lines.push(`${key}=${runsOn}`);
  }
  if (process.env.GITHUB_OUTPUT) appendFileSync(process.env.GITHUB_OUTPUT, `${lines.join('\n')}\n`);
  else console.log(lines.join('\n'));
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? '').href) await main();
