import type { QuestionOptionDto } from '../api/questionTypes.js';
import { Button } from './ui/Button.js';
import { Card } from './ui/Card.js';

// Eine Option einer Recherche: Titel, Text, Vorteile und Nachteile; wählbar nur, wenn `onPick` übergeben wird.
export function OptionCard({
  option,
  picked = false,
  disabled = false,
  onPick,
}: {
  option: QuestionOptionDto;
  picked?: boolean;
  /** Gesperrt, z. B. solange ein Freitext eingegeben ist */
  disabled?: boolean;
  onPick?: () => void;
}) {
  return (
    <Card as="article" className={`option-card${option.recommended ? ' recommended' : ''}${picked ? ' picked' : ''}`}>
      <div className="option-head">
        <span className="mono muted">Option {option.id}</span>
        {option.recommended && <span className="badge q-new">empfohlen</span>}
      </div>
      <strong className="option-title">{option.title}</strong>
      <span className="option-text">{option.text}</span>
      <div>
        <div className="muted option-label">Vorteile</div>
        <ul className="option-list">
          {option.pros.map((p) => (
            <li key={p}>
              <span className="pro" aria-hidden="true">+</span>
              <span>{p}</span>
            </li>
          ))}
        </ul>
      </div>
      <div>
        <div className="muted option-label">Nachteile</div>
        <ul className="option-list">
          {option.cons.map((c) => (
            <li key={c}>
              <span className="con" aria-hidden="true">–</span>
              <span>{c}</span>
            </li>
          ))}
        </ul>
      </div>
      {onPick && (
        <Button variant={picked ? 'primary' : 'secondary'} aria-pressed={picked} disabled={disabled && !picked} onClick={onPick}>
          {picked ? 'Gewählt' : 'Diese Option wählen'}
        </Button>
      )}
    </Card>
  );
}
