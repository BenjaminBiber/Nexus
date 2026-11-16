import type { RunSessionEnd as SessionEnd } from '../../../dbboard/runSession.js';

// Ende der Sitzung: Hook-Fehler als Hinweise, darunter eine Zeile mit Ergebnis von Claude, Schritten, Dauer und Exit-Code.
// Ohne Angaben (Lauf läuft noch) nichts.
export function RunSessionEnd({ end }: { end: SessionEnd }) {
  const { status, facts, exit, hooks } = end;
  if (!status && facts.length === 0 && !exit && hooks.length === 0) return null;
  return (
    <section className="run-end" aria-label="Sitzungsende">
      {hooks.map((hook, i) => (
        <div key={i} className="run-hook">
          <span className="material-icons" aria-hidden="true">
            warning_amber
          </span>
          <div className="run-hook-text">
            <span className="run-hook-title">{hook.title}</span>
            <span className="run-hook-detail">{hook.detail}</span>
          </div>
        </div>
      ))}
      {(status || facts.length > 0 || exit) && (
        <ul className="run-end-facts">
          {status && (
            <li className={`run-end-status ${status.ok ? 'ok' : 'error'}`}>
              <span className="material-icons" aria-hidden="true">
                {status.ok ? 'check' : 'error_outline'}
              </span>
              {status.label}
            </li>
          )}
          {facts.map((fact, i) => (
            <li key={i}>{fact}</li>
          ))}
          {exit && <li className={exit.ok ? undefined : 'run-end-error'}>{exit.label}</li>}
        </ul>
      )}
    </section>
  );
}
