import { createRequire } from 'node:module';
import type { Plugin } from 'vite';

// Nackte Modulnamen, die Theme-Module (main.js) über die Import-Map vom Host bekommen, mit Kurzname der Ausgabedatei
export const HOST_MODULES: Record<string, string> = {
  react: 'react',
  'react-dom': 'react-dom',
  'react-dom/client': 'react-dom-client',
  'react/jsx-runtime': 'jsx-runtime',
  '@nexus/theme-sdk': 'theme-sdk',
};

// Pakete im ESM-Format: export * reicht; die übrigen sind CommonJS und brauchen ausgeschriebene Exportnamen
const ESM_MODULES = new Set(['@nexus/theme-sdk']);

const PREFIX = '\0nexus-host:';
// Dev-Server: eigene Modul-Kennung ohne \0, damit sie als /@id/<id> in die Import-Map passt
// (Vite kodiert \0 in /@id/ als __x00__, das wäre in der Import-Map unnötig umständlich)
const DEV_PREFIX = 'virtual:nexus-host/';
// Kurzname → nackter Modulname, Umkehrung von HOST_MODULES
const DEV_SPECS: Record<string, string> = Object.fromEntries(Object.entries(HOST_MODULES).map(([spec, name]) => [name, spec]));
const require = createRequire(import.meta.url);

// Quelltext des Re-Export-Moduls: für CommonJS die Exportnamen zur Bauzeit ermitteln, export * verlöre sie
function reExport(spec: string): string {
  if (ESM_MODULES.has(spec)) return `export * from '${spec}';`;
  const names = Object.keys(require(spec) as object).filter((n) => n !== 'default' && /^[A-Za-z_$][\w$]*$/.test(n));
  return [
    `import * as m from '${spec}';`,
    'const d = m.default ?? m;',
    'export default d;',
    names.length ? `export const { ${names.join(', ')} } = d;` : '',
  ].join('\n');
}

// Erzeugt je Host-Modul ein eigenes Re-Export-Chunk (teilt React mit der App) und setzt die Import-Map in die index.html.
// Im Build zeigt die Map auf diese Chunks, im Dev-Server auf virtuelle Module, die das Plugin selbst als Re-Export liefert –
// so läuft die main.js installierter Code-Themes auch unter npm run dev:web.
export function hostModules(): Plugin {
  let command: 'build' | 'serve' = 'build';
  return {
    name: 'nexus-host-modules',
    configResolved(config) {
      command = config.command;
    },
    buildStart() {
      // Nur der Build braucht eigene Chunks; im Dev-Server lädt Vite die virtuellen Module auf Anfrage
      if (command !== 'build') return;
      for (const [spec, name] of Object.entries(HOST_MODULES)) {
        this.emitFile({ type: 'chunk', id: PREFIX + spec, name: `host-${name}`, preserveSignature: 'strict' });
      }
    },
    resolveId(id) {
      if (id.startsWith(PREFIX)) return id;
      return id.startsWith(DEV_PREFIX) && DEV_SPECS[id.slice(DEV_PREFIX.length)] ? id : null;
    },
    load(id) {
      if (id.startsWith(PREFIX)) return reExport(id.slice(PREFIX.length));
      const spec = id.startsWith(DEV_PREFIX) ? DEV_SPECS[id.slice(DEV_PREFIX.length)] : undefined;
      return spec ? reExport(spec) : null;
    },
    transformIndexHtml: {
      order: 'post',
      handler(_html, ctx) {
        const imports: Record<string, string> = {};
        if (ctx.bundle) {
          for (const file of Object.values(ctx.bundle)) {
            if (file.type === 'chunk' && file.facadeModuleId?.startsWith(PREFIX)) {
              imports[file.facadeModuleId.slice(PREFIX.length)] = `/${file.fileName}`;
            }
          }
        } else {
          // Dev-Server: Vite löst /@id/<id> auf die virtuellen Module oben auf
          for (const [spec, name] of Object.entries(HOST_MODULES)) imports[spec] = `/@id/${DEV_PREFIX}${name}`;
        }
        return [{ tag: 'script', attrs: { type: 'importmap' }, children: JSON.stringify({ imports }), injectTo: 'head-prepend' }];
      },
    },
  };
}
