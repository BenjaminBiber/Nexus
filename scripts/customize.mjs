// Anpassungen im Clone prüfen, Standard kopieren oder zurücksetzen: npm run customize -- check | copy … | reset …
// Nutzt packages/core/dist (gebaut von npm run setup); Ordner der Anpassungen für Tests über HARNESS_CUSTOM_DIR.
import { existsSync, readFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const CORE = join(ROOT, 'packages/core/dist/index.js');
const MANIFEST = join(ROOT, 'packages/plugin/manifest');
const CUSTOM = process.env.HARNESS_CUSTOM_DIR || join(ROOT, 'custom');

if (!existsSync(CORE)) {
  console.error('Fehlt: packages/core/dist. Bitte zuerst npm run setup -- --no-pull ausführen.');
  process.exit(1);
}

// Prüft alles wie setup, ohne zu schreiben, und zeigt die Diffs vollständig
async function check(core) {
  const harnessVersion = JSON.parse(readFileSync(join(ROOT, 'packages/server/package.json'), 'utf8')).version;
  const input = { cloneDir: ROOT, customDir: CUSTOM, standardDir: MANIFEST, harnessVersion, tools: new core.CloneWorkflowTools(ROOT) };
  const { overview } = await new core.CustomizationApplier().check(input);
  console.log(new core.OverviewFormatter().format(overview, { maxDiffLines: Number.POSITIVE_INFINITY }));
}

try {
  // Laden im try: ein kaputtes dist gibt die schlichte Meldung statt eines Stacktraces
  const core = await import(pathToFileURL(CORE).href);
  const args = new core.CustomizeArgs().parse(process.argv.slice(2));
  if (args.command === 'check') await check(core);
  else {
    const copier = new core.CustomizationCopier(new core.CustomizationPaths(CUSTOM, MANIFEST));
    console.log(args.command === 'copy' ? copier.copy(args.target, { rebase: args.rebase }) : copier.reset(args.target));
  }
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
}
