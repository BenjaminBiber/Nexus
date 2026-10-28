import { HomeFiles } from './HomeFiles.mjs';

// Pfad einer Datei im Home: relativ, mit `/` getrennt, Abschnitte ohne Schrägstriche und Rückwärtsschritte
const HOME_FILE = /^[^/\\]+(?:\/[^/\\]+)*$/;

// Schritt-Art `homeFile`: schreibt eine Datei ins HARNESS_HOME, während das Szenario läuft. Die Fixture `homeFiles` legt
// Dateien vor dem Start an; hier geht es um die, deren Name erst im Lauf feststeht – allen voran das Ergebnis eines
// Workers (`dispatch/runs/<Lauf>.result.json`), dessen Lauf-ID der Durchlauf vergibt. Damit kommt ein Szenario an die
// Zustände nach einem beendeten Lauf (Freigabe vor dem Merge, Merge, Abbruch), ohne je einen Worker zu starten.
export class HomeFileStep {
  // Eigene Felder der Art; der Lader meldet damit Tippfehler im Szenario
  get keys() {
    return ['path', 'content'];
  }

  // Führt den Schritt aus; `context.home` ist das HARNESS_HOME des Szenarios
  async run(step, context) {
    const { path, content } = step.raw;
    const target = context.variables.apply(typeof path === 'string' ? path : '');
    if (!target || !HOME_FILE.test(target) || target.split('/').includes('..')) {
      throw new Error('path muss ein Pfad im Home sein, mit `/` getrennt und ohne `..` (z. B. "dispatch/runs/<Lauf>.result.json").');
    }
    if (content === undefined) throw new Error('content fehlt (ein Text kommt wie er ist, jeder andere JSON-Wert als JSON).');
    const [written] = new HomeFiles(context.home).write({ [target]: content }, context.variables);
    return { request: { kind: 'homeFile', path: target, content: context.variables.apply(content) }, response: { json: { path: written } } };
  }
}
