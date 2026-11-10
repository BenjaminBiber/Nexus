// Demodaten für Screenshots (README, Theme-Vorschauen): erfundenes Projekt „Nova Shop“ mit Tickets, Plan, Notizen und Frage.
// Alles läuft in einem eigenen, temporären NEXUS_HOME (SQLite-Board) und HOME auf einem freien Port:
// weder das Board unter ~/.nexus noch ein laufender Server auf Port 4710 werden berührt.
// `stop()` beendet den Server und entfernt die temporären Ordner (auch nach Fehlern aufrufen).
import { execFileSync, spawn } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { createServer } from 'node:net';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
export const root = resolve(here, '..', '..');
const webDir = join(root, 'packages', 'web', 'dist');
const bin = process.env.NEXUS_BIN ?? join(root, 'target', 'debug', process.platform === 'win32' ? 'nexus.exe' : 'nexus');

export const sleep = (ms) => new Promise((ok) => setTimeout(ok, ms));

function freePort() {
  return new Promise((ok, fail) => {
    const probe = createServer();
    probe.once('error', fail);
    probe.listen(0, '127.0.0.1', () => {
      const { port } = probe.address();
      probe.close(() => ok(port));
    });
  });
}

// Legt Demodaten an und startet den Server. `beforeServe(nexus)` läuft nach den Demodaten und vor dem Start
// (z. B. Themes installieren). Liefert { base, pid, nexus, stop }.
export async function startDemo({ beforeServe } = {}) {
  if (!existsSync(bin)) throw new Error(`Binary fehlt: ${bin} – erst „cargo build -p nexus-cli --bin nexus“ ausführen`);
  if (!existsSync(join(webDir, 'index.html'))) throw new Error('Oberfläche fehlt – erst „npm run build“ ausführen');

  const tmp = mkdtempSync(join(tmpdir(), 'nexus-screenshots-'));
  const home = join(tmp, 'home');
  const nexusHome = join(home, '.nexus');
  const repo = join(tmp, 'nova-shop');
  const env = {
    ...process.env,
    HOME: home,
    USERPROFILE: home,
    NEXUS_HOME: nexusHome,
    HARNESS_HOME: '',
    NEXUS_PORT: '',
    HARNESS_PORT: '',
    NEXUS_INVOKED_AS: '',
  };
  let server;

  // Beendet den Server samt Kindprozessen; unter Windows reicht kill() allein nicht
  function stopServer() {
    if (!server || server.exitCode !== null) return;
    if (process.platform === 'win32') {
      try {
        execFileSync('taskkill', ['/PID', String(server.pid), '/T', '/F'], { stdio: 'ignore' });
      } catch {
        // schon beendet
      }
    } else {
      server.kill('SIGKILL');
    }
  }

  const stop = async () => {
    stopServer();
    await sleep(300);
    rmSync(tmp, { recursive: true, force: true, maxRetries: 10, retryDelay: 200 });
  };

  const nexus = (...args) => execFileSync(bin, args, { env, cwd: repo, encoding: 'utf8', stdio: ['ignore', 'pipe', 'inherit'] });
  const board = (...args) => nexus('board', ...args);

  try {
    mkdirSync(nexusHome, { recursive: true });
    writeFileSync(
      join(nexusHome, 'board.yaml'),
      'provider: sqlite\nuserId: 11111111-1111-1111-1111-111111111111\nuserName: Demo\n',
    );

    // Wegwerf-Repo, damit die Oberfläche ein Projekt kennt
    mkdirSync(repo, { recursive: true });
    const git = (...args) => execFileSync('git', args, { cwd: repo, stdio: 'ignore' });
    git('init', '-q', '-b', 'main');
    git('-c', 'user.name=Demo', '-c', 'user.email=demo@example.com', 'commit', '-q', '--allow-empty', '-m', 'Start');

    nexus('init');

    // Demodaten nur über die CLI, nie direkt in der Datenbank
    board('project', 'create', 'NOVA', 'Nova Shop');
    board('boards', 'create', 'NOVA', 'Webshop', '--use');
    const create = (title, ...rest) => board('create', title, '--by', 'Demo', ...rest);
    const file = (name, text) => {
      const path = join(tmp, name);
      writeFileSync(path, text);
      return path;
    };

    create('Warenkorb: Mengen direkt im Korb ändern', '--desc', 'Kundinnen und Kunden sollen die Menge einer Position im Warenkorb ändern können, ohne die Seite zu verlassen.', '--label', 'frontend');
    create('Rechnungs-PDF mit Firmenlogo', '--desc', 'Das PDF zur Bestellbestätigung soll das Logo und die Steuernummer enthalten.', '--priority', 'low', '--label', 'backend');
    create('Passwort zurücksetzen per E-Mail', '--desc', 'Der Link in der E-Mail ist 30 Minuten gültig und nur einmal verwendbar.', '--priority', 'high', '--label', 'backend', '--planned', '--status', 'geplant',
      '--acceptance', 'Nach Anforderung kommt eine E-Mail an; der Link setzt ein neues Passwort und ist danach ungültig.',
      '--step', 'Token-Tabelle und Ablaufzeit anlegen', '--step', 'Route zum Anfordern und Einlösen bauen', '--step', 'E-Mail-Vorlage und Tests ergänzen');
    create('Produktsuche mit Filtern', '--desc', 'Suche nach Name, Kategorie und Preisspanne mit Trefferzahl je Filter.', '--priority', 'normal', '--label', 'frontend', '--status', 'in_planning');
    create('Versandkosten nach Zielland berechnen', '--desc', 'Die Versandkosten hängen heute nur vom Gewicht ab. Künftig sollen Zielland und Zustellart einfließen.', '--priority', 'high', '--label', 'backend', '--planned',
      '--acceptance', 'Im Checkout ändert sich der Preis mit dem Zielland; für Österreich und die Schweiz gibt es je einen Test.',
      '--step', 'Preistabelle je Land als Konfiguration ablegen', '--step', 'Berechnung um Zielland und Zustellart erweitern', '--step', 'Checkout zeigt die Kosten live an', '--step', 'Tests für Österreich und die Schweiz');
    create('Zahlungsart PayPal anbinden', '--desc', 'PayPal als zusätzliche Zahlungsart im Checkout.', '--priority', 'normal', '--label', 'backend', '--planned');
    create('Bestseller-Kachel auf der Startseite', '--desc', 'Die Startseite zeigt die fünf meistverkauften Artikel der letzten 30 Tage.', '--priority', 'low', '--label', 'frontend', '--planned');
    create('Versandkosten auf der Rechnung ausweisen', '--desc', 'Die Rechnung zeigt Zielland und Versandkosten als eigene Zeile.', '--priority', 'normal', '--label', 'backend', '--planned', '--status', 'geplant', '--depends-on', 'NOVA-0005');
    create('Alten Newsletter-Dialog entfernen', '--desc', 'Der Dialog wurde durch das Formular in der Fußzeile ersetzt.', '--priority', 'low', '--label', 'frontend');

    // Ticket 5: in Arbeit, mit Plan, Fortschritt und Kommentar
    board('claim', 'NOVA-0005', '--agent', 'agent-1');
    board('plan-json', 'NOVA-0005', '--by', 'Demo', '--file', file('plan.json', JSON.stringify({
      plan: [
        '## Ziel',
        'Der Preis im Checkout richtet sich nach Zielland und Zustellart.',
        '',
        '## Vorgehen',
        '- Preistabelle je Land als Konfigurationsdatei, keine Preise im Code',
        '- `ShippingCalculator` bekommt Zielland und Zustellart als Eingabe',
        '- Der Checkout ruft die Berechnung bei jeder Änderung der Adresse auf',
        '',
        '## Tests',
        'Je ein Fall für Deutschland, Österreich und die Schweiz sowie ein unbekanntes Land.',
      ].join('\n'),
      handbook: '- Deutsche Texte und Kommentare\n- Preise nur aus der Konfiguration lesen\n- Änderungen im `CHANGELOG.md` eintragen',
      diagram: 'flowchart LR\n  A[Adresse ändern] --> B[ShippingCalculator]\n  B --> C[Preistabelle]\n  B --> D[Checkout zeigt Kosten]',
      status: 'in_progress',
    })));
    board('step', 'NOVA-0005', '1', '--by', 'agent-1');
    board('step', 'NOVA-0005', '2', '--by', 'agent-1');
    board('comment', 'NOVA-0005', 'Preistabelle und Berechnung stehen. Als Nächstes kommt die Anzeige im Checkout.', '--author', 'agent-1');

    // Ticket 6 blockiert, Ticket 7 erledigt
    board('set', 'NOVA-0006', '--status', 'blockiert', '--blocked-reason', 'Zugangsdaten für das PayPal-Testkonto fehlen', '--by', 'Demo');
    board('move', 'NOVA-0007', 'erledigt', '--acceptance', 'Die Startseite zeigt fünf Artikel; der Test für die Sortierung ist grün.', '--by', 'Demo');

    board('note', 'add', 'Produktbilder werden in voller Größe geladen', '--details', 'Auf der Kategorieseite lädt jedes Bild 2 MB. Vorschlag: Vorschaubilder in 400 Pixel Breite erzeugen.', '--from', 'NOVA-0001', '--author', 'Demo');
    board('note', 'add', 'Fehlermeldung im Checkout ist nicht übersetzt', '--details', 'Bei abgelaufener Karte erscheint der englische Text des Zahlungsanbieters.', '--from', 'NOVA-0005', '--author', 'Demo');
    board('question', 'add', 'Soll der Versand in die Schweiz Zollgebühren im Preis enthalten?', '--ticket', 'NOVA-0005', '--occasion', 'Planung NOVA-0005', '--author', 'agent-1', '--kind', 'session', '--description', [
      '**Worum es geht**',
      'Für Lieferungen in die Schweiz fallen Zollgebühren an. Heute zeigt der Checkout nur die Versandkosten. Ob die Gebühren im Preis stehen sollen, ändert die Berechnung und die Rechnung.',
      '',
      '**Heute**',
      '- Der Checkout zeigt nur Versandkosten',
      '- Zollgebühren zahlen Kundinnen und Kunden bei der Zustellung',
      '',
      '**Zu entscheiden**',
      'Sollen die Zollgebühren im angezeigten Preis enthalten sein?',
      '',
      '**Varianten**',
      '- Im Preis enthalten – klarer Gesamtpreis, aber ein Zolltarif muss gepflegt werden',
      '- Als Hinweis anzeigen – kein Pflegeaufwand, der Endpreis bleibt unsicher',
      '- Schweiz vorerst ausnehmen – schnell umgesetzt, die Schweiz fehlt im Checkout',
      '',
      '**Empfehlung**',
      'Als Hinweis anzeigen: Sie sparen den Pflegeaufwand und bleiben ehrlich über den möglichen Aufpreis.',
    ].join('\n'));


    await beforeServe?.(nexus);

    // Server auf freiem Port starten und das Wegwerf-Repo als Projekt anmelden
    const port = await freePort();
    server = spawn(bin, ['serve', '--port', String(port), '--web-dir', webDir], { env, cwd: repo, stdio: 'ignore', windowsHide: true });
    const base = `http://127.0.0.1:${port}`;
    for (let i = 0; ; i++) {
      try {
        if ((await fetch(`${base}/api/projects`)).ok) break;
      } catch {
        // Server startet noch
      }
      if (server.exitCode !== null || i > 100) throw new Error('Server startet nicht');
      await sleep(200);
    }

    const registered = await fetch(`${base}/api/projects`, {
      method: 'POST',
      headers: { 'content-type': 'application/json', 'x-harness': '1' },
      body: JSON.stringify({ path: repo }),
    });
    if (!registered.ok) throw new Error(`Projekt anmelden schlug fehl: ${registered.status} ${await registered.text()}`);
    const pid = (await registered.json()).id;
    return { base, pid, nexus, stop };
  } catch (error) {
    await stop();
    throw error;
  }
}
