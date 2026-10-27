// Was der Dialog vom Konflikt braucht: Nummer der Frage im aktuellen Stand und welche Änderung offen ist
export interface QuestionConflictInfo {
  current: { id: string };
  scope: 'decision' | 'relations';
}

// Speicherkonflikt: die Frage wurde zwischenzeitlich geändert (z. B. durch eine neue Recherche). Die Entscheidung liegt beim Benutzer.
export function QuestionConflictDialog({ conflict, onChoose }: { conflict: QuestionConflictInfo; onChoose: (choice: 'reload' | 'keep') => void }) {
  return (
    <div role="alertdialog" aria-labelledby="question-conflict-title" className="conflict">
      <strong id="question-conflict-title">Speicherkonflikt</strong>
      <p>
        Die Frage {conflict.current.id} wurde währenddessen geändert, zum Beispiel durch eine neue Recherche.{' '}
        {conflict.scope === 'relations' ? 'Deine Änderung an Bezügen und Abhängigkeiten ist noch nicht gespeichert.' : 'Deine Entscheidung ist noch nicht gespeichert.'}
      </p>
      <div className="actions">
        <button type="button" className="btn primary" onClick={() => onChoose('reload')}>
          Neu laden
        </button>
        <button type="button" className="btn" onClick={() => onChoose('keep')}>
          Meine Fassung behalten
        </button>
      </div>
    </div>
  );
}
