import { useState } from 'react';

// Kleiner Knopf, der eine ID in die Zwischenablage kopiert und es kurz zurückmeldet.
export function CopyIdButton({ id, label }: { id: string; label: string }) {
  const [copied, setCopied] = useState(false);

  // Kopiert die volle ID; ohne Zugriff auf die Zwischenablage bleibt der Knopf unverändert
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(id);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      setCopied(false);
    }
  };

  return (
    <button type="button" className="btn small copy-id" onClick={copy} title={id} aria-label={`${label} kopieren: ${id}`}>
      {copied ? 'Kopiert' : 'ID kopieren'}
    </button>
  );
}
