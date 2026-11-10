// Ebene hinter dem Inhalt: Sterne, Sonne und perspektivisches Neon-Gitter; Aussehen je Phase und Stufe regelt theme.css
export function Horizon() {
  return (
    <div className="synth-horizon" aria-hidden="true">
      <div className="synth-stars" />
      <div className="synth-sun" />
      <div className="synth-grid" />
    </div>
  );
}
