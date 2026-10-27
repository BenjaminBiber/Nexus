import { useEffect, useRef, useState } from 'react';
import { useApi } from '../api/ApiContext.js';

const TARGETS = [
  { target: 'explorer', label: 'Im Explorer zeigen', icon: 'folder_open' },
  { target: 'vscode', label: 'In VS Code öffnen', icon: 'code' },
] as const;

// Knopf „Öffnen“ am Pfad des Dokuments: Menü mit Explorer (Datei markiert) und VS Code. Der Server startet das Programm
// auf diesem Rechner; ein Fehler (z. B. VS Code nicht im PATH) steht unter dem Menü.
export function DocOpenMenu({ pid, path, board }: { pid: string; path: string; board?: number }) {
  const api = useApi();
  const [open, setOpen] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const ref = useRef<HTMLDivElement>(null);

  // Klick außerhalb und Escape schließen das Menü
  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => { if (!ref.current?.contains(e.target as Node)) setOpen(false); };
    const onKey = (e: KeyboardEvent) => { if (e.key === 'Escape') setOpen(false); };
    document.addEventListener('mousedown', onDown);
    document.addEventListener('keydown', onKey);
    return () => {
      document.removeEventListener('mousedown', onDown);
      document.removeEventListener('keydown', onKey);
    };
  }, [open]);

  // Verwirft einen alten Fehler beim Wechsel des Dokuments
  useEffect(() => setError(null), [path, board]);

  const openIn = async (target: 'explorer' | 'vscode') => {
    setOpen(false);
    setError(null);
    try {
      await api.openDoc(pid, path, target, board);
    } catch (e) {
      setError((e as Error).message);
    }
  };

  return (
    <div className="doc-open" ref={ref}>
      <button
        type="button"
        className="doc-open-btn"
        aria-label="Datei öffnen"
        title="Datei öffnen"
        aria-haspopup="menu"
        aria-expanded={open}
        onClick={() => setOpen(!open)}
      >
        <span className="material-icons" aria-hidden="true">open_in_new</span>
      </button>
      {open && (
        <div className="doc-open-menu" role="menu" aria-label="Datei öffnen">
          {TARGETS.map((t) => (
            <button key={t.target} type="button" role="menuitem" onClick={() => void openIn(t.target)}>
              <span className="material-icons" aria-hidden="true">{t.icon}</span>
              {t.label}
            </button>
          ))}
        </div>
      )}
      {error && <p role="alert" className="error doc-open-error">{error}</p>}
    </div>
  );
}
