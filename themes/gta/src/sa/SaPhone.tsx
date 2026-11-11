import { useGame } from '../data/GameContext.js';

// Klobiges Handy der Preview, selbst gezeichnet: Antenne, Gehäuse, grünes Display, Tastenfeld
function Cell() {
  return (
    <svg className="gta-sa-cell" viewBox="0 0 44 84" aria-hidden="true" focusable="false">
      <g stroke="#000" strokeWidth="3" strokeLinejoin="round">
        <rect x="27" y="2" width="6" height="18" rx="2" fill="#2b2f33" />
        <rect x="5" y="16" width="34" height="66" rx="6" fill="#3b4148" />
        <rect x="10" y="22" width="24" height="18" rx="2" fill="#9fd58a" />
      </g>
      <g fill="#c9d3dc">
        {[46, 55, 64].flatMap((y) => [10, 19, 28].map((x) => <rect key={`${x}-${y}`} x={x} y={y} width="6" height="5" />))}
      </g>
    </svg>
  );
}

// Klingelndes Handy unten rechts, wenn Agenten neue Fragen gestellt haben: auf dem Display „Neue Frage“, daneben
// Annehmen (Y, zu den Fragen) und Ablehnen (N). Die Tasten selbst behandelt der GameProvider; bei reduzierter Bewegung
// wackelt das Handy nicht (theme.css).
export function SaPhone() {
  const { phone, answerPhone, dismissPhone } = useGame();
  if (!phone) return null;
  const text = phone.count === 1 ? 'Neue Frage' : `${phone.count} neue Fragen`;
  return (
    <section className="gta-sa-phone" data-gta-part="phone" aria-label={`Anruf: ${text}`}>
      <div className="gta-sa-phone-ring">
        <Cell />
        <span className="gta-sa-phone-display">{text}</span>
      </div>
      <div className="gta-sa-phone-actions">
        <button type="button" className="gta-sa-phone-answer" onClick={answerPhone}>
          <kbd>Y</kbd> Annehmen
        </button>
        <button type="button" className="gta-sa-phone-decline" onClick={dismissPhone}>
          <kbd>N</kbd> Ablehnen
        </button>
      </div>
    </section>
  );
}
