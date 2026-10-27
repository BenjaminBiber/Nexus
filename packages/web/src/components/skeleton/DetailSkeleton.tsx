import { Sk } from './Sk.js';

// Platzhalter für Detailseiten (Ticket, Frage, Einstellungen): Kopf mit Titel und Status, Haupt- und Seitenspalte
export function DetailSkeleton() {
  return (
    <div aria-hidden="true">
      <div className="ticket-head">
        <div className="ticket-ident"><Sk w={64} h={18} /><Sk w={80} h={22} round /></div>
        <Sk h={30} w="55%" />
      </div>
      <div className="ticket-grid">
        <div className="ticket-main">
          <div className="panel"><Sk w={110} h={13} /><Sk h={14} /><Sk h={14} w="92%" /><Sk h={14} w="64%" /></div>
          <div className="panel"><Sk w={90} h={13} /><Sk h={44} /><Sk h={44} /></div>
        </div>
        <div className="ticket-side">
          <div className="panel"><Sk w={80} h={13} /><Sk h={14} w="70%" /><Sk h={14} w="50%" /></div>
        </div>
      </div>
    </div>
  );
}
