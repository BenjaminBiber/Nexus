import { useEffect, useId, useState } from 'react';
import { useApi } from '../../api/ApiContext.js';
import type { PluginInstallDto, PluginInstallProgressDto, PluginSetupDto } from '../../api/types.js';
import { DbModal } from '../db/DbModal.js';

type Phase = { kind: 'idle' } | { kind: 'busy' } | { kind: 'done'; result: PluginInstallDto } | { kind: 'failed'; hints: string[] };
type Step = NonNullable<PluginInstallProgressDto['progress']>;

// Abstand der Abfragen des Fortschritts während der Installation
const POLL_MS = 400;

// Bietet nach dem Start der App an, das mitgelieferte Claude-Code-Plugin zu installieren oder zu aktualisieren;
// während des Laufs zeigt er Fortschritt und laufenden Schritt, Knöpfe und Esc sind gesperrt.
export function PluginSetupDialog({ setup, onLater, onNever, onClose }: { setup: PluginSetupDto; onLater: () => void; onNever: () => void; onClose: () => void }) {
  const api = useApi();
  const titleId = useId();
  const [phase, setPhase] = useState<Phase>({ kind: 'idle' });
  const busy = phase.kind === 'busy';
  const update = setup.state === 'outdated';
  const install = () => {
    setPhase({ kind: 'busy' });
    api.installPlugin().then(
      (result) => setPhase(result.ok ? { kind: 'done', result } : { kind: 'failed', hints: result.hints.length ? result.hints : ['Die Installation ist gescheitert.'] }),
      (error: unknown) => setPhase({ kind: 'failed', hints: [error instanceof Error ? error.message : String(error)] }),
    );
  };
  return (
    <DbModal role="alertdialog" labelledBy={titleId} onEscape={() => { if (!busy) (phase.kind === 'done' ? onClose : onLater)(); }} backdropClassName="db-dialog-backdrop" className="panel db-dialog">
      <strong id={titleId}>Claude-Code-Plugin {update ? 'aktualisieren' : 'installieren'}</strong>
      {update ? (
        <p className="db-dialog-hint">
          Das Claude-Code-Plugin von Nexus ist veraltet (installiert {setup.installedVersion}, mitgeliefert {setup.bundledVersion}).
        </p>
      ) : (
        <p className="db-dialog-hint">
          Das Claude-Code-Plugin von Nexus ist nicht installiert. Ohne Plugin kennt Claude dein Board, die Hooks und die <code>/nexus:…</code>-Skills nicht.
        </p>
      )}
      {!setup.claude && <p className="field-error">Claude Code wurde nicht gefunden (<code>claude</code> im PATH).</p>}
      {busy && <InstallProgress />}
      {phase.kind === 'done' && (
        <div role="status" className="plugin-install-done">
          {phase.result.steps.length > 0 && (
            <ul className="plugin-install-steps">
              {phase.result.steps.map((step) => (
                <li key={step}>
                  <span className="material-icons" aria-hidden="true">check_circle</span>
                  {step}
                </li>
              ))}
            </ul>
          )}
          <p>
            {update ? 'Aktualisiert' : 'Installiert'}. In laufenden Claude-Sessions <code>/reload-plugins</code> eingeben.
          </p>
        </div>
      )}
      {phase.kind === 'failed' && (
        <div role="alert" className="field-error">
          {phase.hints.map((hint) => (
            <pre key={hint}>{hint}</pre>
          ))}
        </div>
      )}
      <div className="actions">
        {phase.kind === 'done' ? (
          <button type="button" className="btn primary" onClick={onClose} autoFocus>
            Schließen
          </button>
        ) : (
          <>
            <button type="button" className="btn primary" disabled={busy || !setup.claude} onClick={install} autoFocus>
              {phase.kind === 'failed' ? 'Erneut versuchen' : update ? 'Aktualisieren' : 'Installieren'}
            </button>
            <button type="button" className="btn" disabled={busy} onClick={onLater}>
              Nicht jetzt
            </button>
            <button type="button" className="btn" disabled={busy} onClick={onNever}>
              Nicht mehr fragen
            </button>
          </>
        )}
      </div>
    </DbModal>
  );
}

// Fortschritt der laufenden Installation: fragt den Server regelmäßig ab und zeigt Balken, Schritt und Dauer.
// Solange kein Stand bekannt ist (oder der Schritt keine Gesamtzahl hat), läuft der Balken unbestimmt.
function InstallProgress() {
  const api = useApi();
  const [step, setStep] = useState<Step | null>(null);
  const [seconds, setSeconds] = useState(0);
  useEffect(() => {
    let alive = true;
    const started = Date.now();
    const poll = () => {
      api.getPluginInstallProgress().then(
        (report) => { if (alive && report.progress) setStep(report.progress); },
        () => {},
      );
      setSeconds(Math.floor((Date.now() - started) / 1000));
    };
    poll();
    const timer = setInterval(poll, POLL_MS);
    return () => { alive = false; clearInterval(timer); };
  }, [api]);
  const known = step !== null && step.total > 0;
  const percent = known ? Math.min(100, Math.round((step.done / step.total) * 100)) : 0;
  return (
    <div className="plugin-install-progress" role="status">
      <div className="plugin-install-progress-head">
        <span className="plugin-install-progress-label">{step?.label ?? 'Installation wird vorbereitet'} …</span>
        <span className="plugin-install-progress-meta">{known ? `${percent} % · ` : ''}{seconds} s</span>
      </div>
      <div
        className={`plugin-install-progress-bar${known ? '' : ' indeterminate'}`}
        role="progressbar"
        aria-label="Fortschritt der Installation"
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={known ? percent : undefined}
      >
        <span style={known ? { width: `${percent}%` } : undefined} />
      </div>
    </div>
  );
}
