import { Button } from './ui/Button.js';
import { Dialog } from './ui/Dialog.js';

// Konflikt beim Speichern: das Ticket wurde zwischenzeitlich geändert. Die Entscheidung liegt beim Benutzer.
// Genutzt vom Datei-Ticket (useTicket) und vom DB-Ticket (useDbTicket); gebraucht wird nur die ID des aktuellen Stands.
export function ConflictDialog({ conflict, onChoose }: { conflict: { current: { id: string } }; onChoose: (choice: 'reload' | 'overwrite') => void }) {
  return (
    <Dialog role="alertdialog" labelledBy="conflict-title" className="conflict">
      <strong id="conflict-title">Dieses Ticket wurde zwischenzeitlich geändert</strong>
      <p>Jemand anderes hat {conflict.current.id} geändert, während du es bearbeitet hast. Es wurde nichts überschrieben.</p>
      <div className="actions">
        <Button variant="primary" onClick={() => onChoose('reload')}>
          Aktuellen Stand laden
        </Button>
        <Button onClick={() => onChoose('overwrite')}>
          Meine Änderung trotzdem speichern
        </Button>
      </div>
    </Dialog>
  );
}
