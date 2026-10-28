import { existsSync, readFileSync } from 'node:fs';
import { isSea } from 'node:sea';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import { DEFAULT_SERVER_PORT, HarnessHome, HarnessError } from '@harness/core';
import type { ServerConfig } from './ServerConfig.js';

export const DEFAULT_PORT = DEFAULT_SERVER_PORT;

// Liest die Einstellungen aus Argumenten und Umgebung (--port, HARNESS_PORT, HARNESS_WEB_DIR, HARNESS_TOAST_DIR,
// HARNESS_HOME, HARNESS_SIDECAR, HARNESS_PUBLIC_PORT).
export class ServerConfigLoader {
  // sea: läuft der Server als gebündeltes Sidecar (Single Executable Application)? Dann liegen Oberfläche und
  // Bilder neben dem Programm, denn es gibt keine Paketordner. Tests setzen den Wert selbst.
  constructor(private readonly sea: boolean = isSea()) {}

  // Baut die Einstellungen; ungültige Ports ergeben einen verständlichen Fehler
  load(argv: string[], env: Record<string, string | undefined> = process.env): ServerConfig {
    return {
      port: this.port(argv, env),
      host: '127.0.0.1',
      home: new HarnessHome(env),
      webDir: env.HARNESS_WEB_DIR ?? this.defaultWebDir(),
      version: this.version(),
      toastDir: env.HARNESS_TOAST_DIR ?? this.toastDir(),
      sidecar: env.HARNESS_SIDECAR === '1',
      publicPort: this.publicPort(env),
    };
  }

  // Basis-Adresse der Links in Meldungen (Windows-Toast, ntfy): hinter dem Rust-Server der
  // öffentliche Port, sonst der eigene
  static noticeBaseUrl(config: Pick<ServerConfig, 'host' | 'port' | 'publicPort'>): string {
    return `http://${config.host}:${config.publicPort ?? config.port}`;
  }

  // Port aus --port, sonst HARNESS_PORT, sonst Standard
  private port(argv: string[], env: Record<string, string | undefined>): number {
    const index = argv.indexOf('--port');
    const raw = index >= 0 ? argv[index + 1] : env.HARNESS_PORT;
    if (raw === undefined) return DEFAULT_PORT;
    return this.checked(raw);
  }

  // Öffentlicher Port des Rust-Servers aus HARNESS_PUBLIC_PORT; ohne Angabe gibt es keinen
  private publicPort(env: Record<string, string | undefined>): number | undefined {
    const raw = env.HARNESS_PUBLIC_PORT;
    if (raw === undefined || raw === '') return undefined;
    return this.checked(raw);
  }

  // Eine Portangabe als Zahl; ungültige ergeben einen verständlichen Fehler
  private checked(raw: string): number {
    const port = Number(raw);
    if (!Number.isInteger(port) || port < 0 || port > 65535) {
      throw new HarnessError('port-invalid', `Der Port „${raw}“ ist ungültig. Erlaubt sind Zahlen von 0 bis 65535.`);
    }
    return port;
  }

  // Gebaute Oberfläche: im Sidecar neben dem Programm, sonst neben dem Serverpaket (packages/web/dist)
  private defaultWebDir(): string {
    if (this.sea) return join(dirname(process.execPath), 'web');
    return join(dirname(fileURLToPath(import.meta.url)), '..', '..', '..', 'web', 'dist');
  }

  // Bilder der Windows-Benachrichtigung: im Sidecar neben dem Programm, im Plugin neben `lib/`, sonst im Serverpaket
  private toastDir(): string {
    if (this.sea) return join(dirname(process.execPath), 'toast');
    const here = dirname(fileURLToPath(import.meta.url));
    const candidates = [join(here, '..', 'toast'), join(here, '..', '..', 'assets', 'toast')];
    return candidates.find((dir) => existsSync(join(dir, 'logo-app.png'))) ?? candidates[1];
  }

  // Version: im Sidecar aus der beim Bündeln gesetzten Konstante (dort gibt es keine package.json),
  // sonst aus der package.json des Serverpakets, im gebündelten Plugin aus dessen Manifest
  private version(): string {
    const bundled = process.env.HARNESS_BUNDLED_VERSION;
    if (bundled) return bundled;
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
