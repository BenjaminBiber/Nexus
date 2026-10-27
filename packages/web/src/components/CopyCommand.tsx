import { useState } from 'react';

// Zeigt einen Befehl in Festbreitenschrift mit Kopieren-Knopf; `compact` zeigt statt „Kopieren“ nur ein Symbol (schmale Spalten).
export function CopyCommand({ command, compact = false }: { command: string; compact?: boolean }) {
  const [copied, setCopied] = useState(false);

  // Kopiert in die Zwischenablage und meldet es kurz zurück
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(command);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      setCopied(false);
    }
  };

  return (
    <div className={compact ? 'code compact' : 'code'}>
      <span>{command}</span>
      <button type="button" className="btn small" onClick={copy} aria-label={`Befehl kopieren: ${command}`} title={compact ? (copied ? 'Kopiert' : 'Kopieren') : undefined}>
        {compact ? (copied ? '✓' : '⧉') : copied ? 'Kopiert' : 'Kopieren'}
      </button>
    </div>
  );
}
