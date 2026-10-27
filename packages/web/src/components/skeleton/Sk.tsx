// Einzelner Platzhalter-Balken mit Schimmer; Größe über Breite/Höhe, Form über `round`.
export function Sk({ w, h = 14, round }: { w?: number | string; h?: number | string; round?: boolean }) {
  return <span className={round ? 'sk round' : 'sk'} style={{ width: w, height: h }} />;
}
