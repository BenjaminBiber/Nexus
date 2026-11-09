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

// Erzeugt je Host-Modul ein eigenes Re-Export-Chunk (teilt React mit der App) und setzt die Import-Map in die gebaute index.html.
// Nur im Build: der Dev-Server bekommt keine Import-Map, Theme-Code wird gegen den Build geprüft.
export function hostModules(): Plugin {
  return {
    name: 'nexus-host-modules',
    apply: 'build',
    buildStart() {
      for (const [spec, name] of Object.entries(HOST_MODULES)) {
        this.emitFile({ type: 'chunk', id: PREFIX + spec, name: `host-${name}`, preserveSignature: 'strict' });
      }
    },
    resolveId(id) {
      return id.startsWith(PREFIX) ? id : null;
    },
    load(id) {
      return id.startsWith(PREFIX) ? reExport(id.slice(PREFIX.length)) : null;
    },
    transformIndexHtml: {
      order: 'post',
      handler(_html, ctx) {
        if (!ctx.bundle) return;
        const imports: Record<string, string> = {};
        for (const file of Object.values(ctx.bundle)) {
          if (file.type === 'chunk' && file.facadeModuleId?.startsWith(PREFIX)) {
            imports[file.facadeModuleId.slice(PREFIX.length)] = `/${file.fileName}`;
          }
        }
        return [{ tag: 'script', attrs: { type: 'importmap' }, children: JSON.stringify({ imports }), injectTo: 'head-prepend' }];
      },
    },
  };
}
