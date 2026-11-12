import { cpSync, existsSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { defineConfig, type Plugin } from 'vite';

const root = import.meta.dirname;

// Legt manifest.json, theme.css und assets/ neben main.js, damit `nexus theme pack dist` ein vollständiges Paket baut
function copyThemeFiles(): Plugin {
  let outDir = '';
  return {
    name: 'nexus-theme-files',
    apply: 'build',
    configResolved(config) {
      outDir = resolve(config.root, config.build.outDir);
    },
    closeBundle() {
      for (const name of ['manifest.json', 'theme.css', 'assets']) {
        if (existsSync(join(root, name))) cpSync(join(root, name), join(outDir, name), { recursive: true });
      }
    },
  };
}

export default defineConfig({
  root,
  // Immer die Produktions-JSX-Runtime (react/jsx-runtime), auch wenn der Build aus einem Test mit NODE_ENV=test läuft
  esbuild: { jsx: 'automatic', jsxDev: false },
  plugins: [copyThemeFiles()],
  build: {
    outDir: 'dist',
    emptyOutDir: true,
    // Kein public/: das Paket enthält nur, was Nexus lädt
    copyPublicDir: false,
    lib: { entry: 'src/main.tsx', formats: ['es'], fileName: () => 'main.js' },
    rollupOptions: {
      // Kommen zur Laufzeit aus Nexus (Import-Map); nie mitbündeln, sonst laufen zwei Reacts
      external: ['react', 'react/jsx-runtime', 'react/jsx-dev-runtime', 'react-dom', '@nexus/theme-sdk'],
    },
  },
});
