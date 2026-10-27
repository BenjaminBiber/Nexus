// Konflikt beim Speichern: das Ticket wurde zwischenzeitlich geändert. Die Entscheidung liegt beim Benutzer.
// Genutzt vom Datei-Ticket (useTicket) und vom DB-Ticket (useDbTicket); gebraucht wird nur die ID des aktuellen Stands.
export function ConflictDialog({ conflict, onChoose }: { conflict: { current: { id: string } }; onChoose: (choice: 'reload' | 'overwrite') => void }) {
  return (
    <div role="alertdialog" aria-labelledby="conflict-title" className="conflict">
      <strong id="conflict-title">Dieses Ticket wurde zwischenzeitlich geändert</strong>
      <p>Jemand anderes hat {conflict.current.id} geändert, während du es bearbeitet hast. Es wurde nichts überschrieben.</p>
      <div className="actions">
        <button type="button" className="btn primary" onClick={() => onChoose('reload')}>
          Aktuellen Stand laden
        </button>
        <button type="button" className="btn" onClick={() => onChoose('overwrite')}>
          Meine Änderung trotzdem speichern
        </button>
      </div>
    </div>
  );
}
