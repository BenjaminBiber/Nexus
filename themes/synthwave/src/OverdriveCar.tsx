// Keilförmiger Sportwagen der 80er von der Seite: Karosserie im Neonverlauf, Rücklichtleiste, zwei Räder mit Speichenstern.
// Fährt im Overdrive einmal von links nach rechts über die Deck-Leiste und zieht Lichtstreifen hinter sich her.
export function OverdriveCar() {
  return (
    <span className="synth-car" aria-hidden="true">
      <i className="synth-car-trail" />
      <svg viewBox="0 0 124 36" focusable="false">
        <defs>
          <linearGradient id="synth-car-body" x1="0" y1="0" x2="1" y2="0">
            <stop offset="0" className="synth-car-stop-a" />
            <stop offset="1" className="synth-car-stop-b" />
          </linearGradient>
        </defs>
        <path className="synth-car-body" d="M4 27V20l12-4 30-3 16-7h28l16 10 14 2v9z" fill="url(#synth-car-body)" />
        <path className="synth-car-glass" d="M49 13.5 64 8h24l11 7z" />
        <path className="synth-car-line" d="M10 21h104" />
        <rect className="synth-car-tail" x="3" y="18.5" width="8" height="3" rx="1" />
        <rect className="synth-car-head" x="112" y="18" width="7" height="2.5" rx="1" />
        <g className="synth-car-wheel">
          <circle cx="30" cy="27" r="7" />
          <path d="M30 21v12M24 27h12M26 23l8 8M34 23l-8 8" />
        </g>
        <g className="synth-car-wheel">
          <circle cx="96" cy="27" r="7" />
          <path d="M96 21v12M90 27h12M92 23l8 8M100 23l-8 8" />
        </g>
      </svg>
    </span>
  );
}
