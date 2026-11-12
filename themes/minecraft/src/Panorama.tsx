// Hintergrund wie im Titelbildschirm: die 6 Panorama-Bilder als Würfel um den Betrachter, langsam drehend (theme.css).
// Ohne Ressourcen bleiben die Seiten leer und der Erdbraun-Verlauf darunter ist zu sehen.
const FACES = ['front', 'right', 'back', 'left', 'top', 'bottom'] as const;

export function Panorama() {
  return (
    <div className="mc-panorama" aria-hidden="true">
      <div className="mc-panorama-cube">
        {FACES.map((face) => (
          <div key={face} className={`mc-panorama-face mc-panorama-face--${face}`} />
        ))}
      </div>
    </div>
  );
}
