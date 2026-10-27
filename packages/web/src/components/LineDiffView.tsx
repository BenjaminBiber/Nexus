import { lineDiff } from '../markdown/lineDiff.js';

export interface LineDiffViewProps {
  original: string;
  proposal: string;
  /** Beschriftung des Blocks; die Tests greifen darüber auf den Diff zu */
  label?: string;
  className?: string;
}

// Zeilen-Diff Original ↔ Vorschlag; Präfix und Klasse machen die Art der Zeile auch in den Tests sichtbar
export function LineDiffView({ original, proposal, label = 'Änderungen', className = 'revise-diff' }: LineDiffViewProps) {
  const lines = lineDiff(original, proposal);
  return (
    <div className={className} aria-label={label}>
      {lines.map((line, index) => (
        <div key={index} className={`diff-line diff-${line.kind}`} aria-label={`${line.kind === 'add' ? 'hinzugefügt' : line.kind === 'remove' ? 'entfernt' : 'unverändert'}: ${line.text}`}>
          <span className="diff-sign" aria-hidden="true">{line.kind === 'add' ? '+' : line.kind === 'remove' ? '−' : ' '}</span>
          <span className="diff-text">{line.text}</span>
        </div>
      ))}
    </div>
  );
}
