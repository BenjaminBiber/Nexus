import { existsSync, readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import { DEFAULT_SERVER_PORT, HarnessHome, HarnessError } from '@harness/core';
import type { ServerConfig } from './ServerConfig.js';

export const DEFAULT_PORT = DEFAULT_SERVER_PORT;

// Liest die Einstellungen aus Argumenten und Umgebung (--port, HARNESS_PORT, HARNESS_WEB_DIR, HARNESS_HOME).
export class ServerConfigLoader {
  // Baut die Einstellungen; ungültige Ports ergeben einen verständlichen Fehler
  load(argv: string[], env: Record<string, string | undefined> = process.env): ServerConfig {
    return {
      port: this.port(argv, env),
      host: '127.0.0.1',
      home: new HarnessHome(env),
      webDir: env.HARNESS_WEB_DIR ?? this.defaultWebDir(),
      version: this.version(),
      toastDir: this.toastDir(),
    };
  }

  // Port aus --port, sonst HARNESS_PORT, sonst Standard
  private port(argv: string[], env: Record<string, string | undefined>): number {
    const index = argv.indexOf('--port');
    const raw = index >= 0 ? argv[index + 1] : env.HARNESS_PORT;
    if (raw === undefined) return DEFAULT_PORT;
    const port = Number(raw);
    if (!Number.isInteger(port) || port < 0 || port > 65535) {
      throw new HarnessError('port-invalid', `Der Port „${raw}“ ist ungültig. Erlaubt sind Zahlen von 0 bis 65535.`);
    }
    return port;
  }

  // Gebaute Oberfläche neben dem Serverpaket (packages/web/dist)
  private defaultWebDir(): string {
    return join(dirname(fileURLToPath(import.meta.url)), '..', '..', '..', 'web', 'dist');
  }

  // Bilder der Windows-Benachrichtigung: im Plugin neben `lib/`, sonst im Serverpaket
  private toastDir(): string {
    const here = dirname(fileURLToPath(import.meta.url));
    const candidates = [join(here, '..', 'toast'), join(here, '..', '..', 'assets', 'toast')];
    return candidates.find((dir) => existsSync(join(dir, 'logo-app.png'))) ?? candidates[1];
  }

  // Version aus der package.json des Serverpakets, im gebündelten Plugin aus dessen Manifest
  private version(): string {
    const here = dirname(fileURLToPath(import.meta.url));
    const candidates = [join(here, '..', '..', 'package.json'), join(here, '..', '.claude-plugin', 'plugin.json')];
    for (const file of candidates) {
      try {
        return JSON.parse(readFileSync(file, 'utf8')).version as string;
      } catch {
        /* nächster Kandidat */
      }
    }
    return '0.0.0';
  }
}
