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

// Klingelndes Handy als Zeile am rechten Ende des Fußes, wenn Agenten neue Fragen gestellt haben: kleines Handy, auf
// dem Display „Neue Frage“, daneben Annehmen (Y, zu den Fragen) und Ablehnen (N). Es ist Teil des Layouts und verdeckt
// keinen Inhalt. Die Hülle mit role="status" bleibt stehen, damit Screenreader den Anruf ansagen. Die Tasten selbst
// behandelt der GameProvider; bei reduzierter Bewegung wackelt das Handy nicht (theme.css).
export function SaPhone() {
  const { phone, answerPhone, dismissPhone } = useGame();
  const text = phone ? (phone.count === 1 ? 'Neue Frage' : `${phone.count} neue Fragen`) : '';
  return (
    <div className="gta-sa-phone-live" role="status">
      {phone ? (
        <section className="gta-sa-phone" data-gta-part="phone" aria-label={`Anruf: ${text}`}>
          <Cell />
          <span className="gta-sa-phone-display">{text}</span>
          <div className="gta-sa-phone-actions">
            <button type="button" className="gta-sa-phone-answer" onClick={answerPhone}>
              <kbd>Y</kbd> Annehmen
            </button>
            <button type="button" className="gta-sa-phone-decline" onClick={dismissPhone}>
              <kbd>N</kbd> Ablehnen
            </button>
          </div>
        </section>
      ) : null}
    </div>
  );
}
