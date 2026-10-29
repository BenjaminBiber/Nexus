// Gegenprobe zum Verzeichnis der Worktrees: schreibt und liest `harness/worktrees.json`
// mit dem `WorktreeRegistry` des gebauten TypeScript-Harness.
// Aufruf: node worktrees_ts.mjs <modus> <index.js> <gemeinsames-git-verzeichnis>
//   schreiben – legt zwei Einträge an und markiert einen als bereit und eingerichtet
//   lesen     – gibt das Verzeichnis als JSON auf der Standardausgabe aus
import { pathToFileURL } from 'node:url';

const [mode, modulePath, commonDir] = process.argv.slice(2);
const { HarnessLayout, WorktreeRegistry } = await import(pathToFileURL(modulePath).href);

const registry = new WorktreeRegistry(new HarnessLayout(commonDir));

if (mode === 'schreiben') {
  await registry.add('ticket/ARC-0001-von-ts', '/pfad/wt ä', 'ARC-0001');
  await registry.addTicket('ticket/ARC-0001-von-ts', 'ARC-0002');
  await registry.setReady('ticket/ARC-0001-von-ts', true);
  await registry.setSetup('ticket/ARC-0001-von-ts', 'pending');
  await registry.add('ticket/ARC-0003-zweiter', '/pfad/zwei', 'ARC-0003');
  await registry.markRemoved('ticket/ARC-0003-zweiter');
} else if (mode === 'lesen') {
  process.stdout.write(JSON.stringify(await registry.read()));
} else {
  throw new Error(`unbekannter Modus: ${mode}`);
}
