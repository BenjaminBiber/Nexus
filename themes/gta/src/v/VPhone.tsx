import { useGame } from '../data/GameContext.js';

const QUESTION =
  'M12 2.5c3.3 0 5.8 2.1 5.8 5.1 0 2.3-1.3 3.5-2.6 4.5-1 .8-1.5 1.3-1.5 2.6h-3.6c0-2.4.9-3.5 2.2-4.5 1-.8 1.7-1.3 1.7-2.4 0-1-.8-1.7-2-1.7-1.3 0-2.2.8-2.3 2.2H6.1c.2-3.5 2.6-5.8 5.9-5.8z';

// Handy-Nachricht in der Fußleiste, wenn Agenten neue Fragen gestellt haben: gelbes Fragezeichen, Anzahl, Ansehen und Schließen
export function VPhone() {
  const { phone, answerPhone, dismissPhone } = useGame();
  if (!phone) return null;
  const text = phone.count === 1 ? 'Neue Frage' : `${phone.count} neue Fragen`;
  return (
    <section className="gta-v-phone" data-gta-part="phone" aria-label="Handy-Nachricht">
      <span className="gta-v-phone-icon" aria-hidden="true">
        <svg viewBox="0 0 24 24" fill="currentColor">
          <path d={QUESTION} />
          <circle cx="11.9" cy="19.2" r="2.3" />
        </svg>
      </span>
      <div className="gta-v-phone-body">
        <b>{text}</b>
        <span>{phone.count === 1 ? 'Ein Agent wartet auf deine Entscheidung.' : 'Agenten warten auf deine Entscheidung.'}</span>
        <div className="gta-v-phone-actions">
          <button type="button" className="gta-v-phone-open" onClick={answerPhone}>
            Ansehen
          </button>
          <button type="button" className="gta-v-phone-close" aria-label="Nachricht schließen" onClick={dismissPhone}>
            ×
          </button>
        </div>
      </div>
    </section>
  );
}
