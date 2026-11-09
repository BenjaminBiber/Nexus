// @vitest-environment node
import { execFileSync } from 'node:child_process';
import { existsSync, mkdtempSync, readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { build } from 'vite';
import { beforeAll, describe, expect, it } from 'vitest';
import { SDK_VERSION } from '@nexus/theme-sdk';

const WEB = resolve(__dirname, '..');
const SPECS = ['react', 'react-dom', 'react-dom/client', 'react/jsx-runtime', '@nexus/theme-sdk'];

let out = '';
let html = '';
let imports: Record<string, string> = {};

// Datei im Build zu einer Import-Map-Adresse (/assets/…)
const fileOf = (url: string) => join(out, url.replace(/^\//, ''));

// Relative Importe einer JS-Datei des Builds, rekursiv (Dateinamen unter assets/)
const importsOf = (file: string, seen = new Set<string>()): Set<string> => {
  const code = readFileSync(join(out, 'assets', file), 'utf8');
  for (const [, dep] of code.matchAll(/from\s*["']\.\/([^"']+\.js)["']|import\s*["']\.\/([^"']+\.js)["']/g)) {
    if (dep && !seen.has(dep)) {
      seen.add(dep);
      importsOf(dep, seen);
    }
  }
  return seen;
};

beforeAll(async () => {
  out = mkdtempSync(join(tmpdir(), 'nexus-importmap-'));
  await build({ root: WEB, configFile: join(WEB, 'vite.config.ts'), logLevel: 'silent', build: { outDir: out, emptyOutDir: true } });
  html = readFileSync(join(out, 'index.html'), 'utf8');
  const map = /<script type="importmap">([\s\S]*?)<\/script>/.exec(html);
  imports = map ? (JSON.parse(map[1]!) as { imports: Record<string, string> }).imports : {};
});

describe('Import-Map des Builds', () => {
  it('steht genau einmal vor jedem Modul-Skript und nennt die fünf Host-Module', () => {
    expect(html.match(/<script type="importmap">/g)).toHaveLength(1);
    const mapAt = html.indexOf('<script type="importmap">');
    const firstModule = html.indexOf('<script type="module"');
    expect(firstModule).toBeGreaterThan(mapAt);
    expect(Object.keys(imports).sort()).toEqual([...SPECS].sort());
    for (const url of Object.values(imports)) {
      expect(url).toMatch(/^\/assets\/.+\.js$/);
      expect(existsSync(fileOf(url))).toBe(true);
    }
  });

  it('enthält genau eine React-Kopie, die App und Host-Module teilen', () => {
    const assets = readdirSync(join(out, 'assets')).filter((f) => f.endsWith('.js'));
    // Nur der React-Kern weist die internen Daten zu; react-dom und Re-Exporte lesen sie bloß
    const reactFiles = assets.filter((f) => readFileSync(join(out, 'assets', f), 'utf8').includes('__CLIENT_INTERNALS_DO_NOT_USE_OR_WARN_USERS_THEY_CANNOT_UPGRADE='));
    expect(reactFiles).toHaveLength(1);
    const entry = /<script type="module" crossorigin src="\/assets\/([^"]+)"/.exec(html)![1]!;
    const reactFile = reactFiles[0]!;
    const reachable = (file: string) => file === reactFile || importsOf(file).has(reactFile);
    expect(reachable(entry)).toBe(true);
    for (const spec of ['react', 'react-dom', 'react-dom/client', 'react/jsx-runtime']) {
      expect(reachable(imports[spec]!.replace('/assets/', ''))).toBe(true);
    }
  });

  it('gibt einem Theme-Modul dieselbe React-Instanz und das SDK der App', () => {
    const dir = mkdtempSync(join(tmpdir(), 'nexus-probe-'));
    const mapped = Object.fromEntries(Object.entries(imports).map(([k, v]) => [k, pathToFileURL(fileOf(v)).href]));
    // Lade-Hook wie der Browser: nackte Namen nur über die Import-Map
    const hook = `export async function resolve(spec, ctx, next) { const m = ${JSON.stringify(mapped)}; return spec in m ? { url: m[spec], shortCircuit: true } : next(spec, ctx); }`;
    writeFileSync(join(dir, 'theme-probe.mjs'), [
      "import React, { useState } from 'react';",
      "import { jsx } from 'react/jsx-runtime';",
      "import { createRoot } from 'react-dom/client';",
      "import * as sdk from '@nexus/theme-sdk';",
      'export { React, useState, jsx, createRoot, sdk };',
    ].join('\n'));
    writeFileSync(join(dir, 'run.mjs'), [
      "import { register } from 'node:module';",
      `register('data:text/javascript,' + encodeURIComponent(${JSON.stringify(hook)}));`,
      `const probe = await import(${JSON.stringify(pathToFileURL(join(dir, 'theme-probe.mjs')).href)});`,
      `const app = await import(${JSON.stringify(mapped.react)});`,
      'console.log(JSON.stringify({',
      '  sameReact: probe.React === app.default,',
      '  sameHook: probe.useState === app.default.useState,',
      "  jsx: typeof probe.jsx, createRoot: typeof probe.createRoot, sdk: probe.sdk.SDK_VERSION,",
      '}));',
    ].join('\n'));
    const result = JSON.parse(execFileSync(process.execPath, [join(dir, 'run.mjs')], { encoding: 'utf8' }).trim().split('\n').pop()!);
    expect(result).toEqual({ sameReact: true, sameHook: true, jsx: 'function', createRoot: 'function', sdk: SDK_VERSION });
  });
});
