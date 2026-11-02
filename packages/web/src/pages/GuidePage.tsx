import { CopyCommand } from '../components/CopyCommand.js';
import { GuideSteps } from '../components/GuideSteps.js';
import { useApi } from '../api/ApiContext.js';
import { useAsync } from '../hooks/useAsync.js';
import { DISPATCHER_COMMANDS, DISPATCHER_OUTCOMES, DISPATCHER_READY, DISPATCHER_SETUP, FAQ, guidePhases, TERMS, TERMINAL } from '../guide/guideContent.js';

// Sprungmarken am Seitenanfang, damit man den gesuchten Abschnitt direkt erreicht.
const NAV: Array<[string, string]> = [
  ['ablauf', 'Ablauf'],
  ['dispatcher', 'Dispatcher'],
  ['terminal', 'Im Terminal'],
  ['einrichten', 'Einrichten'],
  ['aktualisieren', 'Aktualisieren'],
  ['anpassen', 'Anpassen'],
  ['begriffe', 'Begriffe'],
  ['fragen', 'Häufige Fragen'],
];

// Anleitung: zuerst der tägliche Ablauf in Phasen (mit kopierbaren Befehlen), danach Einrichten, Aktualisieren, Begriffe und Fragen.
export function GuidePage() {
  const api = useApi();
  const workflow = useAsync(() => api.getWorkflow(), [api]);
  // Bis der Server antwortet, gilt der Standard; so springt die Seite nicht
  const phases = guidePhases(workflow.state.status === 'ready' ? workflow.state.data.phases : undefined);
  return (
    <div className="page narrow stack">
      <div>
        <h1>Anleitung</h1>
        <p className="muted lead">Vom Plan bis zur Umsetzung, Schritt für Schritt.</p>
        <nav className="guide-nav" aria-label="Abschnitte">
          {NAV.map(([id, label]) => (
            <a key={id} href={`#${id}`}>{label}</a>
          ))}
        </nav>
      </div>

      <section id="ablauf" className="stack" aria-labelledby="ablauf-titel">
        <h2 id="ablauf-titel" className="sr-only">Ablauf</h2>
        {phases.map((phase, p) => (
          <div key={phase.id} className="panel">
            <h3>
              <span className="phase-no">{p + 1}</span> {phase.title}
            </h3>
            <p className="muted phase-hint">{phase.hint}</p>
            <GuideSteps steps={phase.steps} label={phase.title} />
          </div>
        ))}
      </section>

      <section id="dispatcher" className="panel">
        <h2>Dispatcher</h2>
        <p>
          Der Dispatcher läuft im Harness-Server und setzt bereite Tickets ohne dich um. Jede Minute prüft er die Boards aus seiner Einstellung boards
          (ohne Auswahl: das aktive Board). Für ein bereites Ticket legt er Branch und Worktree an und startet claude -p als junior-&lt;ID&gt;. Das Profil
          wählt er nach Label profil:, Aufwand im Plan, Regeln und Standardprofil; die Labels model: und effort: gehen vor. Er startet höchstens so viele
          Läufe, wie die WIP-Grenze erlaubt, und nur im Zeitfenster.
        </p>
        <h3>Bereit ist ein Ticket, wenn</h3>
        <ul>
          {DISPATCHER_READY.map((line) => (
            <li key={line}>{line}</li>
          ))}
        </ul>
        <h3>Ausgänge eines Laufs</h3>
        <dl className="terms">
          {DISPATCHER_OUTCOMES.map((o) => (
            <div key={o.term} className="term">
              <dt>{o.term}</dt>
              <dd>{o.meaning}</dd>
            </div>
          ))}
        </dl>
        <p>
          Recherchen zu Fragen startet er ebenfalls: von Hand mit „Recherche starten“ auf der Seite der Frage oder automatisch für neue Fragen zu deinen
          Boards (Einstellungen → Dispatcher → Recherchen). Eine Recherche liest nur und ändert keinen Code.
        </p>
        <h3>Dispatcher einrichten</h3>
        <GuideSteps steps={DISPATCHER_SETUP} label="Dispatcher einrichten" />
        <h3>Dispatcher-Befehle</h3>
        <GuideSteps steps={DISPATCHER_COMMANDS} label="Dispatcher-Befehle" />
      </section>

      <section id="terminal" className="panel">
        <h2>Im Terminal</h2>
        <p className="muted">Optional. Claude ruft diese Befehle bei Bedarf selbst auf; du brauchst sie nur, wenn du ohne Claude arbeitest.</p>
        <GuideSteps steps={TERMINAL} label="Terminal" />
      </section>

      <section id="einrichten" className="panel">
        <h2>Einrichten</h2>
        <p>Einmal pro Rechner, im Ordner des Harness-Repositories. Das baut Oberfläche und Plugin (npm run build, npm run build:plugin); der letzte Befehl installiert das Plugin und den Befehl harness im Terminal:</p>
        <p>
          <CopyCommand command="packages/plugin/dist/nexus/bin/nexus install" />
        </p>
        <p>Danach ein neues Terminal öffnen. Das Plugin ist zunächst ausgeschaltet und belastet Projekte ohne Harness nicht.</p>
        <p>Einmal pro Projekt, im Projektordner. Das richtet das Projekt ein, schaltet das Plugin dafür ein und bindet Doku- und Board-Regel ein:</p>
        <p>
          <CopyCommand command="harness init" />
        </p>
        <p>Danach in Claude Code /reload-plugins eingeben (oder neu starten), damit Hooks und Skills laufen. Dann das Board öffnen:</p>
        <p>
          <CopyCommand command="harness board" />
        </p>
        <p>Board einrichten, einmal je Person und Projekt: Im Board unter Einstellungen → Board-Datenbank die Verbindung ausfüllen. Dann ein Projekt anlegen bzw. wählen und ein eigenes Board anlegen und aktivieren:</p>
        <p>
          <CopyCommand command={'harness board project create ARC "Mein Projekt"'} />
        </p>
        <p>
          <CopyCommand command={'harness board boards create ARC "Mein Board" --use'} />
        </p>
        <p>Zuletzt das Repo dem Board zuordnen (Nummer des Boards und Pfad anpassen). Der Repo-Pfad ist je Person lokal – jeder trägt seinen eigenen Ordner ein:</p>
        <p>
          <CopyCommand command={'harness board boards repo 7 C:\\Projekte\\Demo'} />
        </p>
      </section>

      <section id="aktualisieren" className="panel">
        <h2>Aktualisieren</h2>
        <p>Im Ordner des Harness-Repositories: Änderungen holen (git pull), neu bauen (npm run build, npm run build:plugin) und Plugin, Terminal-Befehl und Server aktualisieren:</p>
        <p>
          <CopyCommand command="packages/plugin/dist/nexus/bin/nexus install" />
        </p>
        <p>Danach in laufenden Sessions /reload-plugins eingeben und harness board starten. Bestehende Projekte brauchen danach einmal harness init, damit die Board-Regel (.claude/rules/ai-harness-board.md) ankommt:</p>
        <p>
          <CopyCommand command="harness init" />
        </p>
      </section>

      <section id="anpassen" className="panel">
        <h2>Anpassen</h2>
        <p>Workflow, Skills und Output-Styles des Harness kannst du für dich ersetzen oder ergänzen. Die Anpassungen liegen im Ordner custom/ des Harness-Repositories, den Git ignoriert, und überstehen Updates ohne Merge. Am einfachsten sagst du Claude in einer beliebigen Session, was du ändern willst:</p>
        <p>
          <CopyCommand command="/nexus:harness-customize" />
        </p>
        <p>Anpassungen wirken erst, nachdem du sie im Ordner des Harness-Repositories anwendest: npm run build:plugin baut das Plugin mit den Anpassungen, danach:</p>
        <p>
          <CopyCommand command="packages/plugin/dist/nexus/bin/nexus install" />
        </p>
        <p>Danach eine neue Session starten oder /reload-plugins eingeben. Welche Anpassungen gelten, zeigen die Einstellungen im Abschnitt „Anpassungen“.</p>
      </section>

      <section id="begriffe" className="panel" aria-label="Begriffe">
        <h2>Begriffe</h2>
        <dl className="terms">
          {TERMS.map((t) => (
            <div key={t.term} className="term">
              <dt>{t.term}</dt>
              <dd>{t.meaning}</dd>
            </div>
          ))}
        </dl>
      </section>

      <section id="fragen" className="panel">
        <h2>Häufige Fragen</h2>
        {FAQ.map((f) => (
          <details key={f.question}>
            <summary>{f.question}</summary>
            <p>{f.answer}</p>
          </details>
        ))}
      </section>
    </div>
  );
}
