#!/usr/bin/env node
// Erzeugt aus den gepackten Themes (`npm run build:themes` → dist/themes/*.nexus) die Release-Dateien für den
// Theme-Store: je Theme `<id>-<version>.nexus`, ein Vorschaubild aus dem Paket als `<id>-<version>-preview.<endung>`
// und den Index `nexus-themes.json`.
// Aufruf: node scripts/release/theme-index.mjs [<pakete> [<basisadresse> [<ausgabe>]]]
//   <pakete>        Ordner mit .nexus-Dateien (Standard dist/themes)
//   <basisadresse>  wo die Pakete später liegen, z. B. https://github.com/<repo>/releases/download/<tag>
//                   (Standard: Platzhalter-Tag v0.0.0 von BenjaminBiber/Nexus, nur für Trockenläufe)
//   <ausgabe>       Zielordner (Standard dist/release-themes; wird geleert)
import { copyFileSync, existsSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { packageManifest, packageName, packagePreview, themeIndex } from './lib/ThemeIndex.mjs';

const [src = 'dist/themes', baseUrl = 'https://github.com/BenjaminBiber/Nexus/releases/download/v0.0.0', out = 'dist/release-themes'] =
  process.argv.slice(2);

try {
  if (!existsSync(src)) throw new Error(`Ordner ${src} fehlt – zuerst npm run build:themes.`);
  const files = readdirSync(src).filter((name) => name.endsWith('.nexus')).sort();
  if (files.length === 0) throw new Error(`Keine .nexus-Dateien in ${src}.`);
  const packages = files.map((name) => readFileSync(join(src, name)));
  const index = themeIndex(packages, { baseUrl });

  rmSync(out, { recursive: true, force: true });
  mkdirSync(out, { recursive: true });
  files.forEach((name, i) => {
    const manifest = packageManifest(packages[i]);
    copyFileSync(join(src, name), join(out, packageName(manifest)));
    const preview = packagePreview(packages[i], manifest);
    if (preview) writeFileSync(join(out, preview.name), preview.data);
  });
  writeFileSync(join(out, 'nexus-themes.json'), `${JSON.stringify(index, null, 2)}\n`);
  console.log(`${index.length} Theme(s) → ${resolve(out)}`);
} catch (error) {
  console.error(error.message);
  process.exit(1);
}
