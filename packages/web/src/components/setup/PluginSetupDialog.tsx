import { useId, useState } from 'react';
import { useApi } from '../../api/ApiContext.js';
import type { PluginInstallDto, PluginSetupDto } from '../../api/types.js';
import { DbModal } from '../db/DbModal.js';

type Phase = { kind: 'idle' } | { kind: 'busy' } | { kind: 'done'; result: PluginInstallDto } | { kind: 'failed'; hints: string[] };

// Bietet nach dem Start der App an, das mitgelieferte Claude-Code-Plugin zu installieren oder zu aktualisieren;
// während des Laufs sind Knöpfe und Esc gesperrt.
export function PluginSetupDialog({ setup, onLater, onNever, onClose }: { setup: PluginSetupDto; onLater: () => void; onNever: () => void; onClose: () => void }) {
  const api = useApi();
  const titleId = useId();
  const [phase, setPhase] = useState<Phase>({ kind: 'idle' });
  const busy = phase.kind === 'busy';
  const install = () => {
    setPhase({ kind: 'busy' });
    api.installPlugin().then(
      (result) => setPhase(result.ok ? { kind: 'done', result } : { kind: 'failed', hints: result.hints.length ? result.hints : ['Die Installation ist gescheitert.'] }),
      (error: unknown) => setPhase({ kind: 'failed', hints: [error instanceof Error ? error.message : String(error)] }),
    );
  };
  return (
    <DbModal role="alertdialog" labelledBy={titleId} onEscape={() => { if (!busy) (phase.kind === 'done' ? onClose : onLater)(); }} backdropClassName="db-dialog-backdrop" className="panel db-dialog">
      <strong id={titleId}>Claude-Code-Plugin {setup.state === 'outdated' ? 'aktualisieren' : 'installieren'}</strong>
      {setup.state === 'outdated' ? (
        <p className="db-dialog-hint">
          Das Claude-Code-Plugin von Nexus ist veraltet (installiert {setup.installedVersion}, mitgeliefert {setup.bundledVersion}).
        </p>
      ) : (
        <p className="db-dialog-hint">
          Das Claude-Code-Plugin von Nexus ist nicht installiert. Ohne Plugin kennt Claude dein Board, die Hooks und die <code>/nexus:…</code>-Skills nicht.
        </p>
      )}
      {!setup.claude && <p className="field-error">Claude Code wurde nicht gefunden (<code>claude</code> im PATH).</p>}
      {busy && <p role="status">Plugin wird bei Claude Code angemeldet … (10–30 s)</p>}
      {phase.kind === 'done' && (
        <p role="status">
          Installiert. In laufenden Claude-Sessions <code>/reload-plugins</code> eingeben.
        </p>
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
              {phase.kind === 'failed' ? 'Erneut versuchen' : 'Installieren'}
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
