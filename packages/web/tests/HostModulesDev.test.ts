// @vitest-environment node
import { readFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { describe, expect, it } from 'vitest';
import { createServer, type Plugin } from 'vite';
import { HOST_MODULES, hostModules } from '../vite/hostModules.js';

const WEB = resolve(__dirname, '..');

// Plugin-Hooks sind in Vite entweder Funktion oder { handler }; für den Test genügt die Funktion dahinter
type Hook = (this: unknown, ...args: any[]) => any;
const hookOf = (plugin: Plugin, name: keyof Plugin): Hook => {
  const hook = plugin[name] as unknown;
  return (typeof hook === 'function' ? hook : (hook as { handler: Hook }).handler) as Hook;
};

// Das Plugin im Dev-Server: configResolved mit command 'serve'
const servePlugin = (command: 'serve' | 'build' = 'serve') => {
  const plugin = hostModules();
  hookOf(plugin, 'configResolved').call(null, { command });
  return plugin;
};

const html = async (plugin: Plugin, ctx: Record<string, unknown>) =>
  (await hookOf(plugin, 'transformIndexHtml').call(null, '<html></html>', ctx)) as { attrs?: Record<string, string>; children?: string }[];

describe('hostModules im Dev-Server', () => {
  it('gibt eine Import-Map auf virtuelle Module aus', async () => {
    const tags = await html(servePlugin(), { path: '/index.html', filename: 'index.html', server: {} });
    expect(tags).toHaveLength(1);
    expect(tags[0]!.attrs).toMatchObject({ type: 'importmap' });
    const map = JSON.parse(tags[0]!.children!) as { imports: Record<string, string> };
    expect(Object.keys(map.imports).sort()).toEqual(Object.keys(HOST_MODULES).sort());
    for (const url of Object.values(map.imports)) expect(url).toMatch(/^\/@id\/virtual:nexus-host\/[\w-]+$/);
  });

  it('löst die virtuellen Module als Re-Export der Pakete auf', () => {
    const plugin = servePlugin();
    const resolveId = (id: string) => hookOf(plugin, 'resolveId').call(null, id, undefined, {});
    const load = (id: string) => hookOf(plugin, 'load').call(null, id);
    expect(resolveId('virtual:nexus-host/react')).toBe('virtual:nexus-host/react');
    expect(resolveId('virtual:nexus-host/gibtsnicht')).toBe(null);
    expect(resolveId('react')).toBe(null);
    expect(load('virtual:nexus-host/react')).toMatch(/from 'react'/);
    expect(load('virtual:nexus-host/jsx-runtime')).toMatch(/from 'react\/jsx-runtime'/);
    expect(load('virtual:nexus-host/theme-sdk')).toBe("export * from '@nexus/theme-sdk';");
    expect(load('react')).toBe(null);
  });

  it('gilt für serve und build und emittiert nur im Build Chunks', () => {
    expect(hostModules().apply).toBeUndefined();
    const emitted: unknown[] = [];
    const ctx = { emitFile: (file: unknown) => emitted.push(file) };
    hookOf(servePlugin('serve'), 'buildStart').call(ctx);
    expect(emitted).toHaveLength(0);
    hookOf(servePlugin('build'), 'buildStart').call(ctx);
    expect(emitted).toHaveLength(Object.keys(HOST_MODULES).length);
  });
});

describe('echter Dev-Server mit der Konfiguration von packages/web', () => {
  it('liefert die Import-Map in der index.html und löst die Module auf dieselbe React-Kopie wie die App auf', async () => {
    const server = await createServer({
      root: WEB,
      configFile: join(WEB, 'vite.config.ts'),
      logLevel: 'silent',
      server: { middlewareMode: true },
    });
    try {
      const html = await server.transformIndexHtml('/index.html', readFileSync(join(WEB, 'index.html'), 'utf8'));
      const map = /<script type="importmap">([\s\S]*?)<\/script>/.exec(html);
      const imports = (JSON.parse(map![1]!) as { imports: Record<string, string> }).imports;
      expect(Object.keys(imports).sort()).toEqual(Object.keys(HOST_MODULES).sort());
      // Die Import-Map steht vor dem Modul-Skript der App, sonst greift sie für dessen Importe nicht
      expect(html.indexOf('<script type="importmap">')).toBeLessThan(html.indexOf('<script type="module"'));

      // Jede Adresse der Map ist ein Modul, das das Plugin selbst auflöst
      for (const [spec, url] of Object.entries(imports)) {
        expect(url, spec).toMatch(/^\/@id\/virtual:nexus-host\/[\w-]+$/);
        expect(await server.pluginContainer.resolveId(url.replace('/@id/', '')), spec).toMatchObject({
          id: url.replace('/@id/', ''),
        });
      }
      // Der Dev-Server leitet /themes an den Server weiter, sonst fängt die SPA-Rückfallseite main.js und manifest.json ab
      expect(Object.keys(server.config.server.proxy ?? {})).toContain('/themes');
    } finally {
      await server.close();
    }
  });
});
