import { spawnSync } from 'node:child_process';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { tmpdir } from 'node:os';
import { basename, join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import type { Problem } from './CustomizationOverview.js';
import type { WorkflowTools } from './WorkflowTools.js';
interface Esbuild {
  build(options: Record<string, unknown>): Promise<{ outputFiles: Array<{ text: string }> }>;
}
interface EsbuildPluginBuild {
  onResolve(options: { filter: RegExp }, callback: () => { path: string }): void;
}
// Prüft und lädt custom/workflow.ts mit TypeScript und esbuild aus dem Clone (TypeScript 7 nur als Prozess).
export class CloneWorkflowTools implements WorkflowTools {
  // `timeoutMs`: Zeitgrenze der Typprüfung (zwei Minuten); nur die Tests setzen sie kürzer
  constructor(
    private readonly cloneDir: string,
    private readonly timeoutMs = 120000,
  ) {}
  // Prüft eine erzeugte Datei, die den Default-Export als WorkflowPhase[] verwendet
  typecheck(file: string): Problem[] {
    const dir = mkdtempSync(join(tmpdir(), 'harness-workflow-check-'));
    try {
      this.writeCheck(dir, file);
      // tsc des Clones als Prozess starten, weil TypeScript 7 keine stabile JS-Schnittstelle hat
      const tsc = join(this.cloneDir, 'node_modules', 'typescript', 'bin', 'tsc');
      const result = spawnSync(process.execPath, [tsc, '-p', join(dir, 'tsconfig.json'), '--pretty', 'false'], { cwd: this.cloneDir, encoding: 'utf8', windowsHide: true, timeout: this.timeoutMs });
      // Zeitüberschreitung (ETIMEDOUT) oder Abbruch durch ein Signal: eigene Meldung statt „ließ sich nicht starten“
      if ((result.error as NodeJS.ErrnoException | undefined)?.code === 'ETIMEDOUT' || result.signal) {
        return [{ code: 'type-error', message: 'Die Typprüfung hat zu lange gedauert (über 2 Minuten).', file }];
      }
      if (result.error) return [{ code: 'type-error', message: `TypeScript ließ sich nicht starten: ${result.error.message}`, file }];
      return this.parse(`${result.stdout}${result.stderr}`, file, result.status ?? 1);
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  }
  // Bündelt die Datei mit esbuild, lädt sie und gibt den Default-Export als reine Daten (JSON) zurück
  async load(file: string): Promise<unknown> {
    const esbuild = createRequire(join(this.cloneDir, 'package.json'))('esbuild') as Esbuild;
    const dir = mkdtempSync(join(tmpdir(), 'harness-workflow-load-'));
    try {
      // @harness/core zeigt auf das gebaute core-Paket des Clones (Plugin statt alias, weil alias keine absoluten Pfade nimmt)
      const core = this.coreFile('index.js');
      // Hilfsfunktion `require` für gebündelte CommonJS-Abhängigkeiten von core (z. B. yaml); ohne sie scheitert der Import in reinem Node
      const banner = "import { createRequire as __createRequire } from 'node:module';\nconst require = __createRequire(import.meta.url);";
      const out = await esbuild.build({
        entryPoints: [file], bundle: true, platform: 'node', format: 'esm', write: false, logLevel: 'silent', banner: { js: banner },
        plugins: [{ name: 'harness-core', setup: (b: EsbuildPluginBuild) => b.onResolve({ filter: /^@harness\/core$/ }, () => ({ path: core })) }],
      });
      // Gebündelten Code in eine temporäre Datei schreiben und importieren; JSON entfernt Funktionen
      const target = join(dir, 'workflow.mjs');
      writeFileSync(target, out.outputFiles[0].text);
      const module = (await import(pathToFileURL(target).href)) as { default?: unknown };
      return JSON.parse(JSON.stringify(module.default ?? null));
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  }
  // Schreibt check.ts und tsconfig.json; @harness/core zeigt auf die gebauten Typen des Clones
  private writeCheck(dir: string, file: string): void {
    // Absoluter Importpfad mit `/` (ein relativer scheitert unter Windows, wenn Temp und Clone auf verschiedenen Laufwerken liegen)
    const target = resolve(file).split('\\').join('/');
    writeFileSync(join(dir, 'check.ts'), [
      "import type { WorkflowPhase } from '@harness/core';",
      `import phases from ${JSON.stringify(target)};`,
      'export const checked: WorkflowPhase[] = phases;',
      '',
    ].join('\n'));
    const compilerOptions = {
      strict: true, noEmit: true, target: 'es2023', module: 'esnext', moduleResolution: 'bundler',
      allowImportingTsExtensions: true, skipLibCheck: true, types: [],
      paths: { '@harness/core': [this.coreFile('index.d.ts')] },
    };
    writeFileSync(join(dir, 'tsconfig.json'), JSON.stringify({ compilerOptions, files: ['check.ts'] }, null, 2));
  }
  // Zerlegt „datei(zeile,spalte): error TSnnnn: text“; Fehler in check.ts betreffen den Default-Export
  private parse(output: string, file: string, status: number): Problem[] {
    const problems: Problem[] = [];
    for (const match of output.matchAll(/^(.+?)\((\d+),\d+\): error (TS\d+): (.+)$/gm)) {
      if (basename(match[1]) === 'check.ts') {
        problems.push({ code: 'type-error', message: `Der Default-Export passt nicht zu WorkflowPhase[]: ${match[4]}`, file });
      } else {
        problems.push({ code: 'type-error', message: `${match[3]}: ${match[4]}`, file: resolve(this.cloneDir, match[1]), line: Number(match[2]) });
      }
    }
    // Ohne erkennbare Fehlerzeile trotzdem melden, wenn tsc fehlschlug
    if (!problems.length && status !== 0) problems.push({ code: 'type-error', message: output.trim().split('\n')[0] || 'Typprüfung fehlgeschlagen.', file });
    return problems;
  }
  // Datei im gebauten core-Paket des Clones
  private coreFile(name: string): string {
    return join(this.cloneDir, 'packages', 'core', 'dist', name);
  }
}
