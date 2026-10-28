// Gegenstück in TypeScript für die Gegenprobe der Anpassungen (tests/anpassungen_mit_ts.rs).
// Aufruf: node anpassungen_ts.mjs <befehl> <pfad zu packages/core/dist/index.js> [args…]
import { pathToFileURL } from 'node:url';

const [befehl, index, ...rest] = process.argv.slice(2);
const core = await import(pathToFileURL(index).href);

// Der Standard-Workflow als JSON, genau wie BaseSnapshot.formatJson ihn schreibt
if (befehl === 'workflow-json') {
  process.stdout.write(core.BaseSnapshot.formatJson(core.WORKFLOW_PHASES));
} else if (befehl === 'apply') {
  // Anpassungen anwenden wie build-plugin.mjs und die Übersicht ausgeben
  const [cloneDir, customDir, standardDir, pluginDir] = rest;
  const input = { cloneDir, customDir, standardDir, harnessVersion: '0.2.0', tools: new core.CloneWorkflowTools(cloneDir), now: new Date('2026-10-14T10:00:00Z') };
  await new core.CustomizationApplier().apply(input, pluginDir);
  process.stdout.write('fertig');
} else if (befehl === 'check') {
  // Prüfen und die Übersicht als Text ausgeben (wie npm run customize -- check)
  const [cloneDir, customDir, standardDir] = rest;
  const input = { cloneDir, customDir, standardDir, harnessVersion: '0.2.0', tools: new core.CloneWorkflowTools(cloneDir), now: new Date('2026-10-14T10:00:00Z') };
  const { overview } = await new core.CustomizationApplier().check(input);
  process.stdout.write(new core.OverviewFormatter().format(overview, { maxDiffLines: Number.POSITIVE_INFINITY }));
} else {
  throw new Error(`Unbekannter Befehl: ${befehl}`);
}
