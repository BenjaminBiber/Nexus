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

// Re-Export-Module als virtuelle Module (gemeinsam für Build und Dev-Server)
const virtualModules = {
  resolveId(id: string) {
    return id.startsWith(PREFIX) ? id : null;
  },
  load(id: string) {
    return id.startsWith(PREFIX) ? reExport(id.slice(PREFIX.length)) : null;
  },
};

// Import-Map als erstes Element im <head>, damit sie vor jedem Modul-Skript steht
function importMapTag(imports: Record<string, string>) {
  return [{ tag: 'script', attrs: { type: 'importmap' }, children: JSON.stringify({ imports }), injectTo: 'head-prepend' as const }];
}

// Stellt installierten Themes React und das Theme-SDK des Hosts über eine Import-Map bereit, damit sie dieselben Instanzen wie die App nutzen.
// Build: je Host-Modul ein eigenes Re-Export-Chunk. Dev-Server: dieselben Re-Export-Module über Vites /@id/-Adressen;
// Vite löst darin react & Co. auf dieselben vorgebündelten Module auf wie in der App.
export function hostModules(): Plugin[] {
  const build: Plugin = {
    name: 'nexus-host-modules',
    apply: 'build',
    ...virtualModules,
    buildStart() {
      for (const [spec, name] of Object.entries(HOST_MODULES)) {
        this.emitFile({ type: 'chunk', id: PREFIX + spec, name: `host-${name}`, preserveSignature: 'strict' });
      }
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
        return importMapTag(imports);
      },
    },
  };

  const dev: Plugin = {
    name: 'nexus-host-modules-dev',
    apply: 'serve',
    ...virtualModules,
    transformIndexHtml: {
      order: 'post',
      handler() {
        // Vite schreibt das Null-Byte virtueller IDs in Adressen als __x00__
        const imports = Object.fromEntries(Object.keys(HOST_MODULES).map((spec) => [spec, `/@id/__x00__nexus-host:${spec}`]));
        return importMapTag(imports);
      },
    },
  };

  return [build, dev];
}
