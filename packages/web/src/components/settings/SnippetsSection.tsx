import { useState } from 'react';
import { useApi } from '../../api/ApiContext.js';
import type { SnippetDto } from '../../api/types.js';
import { useAsync } from '../../hooks/useAsync.js';
import { applySnippets, snippetSlug } from '../../theme/snippets.js';
import { SettingsSection } from './SettingRow.js';

/** Offener Editor: bestehender Schnipsel (id) oder neuer (id = null) */
type Draft = { id: string | null; name: string; css: string };

// Eigene CSS-Schnipsel: Liste mit An/Aus, Bearbeiten, Anlegen und Löschen; aktive liegen über dem Theme.
export function SnippetsSection() {
  const api = useApi();
  const list = useAsync(() => api.listSnippets(), [api]);
  const [draft, setDraft] = useState<Draft | null>(null);
  const [error, setError] = useState<string | null>(null);
  const snippets = list.state.status === 'ready' ? list.state.data : [];

  // Führt eine Änderung aus, lädt Liste und Schnipsel-CSS neu; Fehler als Meldung
  const change = async (fn: () => Promise<unknown>): Promise<boolean> => {
    setError(null);
    try {
      await fn();
    } catch (e) {
      setError((e as Error).message);
      return false;
    }
    applySnippets();
    await list.reload();
    return true;
  };

  // Speichert den offenen Editor; neue Schnipsel bekommen eine ID aus dem Namen
  const save = async () => {
    if (!draft) return;
    const id = draft.id ?? snippetSlug(draft.name);
    if (await change(() => api.saveSnippet(id, { name: draft.name, css: draft.css }))) setDraft(null);
  };

  // Löscht nach Bestätigung
  const remove = (snippet: SnippetDto) => {
    if (window.confirm(`Schnipsel „${snippet.name}“ löschen?`)) void change(() => api.deleteSnippet(snippet.id));
  };

  return (
    <SettingsSection title="Eigene CSS-Schnipsel" description="Liegen über dem Theme. Kein @import, keine externen Adressen, max. 64 KB.">
      {list.state.status === 'error' && <p className="muted">Schnipsel konnten nicht geladen werden.</p>}
      <ul className="snippet-list">
        {snippets.map((snippet) => (
          <li key={snippet.id} className="snippet-row">
            <label>
              <input type="checkbox" aria-label={`${snippet.name} aktiv`} checked={snippet.enabled} onChange={(e) => void change(() => api.toggleSnippet(snippet.id, e.target.checked))} />
              <span>{snippet.name}</span>
            </label>
            <button type="button" className="icon-btn" aria-label={`${snippet.name} bearbeiten`} onClick={() => { setError(null); setDraft({ id: snippet.id, name: snippet.name, css: snippet.css }); }}>
              <span className="material-icons" aria-hidden="true">edit</span>
            </button>
            <button type="button" className="icon-btn" aria-label={`${snippet.name} löschen`} onClick={() => remove(snippet)}>
              <span className="material-icons" aria-hidden="true">delete</span>
            </button>
          </li>
        ))}
      </ul>
      {draft ? (
        <div className="snippet-editor">
          <input type="text" aria-label="Name" placeholder="Name" value={draft.name} onChange={(e) => setDraft({ ...draft, name: e.target.value })} />
          <textarea aria-label="CSS" spellCheck={false} value={draft.css} onChange={(e) => setDraft({ ...draft, css: e.target.value })} />
          {error && <p className="save-status error" role="alert">{error}</p>}
          <div className="snippet-actions">
            <button type="button" className="btn" onClick={() => setDraft(null)}>Abbrechen</button>
            <button type="button" className="btn primary" disabled={!draft.name.trim()} onClick={() => void save()}>Speichern</button>
          </div>
        </div>
      ) : (
        <>
          {error && <p className="save-status error" role="alert">{error}</p>}
          <button type="button" className="btn" onClick={() => { setError(null); setDraft({ id: null, name: '', css: '' }); }}>
            <span className="material-icons" aria-hidden="true">add</span> Neuer Schnipsel
          </button>
        </>
      )}
    </SettingsSection>
  );
}
