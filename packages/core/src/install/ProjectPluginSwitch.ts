import { existsSync } from 'node:fs';
import { mkdir, readFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { HarnessError } from '../errors/HarnessError.js';
import { AtomicWriter } from '../locking/AtomicWriter.js';

export const PLUGIN_ID = 'ai-harness@ai-harness-local';

// Schaltet das Plugin für genau ein Projekt ein: `enabledPlugins` in `<projekt>/.claude/settings.local.json`.
// Das Plugin ist sonst ausgeschaltet (Benutzerebene), damit es in Projekten ohne Harness nichts kostet.
export class ProjectPluginSwitch {
  private readonly writer = new AtomicWriter();

  // Datei mit den lokalen Einstellungen des Projekts
  file(projectDir: string): string {
    return join(projectDir, '.claude', 'settings.local.json');
  }

  // Trägt `true` ein und lässt alle anderen Einstellungen unberührt; liefert false, wenn schon eingetragen
  async enable(projectDir: string): Promise<boolean> {
    const file = this.file(projectDir);
    const settings = await this.read(file);
    const plugins = this.plugins(settings);
    if (plugins[PLUGIN_ID] === true) return false;
    plugins[PLUGIN_ID] = true;
    settings.enabledPlugins = plugins;
    await mkdir(dirname(file), { recursive: true });
    await this.writer.write(file, `${JSON.stringify(settings, null, 2)}\n`);
    return true;
  }

  // Entfernt den Eintrag wieder; liefert false, wenn keiner vorhanden war
  async disable(projectDir: string): Promise<boolean> {
    const file = this.file(projectDir);
    if (!existsSync(file)) return false;
    const settings = await this.read(file);
    const plugins = this.plugins(settings);
    if (!(PLUGIN_ID in plugins)) return false;
    delete plugins[PLUGIN_ID];
    if (Object.keys(plugins).length === 0) delete settings.enabledPlugins;
    else settings.enabledPlugins = plugins;
    await this.writer.write(file, `${JSON.stringify(settings, null, 2)}\n`);
    return true;
  }

  // Liest die Datei; fehlt sie, ist das Ergebnis leer. Kaputtes JSON wird nie überschrieben.
  private async read(file: string): Promise<Record<string, unknown>> {
    if (!existsSync(file)) return {};
    const text = await readFile(file, 'utf8');
    if (text.trim() === '') return {};
    try {
      const parsed: unknown = JSON.parse(text);
      if (typeof parsed === 'object' && parsed !== null && !Array.isArray(parsed)) return parsed as Record<string, unknown>;
    } catch {
      // fällt unten in den Fehler
    }
    throw new HarnessError('settings-unreadable', `${file} ist kein gültiges JSON. Bitte prüfen; die Datei wurde nicht verändert.`);
  }

  // Der Abschnitt `enabledPlugins` als Objekt
  private plugins(settings: Record<string, unknown>): Record<string, unknown> {
    const value = settings.enabledPlugins;
    return typeof value === 'object' && value !== null && !Array.isArray(value) ? { ...(value as Record<string, unknown>) } : {};
  }
}
